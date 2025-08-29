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

// Local module imports
use super::{
    AccountId, AssetRate, Assets, Balance, Balances, Block, BlockNumber, DAYS, EXISTENTIAL_DEPOSIT,
    HOURS, Hash, Nonce, OriginCaller, PalletInfo, Preimage, Runtime, RuntimeCall, RuntimeEvent,
    RuntimeFreezeReason, RuntimeHoldReason, RuntimeOrigin, RuntimeTask, SLOT_DURATION, Scheduler,
    System, Treasury, VERSION, XOR,
};
// Substrate and Polkadot dependencies
use crate::{
    Babe, Bounties, CENTS, ChildBounties, DelegatedStaking, EPOCH_DURATION_IN_SLOTS,
    ElectionProviderMultiPhase, Historical, MILLI_SECS_PER_BLOCK, MINUTES, Moment, NominationPools,
    NposCompactSolution16, Offences, OnChainAccuracy, RandomnessCollectiveFlip, Session,
    SessionKeys, Signature, Staking, Timestamp, TransactionPayment, TxExtension,
    UncheckedExtrinsic, VoterList, bag_thresholds, deposit,
    governance::{StakingAdmin, pallet_custom_origins},
    prod_or_fast,
};
use frame_election_provider_support::{
    Get, SequentialPhragmen, bounds::ElectionBoundsBuilder, onchain,
};
use frame_support::{
    PalletId, derive_impl,
    instances::Instance1,
    pallet_prelude::DispatchClass,
    parameter_types,
    traits::{
        AsEnsureOriginWithArg, ConstU8, ConstU32, ConstU64, ConstU128, EitherOf, EitherOfDiverse,
        EqualPrivilegeOnly, LinearStoragePrice, Nothing, VariantCountOf, WithdrawReasons,
        fungible::{HoldConsideration, NativeFromLeft, NativeOrWithId, UnionOf},
        tokens::{imbalance::ResolveTo, pay::PayAssetFromAccount},
    },
    weights::{
        IdentityFee, Weight,
        constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
    },
};
use frame_system::{
    EnsureRoot, EnsureSigned, EnsureWithSuccess,
    limits::{BlockLength, BlockWeights},
};
use pallet_election_provider_multi_phase::GeometricDepositBase;
use pallet_staking::UseValidatorsMap;
use pallet_transaction_payment::{ConstFeeMultiplier, FungibleAdapter, Multiplier};
use sp_core::ConstBool;
use sp_runtime::{
    FixedPointNumber, FixedU128, Perbill, Percent, Permill, SaturatedConversion, traits,
    traits::{ConvertInto, IdentityLookup, Keccak256, One, OpaqueKeys},
    transaction_validity::TransactionPriority,
};
use sp_staking::{EraIndex, SessionIndex};
use sp_version::RuntimeVersion;

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
    pub const Version: RuntimeVersion = VERSION;

    pub RuntimeBlockLength: BlockLength = BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
    pub const SS58Prefix: u8 = 42;
    pub MaxCollectivesProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;

}

/// The default types are being injected by [`derive_impl`](`frame_support::derive_impl`) from
/// [`SoloChainDefaultConfig`](`struct@frame_system::config_preludes::SolochainDefaultConfig`),
/// but overridden as needed.
#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
    /// The block type for the runtime.
    type Block = Block;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = RuntimeBlockWeights;
    /// The maximum length of a block (in bytes).
    type BlockLength = RuntimeBlockLength;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The type for storing how many extrinsics an account has signed.
    type Nonce = Nonce;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = RocksDbWeight;
    /// Version of the runtime.
    type Version = Version;
    /// The data to be stored in an account.
    type AccountData = pallet_balances::AccountData<Balance>;
    /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
    type SS58Prefix = SS58Prefix;
    type MaxConsumers = frame_support::traits::ConstU32<16>;

    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = IdentityLookup<AccountId>;
}

parameter_types! {
    pub const EpochDuration: u64 = prod_or_fast!(EPOCH_DURATION_IN_SLOTS, MINUTES);
    pub const ExpectedBlockTime: Moment = MILLI_SECS_PER_BLOCK;
    pub const ReportLongevity: u64 =
        BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
    pub const MaxAuthorities: u32 = 100_000;

    // Maximum winners that can be chosen as active validators
    pub const MaxActiveValidators: u32 = 1000;
}

impl pallet_babe::Config for Runtime {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    // session module is the trigger
    type EpochChangeTrigger = pallet_babe::ExternalTrigger;
    type DisabledValidators = ();
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;
    type MaxNominators = MaxNominations;
    type KeyOwnerProof = sp_session::MembershipProof;
    type EquivocationReportSystem =
        pallet_babe::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

impl pallet_session::historical::Config for Runtime {
    type FullIdentification = sp_staking::Exposure<AccountId, Balance>;
    type FullIdentificationOf = pallet_staking::ExposureOf<Self>;
}

pub struct EraPayout;
impl pallet_staking::EraPayout<Balance> for EraPayout {
    fn era_payout(
        _total_staked: Balance,
        _total_issuance: Balance,
        era_duration_millis: u64,
    ) -> (Balance, Balance) {
        const MILLISECONDS_PER_YEAR: u64 = (1000 * 3600 * 24 * 36525) / 100;
        // A normal-sized era will have 1 / 365.25 here:
        let relative_era_len =
            FixedU128::from_rational(era_duration_millis.into(), MILLISECONDS_PER_YEAR.into());

        // Fixed total TI that we use as baseline for the issuance.
        let fixed_total_issuance: i128 = 5_216_342_402_773_185_773;
        let fixed_inflation_rate = FixedU128::from_rational(8, 100);
        let yearly_emission = fixed_inflation_rate.saturating_mul_int(fixed_total_issuance);

        let era_emission = relative_era_len.saturating_mul_int(yearly_emission);
        // 15% to treasury, as per Polkadot ref 1139.
        let to_treasury = FixedU128::from_rational(15, 100).saturating_mul_int(era_emission);
        let to_stakers = era_emission.saturating_sub(to_treasury);

        (to_stakers.saturated_into(), to_treasury.saturated_into())
    }
}
pub const WEIGHT_REF_TIME_PER_SECOND: u64 = 1_000_000_000_000;
/// We allow for 2 seconds of compute with a 6 second average block time, with maximum proof size.
const MAXIMUM_BLOCK_WEIGHT: Weight =
    Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2), u64::MAX);

/// We assume that ~10% of the block weight is consumed by `on_initialize` handlers.
/// This is used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);
parameter_types! {
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
    // phase durations. 1/4 of the last session for each.
    pub SignedPhase: u64 = prod_or_fast!(
        EPOCH_DURATION_IN_SLOTS / 4,
        (1 * MINUTES).min(EpochDuration::get().saturated_into::<u64>() / 2)
    );
    pub UnsignedPhase: u64 = prod_or_fast!(
        EPOCH_DURATION_IN_SLOTS / 4,
        (1 * MINUTES).min(EpochDuration::get().saturated_into::<u64>() / 2)
    );

    // signed config
    pub const SignedMaxSubmissions: u32 = 128;
    pub const SignedMaxRefunds: u32 = 128 / 4;
    pub const SignedFixedDeposit: Balance = deposit(2, 0);
    pub const SignedDepositIncreaseFactor: Percent = Percent::from_percent(10);
    pub const SignedDepositByte: Balance = deposit(0, 10) / 1024;
    // Each good submission will get 0.01 XOR as reward
    pub SignedRewardBase: Balance = 1 * CENTS;

    // 1 hour session, 15 minutes unsigned phase, 4 offchain executions.
    pub OffchainRepeat: BlockNumber = UnsignedPhase::get() / 4;

    pub const MaxElectingVoters: u32 = 22_500;
    /// We take the top 22500 nominators as electing voters and all of the validators as electable
    /// targets. Whilst this is the case, we cannot and shall not increase the size of the
    /// validator intentions.
    pub ElectionBounds: frame_election_provider_support::bounds::ElectionBounds =
        ElectionBoundsBuilder::default().voters_count(MaxElectingVoters::get().into()).build();

    /// A limit for off-chain phragmen unsigned solution submission.
    ///
    /// We want to keep it as high as possible, but can't risk having it reject,
    /// so we always subtract the base block execution weight.
    pub OffchainSolutionWeightLimit: Weight = RuntimeBlockWeights::get()
        .get(DispatchClass::Normal)
        .max_extrinsic
        .expect("Normal extrinsics have weight limit configured by default; qed")
        .saturating_sub(BlockExecutionWeight::get());

    /// A limit for off-chain phragmen unsigned solution length.
    ///
    /// We allow up to 90% of the block's size to be consumed by the solution.
    pub OffchainSolutionLengthLimit: u32 = Perbill::from_rational(90_u32, 100) *
        *RuntimeBlockLength::get()
        .max
        .get(DispatchClass::Normal);

}
impl pallet_election_provider_multi_phase::MinerConfig for Runtime {
    type AccountId = AccountId;
    type Solution = NposCompactSolution16;
    type MaxVotesPerVoter = <
    <Self as pallet_election_provider_multi_phase::Config>::DataProvider
    as
    frame_election_provider_support::ElectionDataProvider
    >::MaxVotesPerVoter;
    type MaxLength = OffchainSolutionLengthLimit;
    type MaxWeight = OffchainSolutionWeightLimit;
    type MaxWinners = MaxActiveValidators;

    // The unsigned submissions have to respect the weight of the submit_unsigned call, thus their
    // weight estimate function is wired to this call's weight.
    fn solution_weight(v: u32, t: u32, a: u32, d: u32) -> Weight {
        <
        <Self as pallet_election_provider_multi_phase::Config>::WeightInfo
        as
        pallet_election_provider_multi_phase::WeightInfo
        >::submit_unsigned(v, t, a, d)
    }
}

pub struct MaybeSignedPhase;

impl Get<u64> for MaybeSignedPhase {
    fn get() -> u64 {
        // 1 day = 4 eras -> 1 week = 28 eras. We want to disable signed phase once a week to test
        // the fallback unsigned phase is able to compute elections on Westend.
        if pallet_staking::CurrentEra::<Runtime>::get().unwrap_or(1) % 28 == 0 {
            0
        } else {
            SignedPhase::get()
        }
    }
}

parameter_types! {
    pub const NposSolutionPriority: TransactionPriority = TransactionPriority::MAX / 2;
}
impl<LocalCall> frame_system::offchain::CreateInherent<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    fn create_inherent(call: RuntimeCall) -> UncheckedExtrinsic {
        UncheckedExtrinsic::new_bare(call)
    }
}

impl<C> frame_system::offchain::CreateTransactionBase<C> for Runtime
where
    RuntimeCall: From<C>,
{
    type Extrinsic = UncheckedExtrinsic;
    type RuntimeCall = RuntimeCall;
}

impl<LocalCall> frame_system::offchain::CreateTransaction<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    type Extension = TxExtension;

    fn create_transaction(call: RuntimeCall, extension: TxExtension) -> UncheckedExtrinsic {
        UncheckedExtrinsic::new_transaction(call, extension)
    }
}
impl frame_system::offchain::SigningTypes for Runtime {
    type Public = <Signature as traits::Verify>::Signer;
    type Signature = Signature;
}

impl pallet_authorship::Config for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
    type EventHandler = Staking;
}

/// The numbers configured here could always be more than the the maximum limits of staking pallet
/// to ensure election snapshot will not run out of memory. For now, we set them to smaller values
/// since the staking is bounded and the weight pipeline takes hours for this single pallet.
pub struct BenchmarkConfig;
impl pallet_election_provider_multi_phase::BenchmarkingConfig for BenchmarkConfig {
    const VOTERS: [u32; 2] = [1000, 2000];
    const TARGETS: [u32; 2] = [500, 1000];
    const ACTIVE_VOTERS: [u32; 2] = [500, 800];
    const DESIRED_TARGETS: [u32; 2] = [200, 400];
    const SNAPSHOT_MAXIMUM_VOTERS: u32 = 1000;
    const MINER_MAXIMUM_VOTERS: u32 = 1000;
    const MAXIMUM_TARGETS: u32 = 300;
}

impl pallet_election_provider_multi_phase::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EstimateCallFee = TransactionPayment;
    type UnsignedPhase = UnsignedPhase;
    type SignedPhase = MaybeSignedPhase;
    // rewards are minted from the void
    type BetterSignedThreshold = ();
    type OffchainRepeat = OffchainRepeat;
    type MinerTxPriority = NposSolutionPriority;
    type MinerConfig = Self;
    type SignedMaxSubmissions = SignedMaxSubmissions;
    type SignedMaxWeight =
        <Self::MinerConfig as pallet_election_provider_multi_phase::MinerConfig>::MaxWeight;
    type SignedMaxRefunds = SignedMaxRefunds;
    type SignedRewardBase = SignedRewardBase;
    type SignedDepositByte = SignedDepositByte;
    type SignedDepositWeight = ();
    type MaxWinners = MaxActiveValidators;
    type SignedDepositBase =
        GeometricDepositBase<Balance, SignedFixedDeposit, SignedDepositIncreaseFactor>;
    type ElectionBounds = ElectionBounds;
    type SlashHandler = ();
    // burn slashes
    type RewardHandler = ();
    type DataProvider = Staking;
    type Fallback = frame_election_provider_support::NoElection<(
        AccountId,
        BlockNumber,
        Staking,
        MaxActiveValidators,
    )>;
    type GovernanceFallback = onchain::OnChainExecution<OnChainSeqPhragmen>;
    type Solver = SequentialPhragmen<
        AccountId,
        pallet_election_provider_multi_phase::SolutionAccuracyOf<Self>,
        (),
    >;
    type ForceOrigin = EnsureRoot<AccountId>;
    type BenchmarkingConfig = BenchmarkConfig;
    type WeightInfo = pallet_election_provider_multi_phase::weights::SubstrateWeight<Self>;
}

/// A reasonable benchmarking config for staking pallet.
pub struct StakingBenchmarkingConfig;
impl pallet_staking::BenchmarkingConfig for StakingBenchmarkingConfig {
    type MaxValidators = ConstU32<1000>;
    type MaxNominators = ConstU32<1000>;
}
impl pallet_staking::Config for Runtime {
    type OldCurrency = Balances;
    type Currency = Balances;
    type RuntimeHoldReason = RuntimeHoldReason;
    type CurrencyBalance = Balance;
    type UnixTime = Timestamp;
    type CurrencyToVote = sp_staking::currency_to_vote::U128CurrencyToVote;
    type ElectionProvider = ElectionProviderMultiPhase;
    type GenesisElectionProvider = onchain::OnChainExecution<OnChainSeqPhragmen>;
    type NominationsQuota = pallet_staking::FixedNominationsQuota<{ MaxNominations::get() }>;
    type HistoryDepth = ConstU32<84>;
    type RewardRemainder = ResolveTo<TreasuryAccount, Balances>;
    type RuntimeEvent = RuntimeEvent;
    type Slash = ResolveTo<TreasuryAccount, Balances>; // send the slashed funds to the treasury.
    type Reward = ();
    type SessionsPerEra = SessionsPerEra;
    type BondingDuration = BondingDuration;
    type SlashDeferDuration = SlashDeferDuration;
    type AdminOrigin = EitherOf<EnsureRoot<AccountId>, StakingAdmin>;
    type SessionInterface = Self;
    type EraPayout = EraPayout;
    type NextNewSession = Session;
    type MaxExposurePageSize = MaxExposurePageSize;
    type VoterList = VoterList;
    type TargetList = UseValidatorsMap<Self>;
    type MaxUnlockingChunks = ConstU32<32>;
    type MaxControllersInDeprecationBatch = MaxControllersInDeprecationBatch;
    type EventListeners = (NominationPools, DelegatedStaking);
    type Filter = Nothing;
    type BenchmarkingConfig = StakingBenchmarkingConfig;
    type WeightInfo = pallet_staking::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const BagThresholds: &'static [u64] = &bag_thresholds::THRESHOLDS;
}

type VoterBagsListInstance = pallet_bags_list::Instance1;
impl pallet_bags_list::Config<VoterBagsListInstance> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_bags_list::weights::SubstrateWeight<Runtime>;
    type ScoreProvider = Staking;
    type BagThresholds = BagThresholds;
    type Score = u64;
}

impl pallet_custom_origins::Config for Runtime {}

parameter_types! {
    pub const PoolsPalletId: PalletId = PalletId(*b"py/nopls");
    pub const MaxPointsToBalance: u8 = 10;
}

/// Convert a balance to an unsigned 256-bit number, use in nomination pools.
pub struct BalanceToU256;
impl sp_runtime::traits::Convert<Balance, sp_core::U256> for BalanceToU256 {
    fn convert(n: Balance) -> sp_core::U256 {
        n.into()
    }
}

/// Convert an unsigned 256-bit number to balance, use in nomination pools.
pub struct U256ToBalance;
impl sp_runtime::traits::Convert<sp_core::U256, Balance> for U256ToBalance {
    fn convert(n: sp_core::U256) -> Balance {
        use frame_support::traits::Defensive;
        n.try_into().defensive_unwrap_or(Balance::MAX)
    }
}
impl pallet_nomination_pools::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_nomination_pools::weights::SubstrateWeight<Self>;
    type Currency = Balances;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type RewardCounter = FixedU128;
    type PalletId = PoolsPalletId;
    type MaxPointsToBalance = MaxPointsToBalance;
    // we use the same number of allowed unlocking chunks as with staking.
    type MaxUnbonding = <Self as pallet_staking::Config>::MaxUnlockingChunks;
    type BalanceToU256 = BalanceToU256;
    type U256ToBalance = U256ToBalance;
    type StakeAdapter =
        pallet_nomination_pools::adapter::DelegateStake<Self, Staking, DelegatedStaking>;
    type PostUnbondingPoolsWindow = ConstU32<4>;
    type MaxMetadataLen = ConstU32<256>;
    type AdminOrigin = EitherOf<EnsureRoot<AccountId>, StakingAdmin>;
    type BlockNumberProvider = System;
    type Filter = Nothing;
}

parameter_types! {
    pub const DelegatedStakingPalletId: PalletId = PalletId(*b"py/dlstk");
    pub const SlashRewardFraction: Perbill = Perbill::from_percent(1);
}

impl pallet_delegated_staking::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PalletId = DelegatedStakingPalletId;
    type Currency = Balances;
    type OnSlash = ();
    type SlashRewardFraction = SlashRewardFraction;
    type RuntimeHoldReason = RuntimeHoldReason;
    type CoreStaking = Staking;
}
pub struct OnChainSeqPhragmen;
impl onchain::Config for OnChainSeqPhragmen {
    type System = Runtime;
    type Solver = SequentialPhragmen<AccountId, OnChainAccuracy>;
    type DataProvider = Staking;
    type WeightInfo = frame_election_provider_support::weights::SubstrateWeight<Runtime>;
    type MaxWinners = MaxActiveValidators;
    type Bounds = ElectionBounds;
}

impl pallet_offences::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<Runtime>;
    type OnOffenceHandler = Staking;
}

parameter_types! {
    // Six sessions in an era (6 hours).
    pub const SessionsPerEra: SessionIndex = prod_or_fast!(6, 1);
    // 2 eras for unbonding (12 hours).
    pub const BondingDuration: EraIndex = 2;
    // 1 era in which slashes can be cancelled (6 hours).
    pub const SlashDeferDuration: EraIndex = 1;
    pub const MaxExposurePageSize: u32 = 64;
    // Note: this is not really correct as Max Nominators is (MaxExposurePageSize * page_count) but
    // this is an unbounded number. We just set it to a reasonably high value, 1 full page
    // of nominators.
    pub const MaxNominators: u32 = 64;
    pub const MaxNominations: u32 = <NposCompactSolution16 as frame_election_provider_support::NposSolution>::LIMIT as u32;
    pub const MaxControllersInDeprecationBatch: u32 = 751;
}

impl pallet_session::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = AccountId;
    type ValidatorIdOf = pallet_staking::StashOf<Self>;
    type ShouldEndSession = Babe;
    type NextSessionRotation = Babe;
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
    type SessionHandler = <SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type Keys = SessionKeys;
    type DisablingStrategy = pallet_session::disabling::UpToLimitWithReEnablingDisablingStrategy;
    type WeightInfo = pallet_session::weights::SubstrateWeight<Runtime>;
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;

    type WeightInfo = ();
    type MaxAuthorities = ConstU32<32>;
    type MaxNominators = ConstU32<0>;
    type MaxSetIdSessionEntries = ConstU64<0>;

    type KeyOwnerProof = sp_core::Void;
    type EquivocationReportSystem = ();
}

impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Babe;
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    type WeightInfo = ();
}
parameter_types! {
    pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}

impl pallet_balances::Config for Runtime {
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
    type DoneSlashHandler = ();
}

parameter_types! {
    pub FeeMultiplier: Multiplier = Multiplier::one();
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = FungibleAdapter<Balances, ()>;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ConstFeeMultiplier<FeeMultiplier>;
    type OperationalFeeMultiplier = ConstU8<5>;
    type WeightInfo = pallet_transaction_payment::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    /// The PalletId for the airdrop pallet, used to derive the sovereign account
    pub const AirdropPalletId: PalletId = PalletId(*b"py/airdr");

    /// The amount of tokens to airdrop per claim
    /// 1000 xor tokens
    pub const AirdropAmount: Balance = 1000 * XOR;

    /// Minimum balance threshold to be considered "unfunded"
    /// Users with balance below this can claim airdrops
    pub const MinimumBalanceThreshold: Balance = 100 * XOR;

    /// Maximum number of airdrops allowed per block
    /// Prevents spam and controls distribution rate
    pub const MaxAirdropsPerBlock: u32 = 100;

    /// Cooldown period between airdrops for the same account
    /// 7200 blocks ≈ 12 hours (6 second block time)
    pub const CooldownPeriod: BlockNumber = 7200;

    /// Maximum total airdrops allowed per account
    /// Prevents single accounts from draining the pool
    pub const MaxAirdropsPerAccount: u32 = 10;
}

pub mod mmr {
    use super::Runtime;
    pub use pallet_mmr::primitives::*;

    pub type Leaf = <<Runtime as pallet_mmr::Config>::LeafData as LeafDataProvider>::LeafData;
    pub type Hash = <Hashing as sp_runtime::traits::Hash>::Output;
    pub type Hashing = <Runtime as pallet_mmr::Config>::Hashing;
}
impl pallet_mmr::Config for Runtime {
    const INDEXING_PREFIX: &'static [u8] = b"mmr";
    type Hashing = Keccak256;
    type LeafData = pallet_mmr::ParentNumberAndHash<Self>;
    type OnNewRoot = ();
    type BlockHashProvider = pallet_mmr::DefaultBlockHashProvider<Runtime>;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

impl pallet_authority_discovery::Config for Runtime {
    type MaxAuthorities = MaxAuthorities;
}

parameter_types! {
    /// A unique identifier for the confidential transactions pallet.
    /// Used to derive the sovereign account that holds the shielded pool funds.
    pub const ConfidentialTransactionsPalletId: PalletId = PalletId(*b"xorionct");

    /// The depth of the Merkle tree used for storing commitments.
    /// A depth of 32 allows for 2^32 (over 4 billion) leaves.
    pub const TreeDepth: u32 = 32;
}

impl pallet_private_transactions::Config for Runtime {
    /// The runtime's event type.
    type RuntimeEvent = RuntimeEvent;

    /// The currency type for managing public funds and fees.
    type Currency = Balances;

    /// The PalletId for creating the sovereign account.
    type PalletId = ConfidentialTransactionsPalletId;

    /// The depth of the Merkle tree.
    type TreeDepth = TreeDepth;
}

parameter_types! {
    pub const BridgePalletId: PalletId = PalletId(*b"brdglock");
    pub const RelayerThreshold: u32 = 1; // require 1 signature for now
    pub const MaxSignatures: u32 = 10;   // max 10 signatures per release
}

impl pallet_bridge::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BridgePalletId = BridgePalletId;
    type RelayerThreshold = RelayerThreshold;
    type MaxSignatures = MaxSignatures;
}

impl pallet_insecure_randomness_collective_flip::Config for Runtime {}

parameter_types! {
    pub const DepositPerItem: Balance = deposit(1, 0);
    pub const DepositPerByte: Balance = deposit(0, 1);
    pub const DefaultDepositLimit: Balance = deposit(1024, 1024 * 1024);
    pub Schedule: pallet_contracts::Schedule<Runtime> = Default::default();
    pub CodeHashLockupDepositPercent: Perbill = Perbill::from_percent(30);
}

impl pallet_contracts::Config for Runtime {
    type Time = Timestamp;
    type Randomness = RandomnessCollectiveFlip;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type RuntimeHoldReason = RuntimeHoldReason;
    /// The safest default is to allow no calls at all.
    ///
    /// Runtimes should whitelist dispatchables that are allowed to be called from contracts
    /// and make sure they are stable. Dispatchables exposed to contracts are not allowed to
    /// change because that would break already deployed contracts. The `Call` structure itself
    /// is not allowed to change the indices of existing pallets, too.
    type CallFilter = Nothing;
    type WeightPrice = pallet_transaction_payment::Pallet<Self>;
    type WeightInfo = pallet_contracts::weights::SubstrateWeight<Self>;
    type ChainExtension = ();
    type Schedule = Schedule;
    type CallStack = [pallet_contracts::Frame<Self>; 5];
    type DepositPerByte = DepositPerByte;
    type DefaultDepositLimit = DefaultDepositLimit;
    type DepositPerItem = DepositPerItem;
    type CodeHashLockupDepositPercent = CodeHashLockupDepositPercent;
    type AddressGenerator = pallet_contracts::DefaultAddressGenerator;
    type MaxCodeLen = ConstU32<{ 123 * 1024 }>;
    type MaxStorageKeyLen = ConstU32<128>;
    type MaxTransientStorageSize = ConstU32<{ 1 * 1024 * 1024 }>;
    type MaxDelegateDependencies = ConstU32<32>;
    type UnsafeUnstableInterface = ConstBool<false>;
    type MaxDebugBufferLen = ConstU32<{ 2 * 1024 * 1024 }>;
    type UploadOrigin = EnsureSigned<Self::AccountId>;
    type InstantiateOrigin = EnsureSigned<Self::AccountId>;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type Migrations = ();
    #[cfg(feature = "runtime-benchmarks")]
    type Migrations = pallet_contracts::migration::codegen::BenchMigrations;
    type Debug = ();
    type Environment = ();
    type ApiVersion = ();
    type Xcm = ();
}

parameter_types! {
    pub const AssetDeposit: Balance = 100 * XOR;
    pub const ApprovalDeposit: Balance = 1 * XOR;
    pub const StringLimit: u32 = 50;
    pub const MetadataDepositBase: Balance = 10 * XOR;
    pub const MetadataDepositPerByte: Balance = 1 * XOR;
}

impl pallet_assets::Config<Instance1> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type RemoveItemsLimit = ConstU32<1000>;
    type AssetId = u32;
    type AssetIdParameter = codec::Compact<u32>;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = ConstU128<XOR>;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = StringLimit;
    type Freezer = ();
    type Holder = ();
    type Extra = ();
    type CallbackHandle = ();
    type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

parameter_types! {
    pub const LaunchPeriod: BlockNumber = prod_or_fast!(36 * 60 * MINUTES, MINUTES); // 14 days
    pub const VotingPeriod: BlockNumber = prod_or_fast!(2 * 24 * 60 * MINUTES, MINUTES);
    pub const FastTrackVotingPeriod: BlockNumber = prod_or_fast!(6 * 60 * MINUTES, MINUTES / 2);
    pub const MinimumDeposit: Balance = 100 * XOR;
    pub const EnactmentPeriod: BlockNumber = prod_or_fast!(3 * HOURS, 2* MINUTES);
    pub const CooloffPeriod: BlockNumber = prod_or_fast!(3 * 24 * 60 * MINUTES, MINUTES);
    pub const MaxProposals: u32 = 1000;
}

parameter_types! {
    pub const SpendPeriod: BlockNumber = 1 * DAYS;
    pub const Burn: Permill = Permill::from_percent(50);
    pub const TipCountdown: BlockNumber = 1 * DAYS;
    pub const TipFindersFee: Percent = Percent::from_percent(20);
    pub const TipReportDepositBase: Balance = 1 * XOR;
    pub const DataDepositPerByte: Balance = 1 * CENTS;
    pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
    pub const MaximumReasonLength: u32 = 300;
    pub const MaxApprovals: u32 = 100;
    pub const MaxBalance: Balance = Balance::MAX;
    pub const SpendPayoutPeriod: BlockNumber = prod_or_fast!(15 * DAYS, MINUTES);
}

pub type NativeAndAssets =
    UnionOf<Balances, Assets, NativeFromLeft, NativeOrWithId<u32>, AccountId>;
impl pallet_treasury::Config for Runtime {
    type Currency = Balances;
    type RejectOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
    >;
    type RuntimeEvent = RuntimeEvent;
    type SpendPeriod = SpendPeriod;
    type Burn = Burn;
    type PalletId = TreasuryPalletId;
    type BurnDestination = ();
    type WeightInfo = pallet_treasury::weights::SubstrateWeight<Runtime>;
    type SpendFunds = Bounties;
    type MaxApprovals = MaxApprovals;
    type SpendOrigin = EnsureWithSuccess<EnsureRoot<AccountId>, AccountId, MaxBalance>;
    type AssetKind = NativeOrWithId<u32>;
    type Beneficiary = AccountId;
    type BeneficiaryLookup = IdentityLookup<AccountId>;
    type Paymaster = PayAssetFromAccount<NativeAndAssets, TreasuryAccount>;
    type BalanceConverter = AssetRate;
    type PayoutPeriod = SpendPayoutPeriod;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = PalletTreasuryArguments;
    type BlockNumberProvider = System;
}
parameter_types! {
    pub const Budget: Balance = 10_000 * XOR;
    pub TreasuryAccount: AccountId = Treasury::account_id();
}

impl pallet_democracy::Config for Runtime {
    type WeightInfo = pallet_democracy::weights::SubstrateWeight<Runtime>;
    type RuntimeEvent = RuntimeEvent;
    type Scheduler = Scheduler;
    type Preimages = Preimage;
    type Currency = Balances;
    type EnactmentPeriod = EnactmentPeriod;
    type LaunchPeriod = LaunchPeriod;
    type VotingPeriod = VotingPeriod;
    type VoteLockingPeriod = EnactmentPeriod;
    // Same as EnactmentPeriod
    type MinimumDeposit = MinimumDeposit;
    type InstantAllowed = ConstBool<true>;
    type FastTrackVotingPeriod = FastTrackVotingPeriod;
    type CooloffPeriod = CooloffPeriod;
    type MaxVotes = ConstU32<1000>;
    type MaxProposals = MaxProposals;
    type MaxDeposits = ConstU32<100>;
    type MaxBlacklisted = ConstU32<100>;
    /// A straight majority of the council can decide what their next motion is.
    type ExternalOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>;
    /// A super-majority can have the next scheduled referendum be a straight majority-carries vote.
    type ExternalMajorityOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>;
    /// A unanimous council can have the next scheduled referendum be a straight default-carries
    /// (NTB) vote.
    type ExternalDefaultOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>;
    type SubmitOrigin = EnsureSigned<AccountId>;
    /// Two thirds of the technical committee can have an ExternalMajority/ExternalDefault vote
    /// be tabled immediately and with a shorter voting/enactment period.
    type FastTrackOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>;
    type InstantOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>;
    // To cancel a proposal which has been passed, 2/3 of the council must agree to it.
    type CancellationOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>;
    type BlacklistOrigin = EnsureRoot<AccountId>;
    // To cancel a proposal before it has been passed, the technical committee must be unanimous or
    // Root must agree.
    type CancelProposalOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
    >;
    // Any single technical committee member may veto a coming council proposal, however they can
    // only do it once and it lasts only for the cool-off period.
    type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
    type PalletsOrigin = OriginCaller;
    type Slash = Treasury;
}

parameter_types! {
    pub const CouncilMotionDuration: BlockNumber = prod_or_fast!(DAYS, MINUTES);
    pub const CouncilMaxProposals: u32 = 100;
    pub const CouncilMaxMembers: u32 = 100;
    pub const ProposalDepositOffset: Balance = ExistentialDeposit::get() + ExistentialDeposit::get();
    pub const ProposalHoldReason: RuntimeHoldReason =
        RuntimeHoldReason::Council(pallet_collective::HoldReason::ProposalSubmission);
}

type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = CouncilMotionDuration;
    type MaxProposals = CouncilMaxProposals;
    type MaxMembers = CouncilMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
    type SetMembersOrigin = EnsureRoot<Self::AccountId>;
    type MaxProposalWeight = MaxCollectivesProposalWeight;
    type DisapproveOrigin = EnsureRoot<Self::AccountId>;
    type KillOrigin = EnsureRoot<Self::AccountId>;
    type Consideration = HoldConsideration<
        AccountId,
        Balances,
        ProposalHoldReason,
        pallet_collective::deposit::Delayed<
            ConstU32<2>,
            pallet_collective::deposit::Linear<ConstU32<2>, ProposalDepositOffset>,
        >,
        u32,
    >;
}
parameter_types! {
    pub const TechnicalMotionDuration: BlockNumber = 3 * HOURS;
    pub const TechnicalMaxProposals: u32 = 100;
    pub const TechnicalMaxMembers: u32 = 100;
}

type TechnicalCollective = pallet_collective::Instance2;
impl pallet_collective::Config<TechnicalCollective> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = TechnicalMotionDuration;
    type MaxProposals = TechnicalMaxProposals;
    type MaxMembers = TechnicalMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
    type SetMembersOrigin = EnsureRoot<Self::AccountId>;
    type MaxProposalWeight = MaxCollectivesProposalWeight;
    type DisapproveOrigin = EnsureRoot<Self::AccountId>;
    type KillOrigin = EnsureRoot<Self::AccountId>;
    type Consideration = ();
}

parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
        RuntimeBlockWeights::get().max_block;
}

impl pallet_scheduler::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    #[cfg(feature = "runtime-benchmarks")]
    type MaxScheduledPerBlock = ConstU32<512>;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type MaxScheduledPerBlock = ConstU32<50>;
    type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Runtime>;
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
    type Preimages = Preimage;
    type BlockNumberProvider = frame_system::Pallet<Runtime>;
}

parameter_types! {
    pub const PreimageHoldReason: RuntimeHoldReason =
        RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);
}

impl pallet_preimage::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_preimage::weights::SubstrateWeight<Runtime>;
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type Consideration = HoldConsideration<
        AccountId,
        Balances,
        PreimageHoldReason,
        LinearStoragePrice<
            ConstU128<XOR>,   // Configures the base deposit of storing some data.
            ConstU128<CENTS>, // Configures the per-byte deposit of storing some data.
            Balance,
        >,
    >;
}

impl pallet_asset_rate::Config for Runtime {
    type WeightInfo = pallet_asset_rate::weights::SubstrateWeight<Runtime>;
    type RuntimeEvent = RuntimeEvent;
    type CreateOrigin = EnsureRoot<AccountId>;
    type RemoveOrigin = EnsureRoot<AccountId>;
    type UpdateOrigin = EnsureRoot<AccountId>;
    type Currency = Balances;
    type AssetKind = NativeOrWithId<u32>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = AssetRateArguments;
}

parameter_types! {
    pub const BountyCuratorDeposit: Permill = Permill::from_percent(50);
    pub const BountyValueMinimum: Balance = 5 * XOR;
    pub const BountyDepositBase: Balance = 1 * XOR;
    pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
    pub const CuratorDepositMin: Balance = 1 * XOR;
    pub const CuratorDepositMax: Balance = 100 * XOR;
    pub const BountyDepositPayoutDelay: BlockNumber = 1 * DAYS;
    pub const BountyUpdatePeriod: BlockNumber = 14 * DAYS;
}

impl pallet_bounties::Config for Runtime {
    type BountyDepositBase = BountyDepositBase;
    type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
    type BountyUpdatePeriod = BountyUpdatePeriod;
    type CuratorDepositMultiplier = CuratorDepositMultiplier;
    type CuratorDepositMax = CuratorDepositMax;
    type CuratorDepositMin = CuratorDepositMin;
    type BountyValueMinimum = BountyValueMinimum;
    type DataDepositPerByte = DataDepositPerByte;
    type RuntimeEvent = RuntimeEvent;
    type MaximumReasonLength = MaximumReasonLength;
    type WeightInfo = pallet_bounties::weights::SubstrateWeight<Runtime>;
    type ChildBountyManager = ChildBounties;
    type OnSlash = Treasury;
}

parameter_types! {
    pub const ChildBountyValueMinimum: Balance = 1 * XOR;
}

impl pallet_child_bounties::Config for Runtime {
    type MaxActiveChildBountyCount = ConstU32<5>;
    type ChildBountyValueMinimum = ChildBountyValueMinimum;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_child_bounties::weights::SubstrateWeight<Runtime>;
}

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const MinVestedTransfer: Balance = 100 * XOR;
    pub UnvestedFundsAllowedWithdrawReasons: WithdrawReasons =
        WithdrawReasons::except(WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE);
}

impl pallet_vesting::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BlockNumberToBalance = ConvertInto;
    type MinVestedTransfer = MinVestedTransfer;
    type WeightInfo = pallet_vesting::weights::SubstrateWeight<Runtime>;
    type UnvestedFundsAllowedWithdrawReasons = UnvestedFundsAllowedWithdrawReasons;
    type BlockNumberProvider = System;
    // `VestingInfo` encode length is 36bytes. 28 schedules gets encoded as 1009 bytes, which is the
    // highest number of schedules that encodes less than 2^10.
    const MAX_VESTING_SCHEDULES: u32 = 28;
}

parameter_types! {
    // One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
    pub const DepositBase: Balance = deposit(1, 88);
    // Additional storage item size of 32 bytes.
    pub const DepositFactor: Balance = deposit(0, 32);

    pub const VestingPeriod: BlockNumber = 6*30 * DAYS;
}

impl pallet_multisig::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = ConstU32<100>;
    type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
    type BlockNumberProvider = frame_system::Pallet<Runtime>;
}

impl pallet_launch_claim::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type VestingPeriod = VestingPeriod;
}
