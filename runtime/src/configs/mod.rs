// This is free and unencumbered software released into the public domain.
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// For more information, please refer to <http://unlicense.org>

mod xcm_config;

// Substrate and Polkadot dependencies
use crate::Timestamp;
use cumulus_pallet_parachain_system::RelayNumberMonotonicallyIncreases;
use cumulus_primitives_core::{AggregateMessageOrigin, ParaId};
use frame_support::{
	derive_impl,
	dispatch::DispatchClass,
	parameter_types,
	ord_parameter_types,
	traits::{
		ConstBool, ConstU32, ConstU64, ConstU8, EitherOfDiverse, TransformOrigin, VariantCountOf,
		AsEnsureOriginWithArg,Randomness,
		fungible::{Balanced, Credit},
		OnUnbalanced,Imbalance,
		tokens::imbalance::ResolveTo,
		EnsureOrigin
	},
	weights::{ConstantMultiplier, Weight},
	PalletId,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot,EnsureSigned,pallet_prelude::BlockNumberFor,
	EnsureWithSuccess,
};
use pallet_xcm::{EnsureXcm, IsVoiceOfBody};
use parachains_common::message_queue::{NarrowOriginToSibling, ParaIdToSibling};
use polkadot_runtime_common::{
	xcm_sender::NoPriceForMessageDelivery, BlockHashCount, SlowAdjustingFeeUpdate,
};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_runtime::Perbill;
use sp_runtime::Percent;
use sp_runtime::traits::AccountIdConversion;
use sp_version::RuntimeVersion;
use xcm::latest::prelude::BodyId;
use sp_runtime::Saturating;
use sp_runtime::traits::Zero;


// Local module imports
use super::{
	weights::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
	AccountId, Aura, Balance, Balances, Block, BlockNumber, CollatorSelection, ConsensusHook, Hash,
	MessageQueue, Nonce, PalletInfo, ParachainSystem, Runtime, RuntimeCall, RuntimeEvent,
	RuntimeFreezeReason, RuntimeHoldReason, RuntimeOrigin, RuntimeTask, Session, SessionKeys,
	System, WeightToFee, XcmpQueue, AVERAGE_ON_INITIALIZE_RATIO, EXISTENTIAL_DEPOSIT, DAYS, HOURS, MINUTES,
	MAXIMUM_BLOCK_WEIGHT, MICRO_UNIT, NORMAL_DISPATCH_RATIO, SLOT_DURATION, VERSION,
	// Governance
	TechnicalCommittee, TreasuryCouncil,
};
use xcm_config::{RelayLocation, XcmOriginToTransactDispatchOrigin};

use pallet_balances::Call as BalancesCall;
pub enum AllowBalancesCall {}
impl frame_support::traits::Contains<RuntimeCall> for AllowBalancesCall {
    fn contains(call: &RuntimeCall) -> bool {
        matches!(
            call,
            RuntimeCall::Balances(BalancesCall::transfer_allow_death { .. })
        )
    }
}

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;

	// This part is copied from Substrate's `bin/node/runtime/src/lib.rs`.
	//  The `RuntimeBlockLength` and `RuntimeBlockWeights` exist here because the
	// `DeletionWeightLimit` and `DeletionQueueDepth` depend on those to parameterize
	// the lazy contract deletion.
	pub RuntimeBlockLength: BlockLength =
		BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have some extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub const SS58Prefix: u16 = 42;
}

/// The default types are being injected by [`derive_impl`](`frame_support::derive_impl`) from
/// [`ParaChainDefaultConfig`](`struct@frame_system::config_preludes::ParaChainDefaultConfig`),
/// but overridden as needed.
#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig)]
impl frame_system::Config for Runtime {
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The index type for storing how many extrinsics an account has signed.
	type Nonce = Nonce;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The block type.
	type Block = Block;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// Runtime version.
	type Version = Version;
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = RuntimeBlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = RuntimeBlockLength;
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	/// The action to take on a Runtime Upgrade
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Aura;
	type MinimumPeriod = ConstU64<0>;
	type WeightInfo = ();
}

impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
	type EventHandler = (CollatorSelection,);
}

parameter_types! {
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = ConstU32<50>;
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = RuntimeFreezeReason;
	type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
}

pub struct ToAuthor<R>(core::marker::PhantomData<R>);
impl<R> OnUnbalanced<Credit<R::AccountId, pallet_balances::Pallet<R>>> for ToAuthor<R>
where
    R: pallet_balances::Config + pallet_authorship::Config + pallet_xode_staking::Config,
    <R as frame_system::Config>::AccountId: From<AccountId>,
    <R as frame_system::Config>::AccountId: Into<AccountId>,
{
    fn on_nonzero_unbalanced(
        amount: Credit<<R as frame_system::Config>::AccountId, pallet_balances::Pallet<R>>,
    ) {
        if let Some(author) = <pallet_authorship::Pallet<R>>::author() {
			if let Some(candidate) = pallet_xode_staking::ProposedCandidates::<R>::get().iter().find(|c| c.who == author) {
				if let Some(delegations) = pallet_xode_staking::Delegations::<R>::get(&author) {
					let commission = Percent::from_percent(candidate.commission.into());
					let share = amount.peek();
					let mut remaining_imbalance = amount;
					for (_index, delegation) in delegations.iter().enumerate() {
						if delegation.stake > Zero::zero() {
							let ratio = Percent::from_rational(delegation.stake, candidate.total_stake);
							let delegator_share = share.saturating_mul(ratio.deconstruct().into()).saturating_mul(commission.deconstruct().into());
							let (imbalance_share, leftover) = remaining_imbalance.split(delegator_share.into());
							remaining_imbalance = leftover; 
							// For the delegator
							let _ = <pallet_balances::Pallet<R>>::resolve(&delegation.delegator,imbalance_share,);
						}
					}
					// For the Author
					if remaining_imbalance.peek() > Zero::zero() {
						let _ = <pallet_balances::Pallet<R>>::resolve(&author, remaining_imbalance);
					}
				} 
			}
        }
    }
}

pub const TREASURY_SHARE: u32 = 20;
pub const AUTHOR_SHARE: u32 = 80;

pub struct DealWithFees<R>(core::marker::PhantomData<R>);
impl<R> OnUnbalanced<Credit<R::AccountId, pallet_balances::Pallet<R>>> for DealWithFees<R>
where
	R: pallet_balances::Config + pallet_authorship::Config + pallet_treasury::Config + pallet_xode_staking::Config,
    <R as frame_system::Config>::AccountId: From<AccountId>,
    <R as frame_system::Config>::AccountId: Into<AccountId>,
{
	fn on_unbalanceds(
		mut fees_then_tips: impl Iterator<Item = Credit<R::AccountId, pallet_balances::Pallet<R>>>,
	) {
		if let Some(fees) = fees_then_tips.next() {
			let mut split = fees.ration(TREASURY_SHARE, AUTHOR_SHARE);
			if let Some(tips) = fees_then_tips.next() {
				tips.merge_into(&mut split.1);
			}
			ResolveTo::<pallet_treasury::TreasuryAccountId<R>, pallet_balances::Pallet<R>>::on_unbalanced(split.0);
			<ToAuthor<R> as OnUnbalanced<_>>::on_unbalanced(split.1);
		}
	}
}

parameter_types! {
	/// Relay Chain `TransactionByteFee` / 10
	pub const TransactionByteFee: Balance = 10 * MICRO_UNIT;
}

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, DealWithFees<Runtime>>;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type OperationalFeeMultiplier = ConstU8<5>;
}

parameter_types! {
	pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const RelayOrigin: AggregateMessageOrigin = AggregateMessageOrigin::Parent;
}

impl cumulus_pallet_parachain_system::Config for Runtime {
	type WeightInfo = ();
	type RuntimeEvent = RuntimeEvent;
	type OnSystemEvent = ();
	type SelfParaId = parachain_info::Pallet<Runtime>;
	type OutboundXcmpMessageSource = XcmpQueue;
	type DmpQueue = frame_support::traits::EnqueueWithOrigin<MessageQueue, RelayOrigin>;
	type ReservedDmpWeight = ReservedDmpWeight;
	type XcmpMessageHandler = XcmpQueue;
	type ReservedXcmpWeight = ReservedXcmpWeight;
	type CheckAssociatedRelayNumber = RelayNumberMonotonicallyIncreases;
	type ConsensusHook = ConsensusHook;
}

impl parachain_info::Config for Runtime {}

parameter_types! {
	pub MessageQueueServiceWeight: Weight = Perbill::from_percent(35) * RuntimeBlockWeights::get().max_block;
}

impl pallet_message_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type MessageProcessor = pallet_message_queue::mock_helpers::NoopMessageProcessor<
		cumulus_primitives_core::AggregateMessageOrigin,
	>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type MessageProcessor = xcm_builder::ProcessXcmMessage<
		AggregateMessageOrigin,
		xcm_executor::XcmExecutor<xcm_config::XcmConfig>,
		RuntimeCall,
	>;
	type Size = u32;
	// The XCMP queue pallet is only ever able to handle the `Sibling(ParaId)` origin:
	type QueueChangeHandler = NarrowOriginToSibling<XcmpQueue>;
	type QueuePausedQuery = NarrowOriginToSibling<XcmpQueue>;
	type HeapSize = sp_core::ConstU32<{ 103 * 1024 }>;
	type MaxStale = sp_core::ConstU32<8>;
	type ServiceWeight = MessageQueueServiceWeight;
	type IdleMaxServiceWeight = ();
}

impl cumulus_pallet_aura_ext::Config for Runtime {}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = ();
	// Enqueue XCMP messages from siblings for later processing.
	type XcmpQueue = TransformOrigin<MessageQueue, AggregateMessageOrigin, ParaId, ParaIdToSibling>;
	type MaxInboundSuspended = sp_core::ConstU32<1_000>;
	type MaxActiveOutboundChannels = ConstU32<128>;
	type MaxPageSize = ConstU32<{ 1 << 16 }>;
	type ControllerOrigin = EnsureTwoThirdsTechnicalCouncil;
	type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
	type WeightInfo = ();
	type PriceForSiblingDelivery = NoPriceForMessageDelivery<ParaId>;
}

parameter_types! {
	//pub const Period: u32 = 6 * HOURS;
	pub const Period: u32 = MINUTES;
	pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	// type SessionManager = CollatorSelection;
	type SessionManager = crate::XodeStaking;
	// Essentially just Aura, but let's be pedantic.
	type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type WeightInfo = ();
}

#[docify::export(aura_config)]
impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = ConstU32<1_000>;
	type AllowMultipleBlocksPerSlot = ConstBool<true>;
	type SlotDuration = ConstU64<SLOT_DURATION>;
}

parameter_types! {
	pub const PotId: PalletId = PalletId(*b"PotStake");
	// pub const SessionLength: BlockNumber = 6 * HOURS;
	pub const SessionLength: BlockNumber = 6 * MINUTES;
	// StakingAdmin pluralistic body.
	pub const StakingAdminBodyId: BodyId = BodyId::Defense;
}

/// We allow root and the StakingAdmin to execute privileged collator selection operations.  It needs now technical council approval (sudo removed)
pub type CollatorSelectionUpdateOrigin = EitherOfDiverse<
	EnsureTwoThirdsTechnicalCouncil,
	EnsureXcm<IsVoiceOfBody<RelayLocation, StakingAdminBodyId>>,
>;

impl pallet_collator_selection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type UpdateOrigin = CollatorSelectionUpdateOrigin;
	type PotId = PotId;
	type MaxCandidates = ConstU32<100>;
	type MinEligibleCollators = ConstU32<4>;
	type MaxInvulnerables = ConstU32<20>;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = Period;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = Session;
	type WeightInfo = ();
}

/// ======
/// Assets
/// ======
pub const ASSETS_UNIT: Balance = 1_000_000_000_000;
pub const ASSETS_MILLIUNIT: Balance = 1_000_000_000;
pub const ASSETS_MICROUNIT: Balance = 1_000_000;
pub const ASSETS_EXISTENTIAL_DEPOSIT: Balance = ASSETS_MILLIUNIT;

pub const fn deposit(items: u32, bytes: u32) -> Balance {
	(items as Balance * 20 * ASSETS_UNIT + (bytes as Balance) * 100 * ASSETS_MICROUNIT) / 100
}

parameter_types! {
	pub const AssetDeposit: Balance = 10 * ASSETS_UNIT;
	pub const AssetAccountDeposit: Balance = deposit(1, 16);
	pub const ApprovalDeposit: Balance = ASSETS_EXISTENTIAL_DEPOSIT;
	pub const StringLimit: u32 = 50;
	pub const MetadataDepositBase: Balance = deposit(1, 68);
	pub const MetadataDepositPerByte: Balance = deposit(0, 1);
}

impl pallet_assets::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type RemoveItemsLimit = ConstU32<1_000>;
	type AssetId = u32;
	type AssetIdParameter = codec::Compact<u32>;
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type ForceOrigin = EnsureTwoThirdsTreasuryCouncil;
	type AssetDeposit = AssetDeposit;
	type AssetAccountDeposit = AssetAccountDeposit;
	type MetadataDepositBase = MetadataDepositBase;
	type MetadataDepositPerByte = MetadataDepositPerByte;
	type ApprovalDeposit = ApprovalDeposit;
	type StringLimit = StringLimit;
	type Freezer = ();
	type Extra = ();
	type CallbackHandle = ();
	type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

/// =========
/// Contracts
/// =========
pub struct DummyRandomness<T: pallet_contracts::Config>(sp_std::marker::PhantomData<T>);

impl<T: pallet_contracts::Config> Randomness<T::Hash, BlockNumberFor<T>> for DummyRandomness<T> {
    fn random(_subject: &[u8]) -> (T::Hash, BlockNumberFor<T>) {
        (Default::default(), Default::default())
    }
}

fn schedule<T: pallet_contracts::Config>() -> pallet_contracts::Schedule<T> {
    pallet_contracts::Schedule {
        limits: pallet_contracts::Limits {
            runtime_memory: 1024 * 1024 * 1024,
            ..Default::default()
        },
        ..Default::default()
    }
}

parameter_types! {
    pub const DepositPerItem: Balance = deposit(1, 0);
    pub const DepositPerByte: Balance = deposit(0, 1);
    pub Schedule: pallet_contracts::Schedule<Runtime> = schedule::<Runtime>();
    pub const DefaultDepositLimit: Balance = deposit(1024, 1024 * 1024);
    pub const CodeHashLockupDepositPercent: Perbill = Perbill::from_percent(0);
    pub const MaxDelegateDependencies: u32 = 32;
}

impl pallet_contracts::Config for Runtime {
    type Time = Timestamp;
    type Randomness = DummyRandomness<Self>;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;

    /// The safest default is to allow no calls at all.
    ///
    /// Runtimes should whitelist dispatchables that are allowed to be called from contracts
    /// and make sure they are stable. Dispatchables exposed to contracts are not allowed to
    /// change because that would break already deployed contracts. The `RuntimeCall` structure
    /// itself is not allowed to change the indices of existing pallets, too.
    type CallFilter = AllowBalancesCall;
    type DepositPerItem = DepositPerItem;
    type DepositPerByte = DepositPerByte;
    type CallStack = [pallet_contracts::Frame<Self>; 23];
    type WeightPrice = pallet_transaction_payment::Pallet<Self>;
    type WeightInfo = pallet_contracts::weights::SubstrateWeight<Self>;
    type ChainExtension = ();
    type Schedule = Schedule;
    type AddressGenerator = pallet_contracts::DefaultAddressGenerator;
    // This node is geared towards development and testing of contracts.
    // We decided to increase the default allowed contract size for this
    // reason (the default is `128 * 1024`).
    //
    // Our reasoning is that the error code `CodeTooLarge` is thrown
    // if a too-large contract is uploaded. We noticed that it poses
    // less friction during development when the requirement here is
    // just more lax.
    type MaxCodeLen = ConstU32<{ 256 * 1024 }>;
    type DefaultDepositLimit = DefaultDepositLimit;
    type MaxStorageKeyLen = ConstU32<128>;
    type MaxDebugBufferLen = ConstU32<{ 2 * 1024 * 1024 }>;
    type UnsafeUnstableInterface = ConstBool<true>;
    type CodeHashLockupDepositPercent = CodeHashLockupDepositPercent;
    type MaxDelegateDependencies = MaxDelegateDependencies;
    type RuntimeHoldReason = RuntimeHoldReason;

    type Environment = ();
    type Debug = ();
    type ApiVersion = ();
    type Migrations = ();
    type Xcm = pallet_xcm::Pallet<Self>;
    type UploadOrigin = EnsureSigned<Self::AccountId>;
    type InstantiateOrigin = EnsureSigned<Self::AccountId>;

	type MaxTransientStorageSize = ConstU32<{ 1 * 1024 * 1024 }>;
}

/// ========
/// Treasury  
/// ========
pub const MILLICENTS: Balance = 1_000_000_000;
pub const CENTS: Balance = 1_000 * MILLICENTS; 
pub const DOLLARS: Balance = 100 * CENTS;

parameter_types! {
	pub const IndexDeposit: Balance = 1 * DOLLARS;
}

impl pallet_indices::Config for Runtime {
	type AccountIndex = u32;
	type Currency = Balances;
	type Deposit = IndexDeposit;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_indices::weights::SubstrateWeight<Runtime>;
}

impl pallet_asset_rate::Config for Runtime {
	type CreateOrigin = EnsureTwoThirdsTreasuryCouncil;
	type RemoveOrigin = EnsureTwoThirdsTreasuryCouncil;
	type UpdateOrigin = EnsureTwoThirdsTreasuryCouncil;
	type Currency = Balances;
	type AssetKind = u32;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_asset_rate::weights::SubstrateWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

parameter_types! {
    pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	// pub const SpendPeriod: BlockNumber = 1 * DAYS;
	pub const SpendPeriod: BlockNumber = 1 * MINUTES; // Testing purpose only
	pub const MaxApprovals: u32 = 100;
	pub const MaxBalance: Balance = Balance::max_value();
	pub XodeTreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
	// pub const SpendPayoutPeriod: BlockNumber = 30 * DAYS;
	pub const SpendPayoutPeriod: BlockNumber = 30 * MINUTES; // Testing purpose only
}

impl pallet_treasury::Config for Runtime {
    type PalletId = TreasuryPalletId; 
    type Currency = Balances;        
    type RejectOrigin = EnsureTwoThirdsTreasuryCouncil;  
	type SpendOrigin = EnsureWithSuccess<EnsureTwoThirdsTreasuryCouncil, AccountId, MaxBalance>; 
	type RuntimeEvent = RuntimeEvent; 
	type SpendPeriod = SpendPeriod;
    type Burn = ();                  
    type BurnDestination = ();
	type SpendFunds = ();  
    type WeightInfo = pallet_treasury::weights::SubstrateWeight<Runtime>;
    type MaxApprovals = MaxApprovals;
	type AssetKind = u32;
	type Beneficiary = AccountId;
	type BeneficiaryLookup = pallet_indices::Pallet<Runtime>;
	type Paymaster = frame_support::traits::tokens::pay::PayAssetFromAccount<pallet_assets::Pallet<Runtime>, XodeTreasuryAccount>;
	type BalanceConverter = pallet_asset_rate::Pallet<Runtime>;
	type PayoutPeriod = SpendPayoutPeriod;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

/// ==========
/// Governance
/// ==========
use pallet_collective::{EnsureMember, EnsureProportionAtLeast, EnsureProportionMoreThan};

pub type TechnicalCouncilInstance = pallet_collective::Instance1;
pub type TreasuryCouncilInstance = pallet_collective::Instance2;

pub type EnsureTwoThirdsTechnicalCouncil = EnsureProportionMoreThan<AccountId, TechnicalCouncilInstance, 2, 3>;
pub type EnsureAllTechnicalCouncil = EnsureProportionAtLeast<AccountId, TechnicalCouncilInstance, 1, 1>; // Adding members everyone must agree
pub type EnsureTwoThirdsTreasuryCouncil = EnsureProportionMoreThan<AccountId, TreasuryCouncilInstance, 2, 3>;
pub type EnsureAllTreasuryCouncil = EnsureProportionAtLeast<AccountId, TreasuryCouncilInstance, 1, 1>; // Adding members everyone must agree

parameter_types! {
    // pub const TecnicalCouncilMotionDuration: BlockNumber = 5 * DAYS;
	pub const TecnicalCouncilMotionDuration: BlockNumber = 1 * MINUTES; // For testing purpose only
    pub const TecnicalCouncilMaxProposals: u32 = 100;
    pub const TecnicalCouncilMaxMembers: u32 = 100;
	pub TechnicalMaxProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
}

impl pallet_collective::Config<TechnicalCouncilInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = TecnicalCouncilMotionDuration;
	type MaxProposals = TecnicalCouncilMaxProposals;
	type MaxMembers = TecnicalCouncilMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type SetMembersOrigin = EnsureAllTechnicalCouncil;
	type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
	type MaxProposalWeight = TechnicalMaxProposalWeight;
}

parameter_types! {
	pub const TechnicalMembershipMaxMembers: u32 = 100;
}

impl pallet_membership::Config<TechnicalCouncilInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = EnsureTwoThirdsTechnicalCouncil;
	type RemoveOrigin = EnsureAllTechnicalCouncil;
	type SwapOrigin = EnsureAllTechnicalCouncil;
	type ResetOrigin = EnsureAllTechnicalCouncil;
	type PrimeOrigin = EnsureAllTechnicalCouncil;
	type MembershipInitialized = TechnicalCommittee;
	type MembershipChanged = TechnicalCommittee;
	type MaxMembers = TechnicalMembershipMaxMembers;
	type WeightInfo = pallet_membership::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    // pub const TreasuryCouncilMotionDuration: BlockNumber = 5 * DAYS;
	pub const TreasuryCouncilMotionDuration: BlockNumber = 1 * MINUTES; // For testing purpose only
    pub const TreasuryCouncilMaxProposals: u32 = 100;
    pub const TreasuryCouncilMaxMembers: u32 = 100;
	pub TreasuryMaxProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
}

impl pallet_collective::Config<TreasuryCouncilInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = TreasuryCouncilMotionDuration;
	type MaxProposals = TreasuryCouncilMaxProposals;
	type MaxMembers = TreasuryCouncilMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type SetMembersOrigin = EnsureAllTreasuryCouncil;
	type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
	type MaxProposalWeight = TreasuryMaxProposalWeight;
}

parameter_types! {
	pub const TreasuryMembershipMaxMembers: u32 = 100;
}

impl pallet_membership::Config<TreasuryCouncilInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = EnsureTwoThirdsTreasuryCouncil;
	type RemoveOrigin = EnsureAllTreasuryCouncil;
	type SwapOrigin = EnsureAllTreasuryCouncil;
	type ResetOrigin = EnsureAllTreasuryCouncil;
	type PrimeOrigin = EnsureAllTreasuryCouncil;
	type MembershipInitialized = TreasuryCouncil;
	type MembershipChanged = TreasuryCouncil;
	type MaxMembers = TreasuryMembershipMaxMembers;
	type WeightInfo = pallet_membership::weights::SubstrateWeight<Runtime>;
}

/// ==============
/// Staking (Xode)
/// ==============
parameter_types! {
	pub const Nodes: &'static [&'static str] = &[
		"0x306721211d5404bd9da88e0204360a1a9ab8b87c66c1bc2fcdd37f3c2222cc20",  	// Charlie (Use for development)
		"0x90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22", 	// Dave (Use for development)
		"0xe659a7a1628cdd93febc04a4e0646ea20e9f5f0ce097d9a05290d4a9e054df4e",   // Eve (Use for development)
	];
}

impl pallet_xode_staking::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_xode_staking::weights::SubstrateWeight<Runtime>;
	// Total number collator candidate - Total number of Xaver Nodes = Remaining slot for the proposed candidates
	type MaxProposedCandidates = ConstU32<200>;  
	type MaxProposedCandidateDelegates = ConstU32<200>;  
	type XaverNodes = Nodes;
	type StakingCurrency = Balances;
}

