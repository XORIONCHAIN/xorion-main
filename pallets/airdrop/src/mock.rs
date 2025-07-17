use frame_support::{
    derive_impl,
    pallet_prelude::{ConstU32, Hooks},
    parameter_types, PalletId,
};
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;
pub type AccountId = u64;

#[frame_support::runtime]
mod runtime {
    // The main runtime
    #[runtime::runtime]
    // Runtime Types to be generated
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask,
        RuntimeViewFunction
    )]
    pub struct Test;

    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<Test>;

    #[runtime::pallet_index(1)]
    pub type Balances = pallet_balances::Pallet<Test>;
    #[runtime::pallet_index(2)]
    pub type Airdrop = crate::Pallet<Test>;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId;
    type AccountData = pallet_balances::AccountData<u128>;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}
impl pallet_balances::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type WeightInfo = ();
    type Balance = u128;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = ();
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type MaxFreezes = ConstU32<0>;
    type DoneSlashHandler = ();
}

parameter_types! {
    pub const AirdropPalletId: PalletId = PalletId(*b"airdrop!");
    pub const AirdropAmount: u128 = 1000;
    pub const MinimumBalanceThreshold: u128 = 100;
    pub const MaxAirdropsPerBlock: u32 = 10;
    pub const CooldownPeriod: u64 = 5; // 5 blocks
    pub const MaxAirdropsPerAccount: u32 = 3;
}

impl crate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type PalletId = AirdropPalletId;
    type AirdropAmount = AirdropAmount;
    type MinimumBalanceThreshold = MinimumBalanceThreshold;
    type MaxAirdropsPerBlock = MaxAirdropsPerBlock;
    type CooldownPeriod = CooldownPeriod;
    type MaxAirdropsPerAccount = MaxAirdropsPerAccount;
}

// Helper function to create a test externalities
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    // Configure genesis for balances
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 5000), // Account 1 has high balance (above threshold)
            (2, 50),   // Account 2 has low balance (below threshold)
            (3, 10),   // Account 3 has zero balance
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    // Configure genesis for airdrop pallet
    crate::GenesisConfig::<Test> {
        initial_funding: 20000,       // Fund the airdrop pool
        pre_funded_accounts: vec![4], // Pre-fund account 4
    }
    .assimilate_storage(&mut t)
    .unwrap();
    let mut ext: sp_io::TestExternalities = t.into();
    ext.execute_with(|| {
        // Set the initial block number to 1
        System::set_block_number(1);
        // Initialize the airdrop pallet for block 1
        Airdrop::on_initialize(1);
    });
    ext
}

// Helper function to run to block
pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        System::set_block_number(System::block_number() + 1);
        Airdrop::on_initialize(System::block_number());
    }
}

#[test]
fn genesis_config_works() {
    new_test_ext().execute_with(|| {
        // Check airdrop pool is funded
        let airdrop_account = Airdrop::airdrop_account_id();
        assert_eq!(Balances::free_balance(&airdrop_account), 19000);

        // Check that account 4 was pre-funded
        assert_eq!(Airdrop::airdrop_records(&4).unwrap().claims_count, 1);
        assert_eq!(Balances::free_balance(&4), 1000);
    });
}
