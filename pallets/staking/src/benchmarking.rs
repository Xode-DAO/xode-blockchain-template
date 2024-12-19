//! Benchmarking setup for pallet-template
#![cfg(feature = "runtime-benchmarks")]

use super::{Pallet as XodeStaking, *};
use frame_benchmarking::v2::*;
use crate::pallet::BalanceOf;

#[benchmarks]
mod benchmarks {
  use super::*;
  #[cfg(test)]
  use crate::pallet::Pallet as XodeStaking;
  use sp_runtime::traits::Zero;
  use frame_system::RawOrigin;
  use frame_support:: {
    traits::Currency,
    assert_ok
  };
 
  pub(super) fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
  }

  pub(super) fn initial_config<T: Config>() {
    let candidate_account: T::AccountId = account("candidate", 0, 0);
    ProposedCandidates::<T>::try_mutate(|candidates| {
      candidates.try_push(CandidateInfo {
        who: candidate_account,
        bond: Zero::zero(),
        total_stake: Zero::zero(),
        last_updated: frame_system::Pallet::<T>::block_number(),
        leaving: false,
        offline: false,
        commission: 0,
      })
    }).expect("Failed to add initial candidate");
  }

	pub(super) fn set_free_balance<T: Config>(receiver: T::AccountId) -> BalanceOf<T> {
		let balance_to_fund: BalanceOf<T> = create_balance::<T>();
    T::StakingCurrency::make_free_balance_be(&receiver, balance_to_fund * 2u32.into());
		return balance_to_fund;
	}

	pub(super) fn create_balance<T: Config>() -> BalanceOf<T> {
		return T::StakingCurrency::minimum_balance() * 10u32.into();
	}

  #[benchmark]
  fn register_candidate() {
    let caller: T::AccountId = whitelisted_caller();

    #[extrinsic_call]
    _(RawOrigin::Signed(caller.clone()));

    assert_last_event::<T>(
      Event::<T>::ProposedCandidateAdded {
        _proposed_candidate: caller.clone(),
      }
      .into(),
    );
  }

  #[benchmark]
  fn bond_candidate() {
    initial_config::<T>();

    let caller: T::AccountId = whitelisted_caller();
    set_free_balance::<T>(caller.clone());

    assert_ok!(XodeStaking::<T>::register_candidate(
      RawOrigin::Signed(caller.clone()).into()
    ));

    let new_bond: BalanceOf<T> = create_balance::<T>();

    #[extrinsic_call]
    _(RawOrigin::Signed(caller.clone()), new_bond);

    assert_last_event::<T>(
      Event::<T>::ProposedCandidateBonded {
        _proposed_candidate: caller.clone(),
      }
      .into(),
    );
  }

  #[benchmark]
  fn set_commission_of_candidate() {
    initial_config::<T>();

    let caller: T::AccountId = whitelisted_caller();
    set_free_balance::<T>(caller.clone());

    assert_ok!(XodeStaking::<T>::register_candidate(
      RawOrigin::Signed(caller.clone()).into()
    ));

    let commission: u8 = 10;

    #[extrinsic_call]
    _(RawOrigin::Signed(caller.clone()), commission);

    assert_last_event::<T>(
      Event::<T>::ProposedCandidateCommissionSet {
        _proposed_candidate: caller.clone(),
      }
      .into(),
    );
  }

  #[benchmark]
  fn leave_candidate() {
    initial_config::<T>();

    let caller: T::AccountId = whitelisted_caller();
    set_free_balance::<T>(caller.clone());

    assert_ok!(XodeStaking::<T>::register_candidate(
      RawOrigin::Signed(caller.clone()).into()
    ));

    assert_ok!(XodeStaking::<T>::add_waiting_candidate(
      caller.clone()
    ));

    // TODO: No event being emitted after adding waiting candidate!
    // let emitted_event = frame_system::Pallet::<T>::events()
    //  .last()
    //  .map(|e| e.event.clone());
    // log::info!("Last emitted event: {:?}", emitted_event);
    // assert_last_event::<T>(
    //  Event::<T>::WaitingCandidateAdded {
    //    _waiting_candidate: caller.clone(),
    //  }
    //  .into(),
    // );

    assert!(WaitingCandidates::<T>::get().contains(&caller));

    #[extrinsic_call]
    _(RawOrigin::Signed(caller.clone()));

    assert_last_event::<T>(
      Event::<T>::ProposedCandidateLeft {
        _proposed_candidate: caller.clone(),
      }
      .into(),
    );
  }


  #[benchmark]
  fn stake_candidate() {
    initial_config::<T>();

    let delegator: T::AccountId = whitelisted_caller();
    let candidate: T::AccountId = account("candidate", 0, 0);

    let amount_to_stake: BalanceOf<T> = set_free_balance::<T>(delegator.clone());

    assert!(ProposedCandidates::<T>::get().iter().any(|c| c.who == candidate));

    #[extrinsic_call]
    _(RawOrigin::Signed(delegator.clone()), candidate.clone(), amount_to_stake);

    assert_last_event::<T>(
      Event::<T>::DelegationAdded {
          _delegator: delegator.clone(),
      }
      .into(),
    );
  }

  #[benchmark]
  fn unstake_candidate() {
    initial_config::<T>();

    let delegator: T::AccountId = whitelisted_caller();
    let candidate: T::AccountId = account("candidate", 1, 0);

    let amount_to_stake: BalanceOf<T> = set_free_balance::<T>(delegator.clone());

    assert_ok!(XodeStaking::<T>::register_candidate(
      RawOrigin::Signed(candidate.clone()).into()
    ));

    assert_ok!(XodeStaking::<T>::stake_candidate(
      RawOrigin::Signed(delegator.clone()).into(),
      candidate.clone(),
      amount_to_stake
    ));

    let delegations = Delegations::<T>::get(&candidate).unwrap();
    assert!(delegations.iter().any(|d| d.delegator == delegator && d.stake == amount_to_stake));

    #[extrinsic_call]
    _(RawOrigin::Signed(delegator.clone()), candidate.clone());

    assert_last_event::<T>(
      Event::<T>::DelegationRevoked {
        _delegator: delegator.clone(),
      }
      .into(),
    );
  }

	#[benchmark]
	fn offline_candidate() {
		initial_config::<T>();

		let candidate: T::AccountId = account("candidate", 0, 0);

		set_free_balance::<T>(candidate.clone());

		assert!(ProposedCandidates::<T>::get()
			.iter()
			.any(|c| c.who == candidate));

		#[extrinsic_call]
		_(RawOrigin::Signed(candidate.clone()));

		assert_last_event::<T>(
			Event::<T>::ProposedCandidateOffline {
				_proposed_candidate: candidate,
			}
			.into(),
		);
	}

	#[benchmark]
	fn online_candidate() {
		initial_config::<T>();

		let candidate: T::AccountId = account("candidate", 0, 0);

		set_free_balance::<T>(candidate.clone());

		assert!(ProposedCandidates::<T>::get()
			.iter()
			.any(|c| c.who == candidate));

		#[extrinsic_call]
		_(RawOrigin::Signed(candidate.clone()));

		assert_last_event::<T>(
			Event::<T>::ProposedCandidateOnline {
				_proposed_candidate: candidate,
			}
			.into(),
		);
	}

  impl_benchmark_test_suite!(XodeStaking, crate::mock::new_test_ext(), crate::mock::Test);
}
