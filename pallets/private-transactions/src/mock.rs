use ark_bn254::Bn254;
use ark_ec::pairing::Pairing;
use ark_groth16::{Proof, VerifyingKey};
use ark_serialize::CanonicalSerialize;
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
    pub type ConfidentialTransactions = crate::Pallet<Test>;
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

impl crate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type PalletId = ConfidentialTransactionsPalletId;
    type TreeDepth = TreeDepth;
}

/// Helper to create a valid, serialized but dummy verification key for testing.
fn create_dummy_vk(num_public_inputs: u32) -> Vec<u8> {
    let identity_g1 = <Bn254 as Pairing>::G1::default();
    let identity_g2 = <Bn254 as Pairing>::G2::default();

    let vk = VerifyingKey::<Bn254> {
        alpha_g1: identity_g1.into(),
        beta_g2: identity_g2.into(),
        gamma_g2: identity_g2.into(),
        delta_g2: identity_g2.into(),
        gamma_abc_g1: vec![identity_g1.into(); (num_public_inputs + 1) as usize],
    };

    let mut vk_bytes = Vec::new();
    vk.serialize_uncompressed(&mut vk_bytes).unwrap();
    vk_bytes
}

/// Helper to create a valid, serialized but dummy proof for testing.
pub fn create_dummy_proof() -> Vec<u8> {
    let identity_g1 = <Bn254 as Pairing>::G1::default();
    let identity_g2 = <Bn254 as Pairing>::G2::default();

    let proof =
        Proof::<Bn254> { a: identity_g1.into(), b: identity_g2.into(), c: identity_g1.into() };

    let mut proof_bytes = Vec::new();
    proof.serialize_uncompressed(&mut proof_bytes).unwrap();
    proof_bytes
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

    // Configure genesis for our pallet by creating and serializing
    // structurally valid (but dummy) verification keys.
    crate::GenesisConfig::<Test> {
        deposit_vk: create_dummy_vk(2), // For deposit circuit with 2 public inputs
        transfer_vk: create_dummy_vk(5), // For transfer circuit with 5 public inputs
        _phantom: Default::default(),
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(storage);
    // Go past block 0 to trigger on_initialize for setting up the tree
    ext.execute_with(|| System::set_block_number(1));
    ext
}
