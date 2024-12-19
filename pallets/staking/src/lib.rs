//! # Xode Staking Pallet
//! 
//! XaverNodes
//! Candidates
//! DesiredCandidates
//! Invulnerables
//! 
//! Steps
//! 1. add_xaver_nodes (Block 0/Hooks)
//! 2. register_candidate
//! 3. bond_candidate
//! 
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::dispatch::DispatchResult;
	use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*, };
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{AccountIdConversion, Zero,};
	use sp_runtime::Saturating;
	use scale_info::prelude::vec::Vec;
	use scale_info::prelude::vec;
	use hex::decode;

	// Sessions
	use pallet_session::SessionManager;
	use sp_staking::SessionIndex;

	use frame_support::PalletId;
	use frame_support::traits::{Currency, ReservableCurrency};

	pub type BalanceOf<T> = <<T as Config>::StakingCurrency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Runtime configuration
	#[pallet::config]
	pub trait Config: pallet_balances::Config + pallet_collator_selection::Config + pallet_aura::Config + frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// A type representing the weights required by the dispatchables of this pallet.
		type WeightInfo: WeightInfo;

		/// The maximum proposed candidates
		type MaxProposedCandidates: Get<u32>;

		/// The maximum proposed candidate delegates
		type MaxProposedCandidateDelegates: Get<u32>;

		/// Xaver nodes that will be always present: vec!["",""]
		type XaverNodes: Get<&'static [&'static str]>;

		/// The staking currency trait.
		type StakingCurrency: ReservableCurrency<Self::AccountId>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);
	
	/// Candidate info
	#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, scale_info::TypeInfo, MaxEncodedLen,)]
	pub struct CandidateInfo<AccountId, Balance, BlockNumber> {
		pub who: AccountId,
		pub bond: Balance,
		pub total_stake: Balance,
		pub last_updated: BlockNumber,
		pub leaving: bool,
		pub offline: bool,
		pub commission: u8,
	}
		
	/// Proposed candidates 
	#[pallet::storage]
	pub type ProposedCandidates<T: Config> = StorageValue<
		_,
		BoundedVec<CandidateInfo<T::AccountId, BalanceOf<T>, BlockNumberFor<T>>, T::MaxCandidates>,
		ValueQuery,
	>;

	/// Desired candidates storage
	#[pallet::storage]
	pub type DesiredCandidates<T: Config> = StorageValue<_, BoundedVec<T::AccountId, T::MaxCandidates>, ValueQuery>;

	/// Waiting candidates storage (Combination of the desired and proposed candidates)
	#[pallet::storage]
	pub type WaitingCandidates<T: Config> = StorageValue<_, BoundedVec<T::AccountId, T::MaxCandidates>, ValueQuery>;

	/// Delegator info (Delegation)
	#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, scale_info::TypeInfo, MaxEncodedLen,)]
	pub struct Delegation<AccountId, Balance> {
		pub delegator: AccountId,
		pub stake: Balance,
	}

	/// Delegations
	#[pallet::storage]
	pub type Delegations<T: Config> = StorageMap<
		_, 
		Twox64Concat, 
		T::AccountId, 
		BoundedVec<Delegation<T::AccountId, BalanceOf<T>>, T::MaxProposedCandidateDelegates>, 
		OptionQuery
	>;

	/// Next block number storage
	#[pallet::storage]
    #[pallet::getter(fn next_block_number)]
    pub type NextBlockNumber<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;


	/// ====================
	/// Events (past events)
	/// ====================
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		InvulernableAdded { _invulnerable: T::AccountId, },

		DesiredCandidateAdded { _desired_candidate: T::AccountId, },

		TreasuryAccountRetrieved { _treasury: T::AccountId, _data: T::AccountData, },

		ProposedCandidateAdded  { _proposed_candidate: T::AccountId, },
		ProposedCandidateBonded  { _proposed_candidate: T::AccountId, },
		ProposedCandidateLeft  { _proposed_candidate: T::AccountId, },
		ProposedCandidateRemoved { _proposed_candidate: T::AccountId, },
		ProposedCandidateTotalStake { _proposed_candidate: T::AccountId, },
		ProposedCandidateCommissionSet { _proposed_candidate: T::AccountId, },
		ProposedCandidateOffline { _proposed_candidate: T::AccountId, },
		ProposedCandidateOnline { _proposed_candidate: T::AccountId, },

		WaitingCandidateAdded { _waiting_candidate: T::AccountId, },
		WaitingCandidateRemoved { _waiting_candidate: T::AccountId, },

		DelegationAdded { _delegator: T::AccountId, },
		DelegationRevoked { _delegator: T::AccountId, },
	}

	/// ======
	/// Errors
	/// ======
	#[pallet::error]
	pub enum Error<T> {
		InvulnerableAlreadyExist,
		InvulnerableMaxExceeded,
		InvulernableMember,

		DesiredCandidateAlreadyExist,
		DesiredCandidateMaxExceeded,

		CandidateAlreadyExist,
		CandidateMaxExceeded,

		ProposedCandidateAlreadyExist,
		ProposedCandidateMaxExceeded,
		ProposedCandidateNotFound,
		ProposedCandidateInvalidCommission,

		WaitingCandidateAlreadyExist,
		WaitingCandidateMaxExceeded,
		WaitingCandidateNotFound,
		WaitingCandidateMember,
		WaitingCandidatesEmpty,

		DelegationToSelfNotAllowed,
		DelegationInsufficientAmount,
		DelegationCandidateDoesNotExist,
		DelegationDelegatorDoesNotExist,
		DelegationsDoesNotExist,
		DelegationsMaxExceeded,
	}

	/// =====
	/// Hooks
	/// =====
	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_current_block: BlockNumberFor<T>) -> Weight {
			match NextBlockNumber::<T>::get() {
				Some(_next_block) => {
					// Todo, since this is replaced by session
					T::DbWeight::get().reads(1)
				}
				None => {
					// At block zero
					Self::add_xaver_nodes();
					T::DbWeight::get().reads(1)
				}
			}
		}
	}

	/// ===============
	/// Extrinsic Calls
	/// ===============
	#[pallet::call]
	impl<T: Config> Pallet<T> {

		/// Retrieve the treasury account
		/// Note:
		/// 	Temporary extrinsic to monitor fees.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().writes(1))]
		pub fn retrieve_treasury_account(_origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			// May panic during runtime (Must fix!)
		 	let treasury= PalletId(*b"py/trsry").try_into_account().expect("Error converting to account");
		 	let account_info = frame_system::Pallet::<T>::account(&treasury);
		 	let account_data = account_info.data;	
		 	Self::deposit_event(Event::TreasuryAccountRetrieved { _treasury: treasury, _data: account_data, });
		 	Ok(().into())
		}

		/// Register a new candidate in the Proposed Candidate list
		#[pallet::call_index(1)]
		#[pallet::weight(<weights::SubstrateWeight<T> as WeightInfo>::register_candidate())]
		pub fn register_candidate(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
            ensure!(!ProposedCandidates::<T>::get().iter().any(|c| c.who == who), Error::<T>::ProposedCandidateAlreadyExist);
            ensure!(
                ProposedCandidates::<T>::get().len() < T::MaxProposedCandidates::get() as usize,
                Error::<T>::ProposedCandidateMaxExceeded
            );
            let candidate_info = CandidateInfo {
                who: who.clone(),
                bond: Zero::zero(),
                total_stake: Zero::zero(),
                last_updated: frame_system::Pallet::<T>::block_number(),
				leaving: false,
				offline: false,
				commission: 0,
            };
            ProposedCandidates::<T>::try_mutate(|candidates| -> Result<(), DispatchError> {
                candidates.try_push(candidate_info).map_err(|_| Error::<T>::ProposedCandidateMaxExceeded)?;
                Ok(())
            })?;
			Self::deposit_event(Event::ProposedCandidateAdded { _proposed_candidate: who });
			Ok(().into())
		}

		/// Bond Proposed Candidate
		/// Note:
		/// 	Get the difference of the existing bond then effect the result: zero no change;
		/// 	if greater than zero, reserve the difference; otherwise unreserve.  Once the bond of 
		/// 	a candidate is updated, sort immediately the proposed candidates.
		/// Todo: 
		/// 	How do we deal with the reserve and unreserve for some reason fails?
		#[pallet::call_index(2)]
		#[pallet::weight(<weights::SubstrateWeight<T> as WeightInfo>::bond_candidate())]
		pub fn bond_candidate(origin: OriginFor<T>, new_bond: BalanceOf<T>,) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			// When we set the new bond to zero we assume that the candidate is leaving
			if new_bond == Zero::zero() {
				ensure!(!WaitingCandidates::<T>::get().contains(&who), Error::<T>::WaitingCandidateMember);
				ensure!(!pallet_collator_selection::Invulnerables::<T>::get().contains(&who), Error::<T>::InvulernableMember);
			} 

			ProposedCandidates::<T>::mutate(|candidates| {
				if let Some(candidate) = candidates.iter_mut().find(|c| c.who == who) {
					if candidate.bond > new_bond {
						// Decrease the bond and reserve or might leave (new bond == 0)
						// Unreserve the difference, of the new bond is 0, unreserve the whole bond because the bond difference
						// is equal to the exisiting bond because any number substracted by 0 would remain the same.
						let bond_diff = candidate.bond.saturating_sub(new_bond);
						candidate.bond = new_bond;
						if bond_diff > Zero::zero() {
							T::StakingCurrency::unreserve(&who, bond_diff);
						}
					} else {
						// Increase the bond and reserve.  If the new bond is greater than the existing bond then just replace
						// the current bond with the new one and get the difference to increase the reserve.
						let bond_diff = new_bond.saturating_sub(candidate.bond);
						candidate.bond = new_bond;
						if bond_diff > Zero::zero() {
							let _ = T::StakingCurrency::reserve(&who, bond_diff);
						}
					}
					candidate.last_updated = frame_system::Pallet::<T>::block_number();
				}
			});

			Self::sort_proposed_candidates();
			Self::deposit_event(Event::ProposedCandidateBonded { _proposed_candidate: who });
			Ok(().into())
		}

		/// Set commission
		/// Note:
		/// 	Numbers accepted are from 1 to 100 and no irrational numbers
		#[pallet::call_index(3)]
		#[pallet::weight(<weights::SubstrateWeight<T> as WeightInfo>::set_commission_of_candidate())]
		pub fn set_commission_of_candidate(origin: OriginFor<T>, commission: u8) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			// Commission control (1-100 percent only)
			ensure!(commission >= 1 && commission <= 100, Error::<T>::ProposedCandidateInvalidCommission);
			// Set commission
			ProposedCandidates::<T>::mutate(|candidates| {
				if let Some(candidate) = candidates.iter_mut().find(|c| c.who == who) {
					candidate.commission = commission;
					candidate.last_updated = frame_system::Pallet::<T>::block_number();

					let _ = Self::remove_waiting_candidate(who.clone());
				}
			});

			Self::deposit_event(Event::ProposedCandidateCommissionSet { _proposed_candidate: who });
			Ok(().into())
		}

		/// Leave Proposed Candidate
		/// Note:
		/// 	Once the leaving flag is set to true, immediately remove the account in the
		/// 	Waiting Candidate list.
		#[pallet::call_index(4)]
		#[pallet::weight(<weights::SubstrateWeight<T> as WeightInfo>::leave_candidate())]
		pub fn leave_candidate(origin: OriginFor<T>,) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ProposedCandidates::<T>::mutate(|candidates| {
				if let Some(candidate) = candidates.iter_mut().find(|c| c.who == who) {
					candidate.leaving = true;
					candidate.last_updated = frame_system::Pallet::<T>::block_number();

					let _ = Self::remove_waiting_candidate(who.clone());
				}
			});
			Self::deposit_event(Event::ProposedCandidateLeft { _proposed_candidate: who });
			Ok(().into())
		}

		/// Stake Proposed Candidate
		/// Note:
		/// 	To stake a proposed candidate means to delegate a balance for the candidate.
		/// 	The balance is reserved.
		/// 	The stake will remain in the storage even if the candidate leaves.
		/// Todo:
		/// 	How to handle the reservation if there is a failure in adding the delegation.
		/// 	Clean delegations when a candidate leaves to save space.
		#[pallet::call_index(5)]
		#[pallet::weight(<weights::SubstrateWeight<T> as WeightInfo>::stake_candidate())]
		pub fn stake_candidate(origin: OriginFor<T>, candidate: T::AccountId, amount: BalanceOf<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			
			// Provide some controls
			ensure!(who != candidate, Error::<T>::DelegationToSelfNotAllowed);
			ensure!(ProposedCandidates::<T>::get().iter().any(|c| c.who == candidate), Error::<T>::DelegationCandidateDoesNotExist); 
			ensure!(T::StakingCurrency::free_balance(&who) >= amount, Error::<T>::DelegationInsufficientAmount);

			// Reserve the balance before updating the stake amount of the delegator
			let _ = T::StakingCurrency::reserve(&who, amount);

			// Update delegation stake amount
			let mut delegations = Delegations::<T>::get(&candidate).unwrap_or_default();
			if let Some(delegation) = delegations.iter_mut().find(|d| d.delegator == who) {
				delegation.stake += amount;
			} else {
				let _ = delegations.try_push(Delegation { delegator: who.clone(), stake: amount }).map_err(|_| Error::<T>::DelegationsMaxExceeded)?;
			}

			// Finaly, update the storage
			Delegations::<T>::insert(&candidate, delegations);
			
			// Update the proposed candidate total stake amount
			let _ = Self::total_stake_proposed_candidate(candidate);
			Self::deposit_event(Event::DelegationAdded { _delegator: who });
			Ok(().into())
		}

		/// Unstake Proposed Candidate
		/// Note:
		/// 	Remove first the delegation (stake amount) before unreserving
		#[pallet::call_index(6)]
		#[pallet::weight(<weights::SubstrateWeight<T> as WeightInfo>::unstake_candidate())]
		pub fn unstake_candidate(origin: OriginFor<T>, candidate: T::AccountId) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			
			// Extract the delegations for that candidate
			let mut delegations = Delegations::<T>::get(&candidate).ok_or(Error::<T>::DelegationsDoesNotExist)?;
			
			// Extract the current stake of that delegator who wants to unstake
			let position = delegations.iter().position(|c| c.delegator == who).ok_or(Error::<T>::DelegationDelegatorDoesNotExist)?;
			let stake_amount = delegations[position].stake;

			// New list of delegations without the delegator who unstake
			delegations.retain(|c| c.delegator != who);

			// Update the delegation storage
			if delegations.is_empty() {
				// If there are no more delegators, remove the delegation for that candidate
				Delegations::<T>::remove(&candidate);
			} else {
				// Insert the new delegations without the delegator who unstake
				Delegations::<T>::insert(&candidate, delegations);
			}

			// Finaly, unreserve the balance
			T::StakingCurrency::unreserve(&who, stake_amount);

			// Update the proposed candidate total stake amount
			let _ = Self::total_stake_proposed_candidate(candidate);
			Self::deposit_event(Event::DelegationRevoked { _delegator: who });
			Ok(().into())
		}

		/// Offline Proposed Candidate 
		/// Note:
		///		Temporarily leave the candidacy without having to unbond and unstake
		#[pallet::call_index(7)]
		#[pallet::weight(<weights::SubstrateWeight<T> as WeightInfo>::offline_candidate())]
		pub fn offline_candidate(origin: OriginFor<T>,) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ProposedCandidates::<T>::mutate(|candidates| {
				if let Some(candidate) = candidates.iter_mut().find(|c| c.who == who) {
					candidate.offline = true;
					candidate.last_updated = frame_system::Pallet::<T>::block_number();
				}
			});
			Self::deposit_event(Event::ProposedCandidateOffline { _proposed_candidate: who });
			Ok(().into())
		}

		/// Online Proposed Candidate 
		/// Note:
		///		Make the candidate online again
		#[pallet::call_index(8)]
		#[pallet::weight(<weights::SubstrateWeight<T> as WeightInfo>::online_candidate())]
		pub fn online_candidate(origin: OriginFor<T>,) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ProposedCandidates::<T>::mutate(|candidates| {
				if let Some(candidate) = candidates.iter_mut().find(|c| c.who == who) {
					candidate.offline = false;
					candidate.last_updated = frame_system::Pallet::<T>::block_number();
				}
			});
			Self::deposit_event(Event::ProposedCandidateOnline { _proposed_candidate: who });
			Ok(().into())
		}
	}

	/// =======
	/// Helpers
	/// =======
	impl<T: Config> Pallet<T> {
		
		/// Convert AuthorityId to AccountId
		/// Note:
		/// 	Needed since we use Hex values when we set the Xaver Nodes at runtime
		pub fn authority_to_account(authority: T::AuthorityId) -> T::AccountId {
			let authority_bytes = authority.encode();
			// Panic if not properly configured at runtime
			let account = <T as frame_system::Config>::AccountId::decode(&mut authority_bytes.as_slice()).unwrap();
			account
		}

		/// Add an account to pallet_collator_selection invulnerables
		/// Substrate Reference:
		/// 	https://github.com/paritytech/polkadot-sdk/blob/stable2409/cumulus/pallets/collator-selection/src/lib.rs#L841
		pub fn add_invulnerable (invulnerable: T::AccountId) -> DispatchResult {
			pallet_collator_selection::Invulnerables::<T>::try_mutate(|invulnerables| -> DispatchResult {
				ensure!(!invulnerables.contains(&invulnerable), Error::<T>::InvulnerableAlreadyExist);
				invulnerables.try_push(invulnerable.clone()).map_err(|_| Error::<T>::InvulnerableMaxExceeded)?;
				Self::deposit_event(Event::InvulernableAdded { _invulnerable: invulnerable });
				Ok(())
			})
		}	

		/// Add a desired candidate
		/// Note:
		/// 	Xaver nodes are automatically added to desired candidates (occupies the first priority).  
		/// 	The remaining slot will be coming from the candidates.
		pub fn add_desired_candidate(desired_candidate: T::AccountId) -> DispatchResult {
			DesiredCandidates::<T>::try_mutate(|desired_candidates| -> DispatchResult {
				ensure!(!desired_candidates.contains(&desired_candidate), Error::<T>::DesiredCandidateAlreadyExist);
				desired_candidates.try_push(desired_candidate.clone()).map_err(|_| Error::<T>::DesiredCandidateMaxExceeded)?;
				Self::deposit_event(Event::DesiredCandidateAdded { _desired_candidate: desired_candidate });
				Ok(())
			})
		}

		/// Remove a proposed candidate
		/// Note:
		/// 	This is called upon cleaning of the proposed candidate storage for a candidate who is leaving 
		/// 	regardless if he/she has still a bond.
		pub fn remove_proposed_candidate(proposed_candidate: T::AccountId) -> DispatchResult {
            ProposedCandidates::<T>::try_mutate(|proposed_candidates| -> DispatchResult {
                proposed_candidates.retain(|c| c.who != proposed_candidate); 
                Self::deposit_event(Event::ProposedCandidateRemoved { _proposed_candidate: proposed_candidate });
                Ok(())
            })
		}

		/// Add a waiting candidate
		/// Note:
		/// 	At runtime instantiation the waiting candidates are the xaver nodes, prior sessions will now 
		/// 	include the proposed candidates. 
		pub fn add_waiting_candidate(waiting_candidate: T::AccountId) -> DispatchResult {
			WaitingCandidates::<T>::try_mutate(|waiting_candidates| -> DispatchResult {
				ensure!(!waiting_candidates.contains(&waiting_candidate), Error::<T>::WaitingCandidateAlreadyExist);
				waiting_candidates.try_push(waiting_candidate.clone()).map_err(|_| Error::<T>::WaitingCandidateMaxExceeded)?;
				Self::deposit_event(Event::WaitingCandidateAdded { _waiting_candidate: waiting_candidate });
				Ok(())
			})
		}

		/// Remove a waiting candidate
		/// Note:	
		/// 	This is called when the candidate wants to leave.
		pub fn remove_waiting_candidate(waiting_candidate: T::AccountId) -> DispatchResult {
			WaitingCandidates::<T>::try_mutate(|waiting_candidates| -> DispatchResult {
				ensure!(waiting_candidates.contains(&waiting_candidate), Error::<T>::WaitingCandidateNotFound);
				if let Some(pos) = waiting_candidates.iter().position(|x| x == &waiting_candidate) {
					waiting_candidates.remove(pos);
				} else {
					return Err(Error::<T>::WaitingCandidateNotFound.into());
				}
				Self::deposit_event(Event::WaitingCandidateRemoved { _waiting_candidate: waiting_candidate });
				Ok(())
			})
		}

		/// Sort proposed candidates:
		/// Note:	
		/// 	Prioritize total_stake first, then bond, then oldest last_updated
		pub fn sort_proposed_candidates() {
            let mut proposed_candidates = ProposedCandidates::<T>::get();
            proposed_candidates.sort_by(|a, b| {
                b.total_stake.cmp(&a.total_stake)
                    .then_with(|| b.bond.cmp(&a.bond))
                    .then_with(|| a.last_updated.cmp(&b.last_updated)) 
            });
            ProposedCandidates::<T>::put(proposed_candidates);
        }

		/// Add Xaver Nodes to the desired candidates at genesis
		pub fn add_xaver_nodes() {
			for xaver_node in T::XaverNodes::get() {
				let xaver_node = if xaver_node.starts_with("0x") { &xaver_node[2..] } else { xaver_node };
				// Panic if runtime not properly configured
				let decoded_bytes = decode(xaver_node).expect("Invalid hex string");
				// Panic if runtime not properly configured
				let authority = T::AuthorityId::decode(&mut decoded_bytes.as_slice()).expect("Error in decoding");
				let account = Self::authority_to_account(authority);
				let _ = Self::add_desired_candidate(account.clone());
				let _ = Self::add_waiting_candidate(account);
			}
		}

		/// Prepare waiting candidates
		/// Note:
		/// 	This function is trigerred every new session.  Waiting candidates is just a storage that
		/// 	merge the desired candidates and the proposed candidates.
		pub fn prepare_waiting_candidates() -> DispatchResult {
			let desired_candidates = DesiredCandidates::<T>::get();
			let proposed_candidates = ProposedCandidates::<T>::get();
			let mut waiting_candidates: BoundedVec<T::AccountId, T::MaxCandidates> = BoundedVec::default();
			// First, add all desired candidates
			for candidate in desired_candidates.iter() {
				if waiting_candidates.len() < T::MaxCandidates::get() as usize {
					waiting_candidates.try_push(candidate.clone()).map_err(|_| Error::<T>::WaitingCandidateAlreadyExist)?;
				}
			}
			// Next, add proposed candidates if there's still space, bond is not zero and not leaving
			for proposed_candidate in proposed_candidates.iter() {
				if waiting_candidates.len() < T::MaxCandidates::get() as usize {
					if !waiting_candidates.contains(&proposed_candidate.who) && 
					   !proposed_candidate.bond.is_zero() && 
					   !proposed_candidate.leaving && 
					   !proposed_candidate.offline {
						waiting_candidates.try_push(proposed_candidate.who.clone()).map_err(|_| Error::<T>::WaitingCandidateAlreadyExist)?;
					}
				}
			}
			WaitingCandidates::<T>::put(waiting_candidates);
			Ok(())
		}

		/// Clean proposed candidate storage from leaving or zero bonded candidates
		/// Note:
		/// 	We need to execute this helper function to make sure that we have space for others to join
		pub fn clean_proposed_candidates() -> DispatchResult {
			let proposed_candidates = ProposedCandidates::<T>::get();

			for proposed_candidate in proposed_candidates.iter() {
				if proposed_candidate.leaving {
					if !WaitingCandidates::<T>::get().contains(&proposed_candidate.who) &&
					   !pallet_collator_selection::Invulnerables::<T>::get().contains(&proposed_candidate.who) {
						if proposed_candidate.bond > Zero::zero() {
							T::StakingCurrency::unreserve(&proposed_candidate.who, proposed_candidate.bond);
						}
						let _ = Self::remove_proposed_candidate(proposed_candidate.who.clone());
					}
				} else {
					if proposed_candidate.bond == Zero::zero() {
						if !WaitingCandidates::<T>::get().contains(&proposed_candidate.who) &&
						   !pallet_collator_selection::Invulnerables::<T>::get().contains(&proposed_candidate.who) {
						 	let _ = Self::remove_proposed_candidate(proposed_candidate.who.clone());
						}
					}
				}
			}
			Ok(())
		}

		/// Compute total_stake in the candidate information
		/// Note:
		/// 	Re-compute the total stake and called every staking extrinsics.
		/// 	Once the total is completed immediately sort the proposed candidates.
		pub fn total_stake_proposed_candidate(proposed_candidate: T::AccountId) -> DispatchResult {
			let mut proposed_candidates = ProposedCandidates::<T>::get();
			let mut found = false;
			for i in 0..proposed_candidates.len() {
				if proposed_candidates[i].who == proposed_candidate {
					let total_stake = if let Some(delegations) = <Delegations<T>>::get(proposed_candidate.clone()) {
						delegations.iter().fold(BalanceOf::<T>::default(), |acc, delegation| acc + delegation.stake)
					} else {
						BalanceOf::<T>::default()
					};
					proposed_candidates[i].total_stake = total_stake;
					proposed_candidates[i].last_updated = frame_system::Pallet::<T>::block_number();
					found = true;
					break;
				}
			}
			ensure!(found, Error::<T>::ProposedCandidateNotFound);
			ProposedCandidates::<T>::put(proposed_candidates);
			Self::sort_proposed_candidates();
			Self::deposit_event(Event::ProposedCandidateTotalStake { _proposed_candidate: proposed_candidate });			
			Ok(())
		}


		/// Assemble the final collator nodes by updating the pallet_collator_selection invulnerables.
		/// Note:
		/// 	This helper function is called every new session.
		pub fn assemble_collators() -> DispatchResult {
			// Ensure the waiting candidates storage is not empty
			let waiting_candidates = WaitingCandidates::<T>::get();
			ensure!(!waiting_candidates.is_empty(), Error::<T>::WaitingCandidatesEmpty);

			// Remove the invulnerables not in the waiting candidates to save space
			let mut invulnerables = pallet_collator_selection::Invulnerables::<T>::get();
			invulnerables.retain(|account| waiting_candidates.contains(account));
			pallet_collator_selection::Invulnerables::<T>::put(invulnerables);

			// Start inserting the waiting candidates to invulnerables
			for waiting_candidate in waiting_candidates.clone() {
				let _ = Self::add_invulnerable(waiting_candidate);
			}

			// Clean proposed candidate storage from leaving candidates and zero bond
			let _ = Self::clean_proposed_candidates();

			// Prepare the new waiting candidates for the next session
			let _ = Self::prepare_waiting_candidates();

			Ok(())
		}
	}

	/// ===============
	/// Session Manager
	/// ===============
	impl<T: Config> SessionManager<T::AccountId> for Pallet<T> {
		fn new_session(_index: SessionIndex) -> Option<Vec<T::AccountId>> {
			let _ = Self::assemble_collators();
			let  collators = pallet_collator_selection::Invulnerables::<T>::get().to_vec();
			Some(collators)
		}
		fn start_session(_: SessionIndex) {
			// Todo, may not do anything
		}
		fn end_session(_: SessionIndex) {
			// Todo, may not do anything
		}
	}

}
