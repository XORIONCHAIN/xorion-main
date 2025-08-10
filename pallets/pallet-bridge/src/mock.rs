use frame_support::{PalletId, derive_impl, pallet_prelude::ConstU32, parameter_types};
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;

// Configure a mock runtime to test the pallet.
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
    pub type Bridge = crate::Pallet<Test>;
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
    pub const ConfidentialTransactionsPalletId: PalletId = PalletId(*b"xorionct");
    pub const TreeDepth: u32 = 32;
}

parameter_types! {
    pub const BridgePalletId: PalletId = PalletId(*b"brdglock");
    pub const RelayerThreshold: u32 = 0; // require 0 signature for mock
    pub const MaxSignatures: u32 = 10;   // max 10 signatures per release
}

impl crate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BridgePalletId = BridgePalletId;
    type RelayerThreshold = RelayerThreshold;
    type MaxSignatures = MaxSignatures;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    // Configure the genesis storage for the balances pallet
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(1, 1000), (2, 1000)], // Alice and Bob start with 1000
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .unwrap();
    let mut ext = sp_io::TestExternalities::new(storage);
    // Go past block 0 to trigger on_initialize for setting up the tree
    ext.execute_with(|| System::set_block_number(1));
    ext
}
