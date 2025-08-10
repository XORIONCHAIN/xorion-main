use ark_bn254::{Bn254, G1Projective, G2Projective};
use ark_groth16::{Proof, VerifyingKey};
use ark_serialize::CanonicalSerialize;
use ark_std::{
    UniformRand,
    rand::{SeedableRng, prelude::StdRng},
};
use frame_support::{PalletId, derive_impl, pallet_prelude::ConstU32, parameter_types};
use sp_runtime::BuildStorage;
use std::{fs, sync::OnceLock};

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
/// Other variable is Proof
fn create_dummy_vk(num_public_inputs: u32) -> (Vec<u8>, Vec<u8>) {
    // 1. Create a deterministic random number generator for reproducibility
    let mut rng = StdRng::seed_from_u64(42);

    // 2. Generate random points on the G1 and G2 curves
    let alpha_g1 = G1Projective::rand(&mut rng);
    let beta_g2 = G2Projective::default();

    // 3. Generate a random vector of G1 points for the IC
    let gamma_abc_g1 = vec![alpha_g1.into(); (num_public_inputs + 1) as usize];

    // 4. Construct the Verifying Key
    let vk = VerifyingKey::<Bn254> {
        alpha_g1: alpha_g1.into(),
        beta_g2: beta_g2.into(),
        gamma_g2: beta_g2.into(),
        delta_g2: beta_g2.into(),
        gamma_abc_g1,
    };

    let mut vk_bytes = Vec::new();

    vk.serialize_uncompressed(&mut vk_bytes).unwrap();
    let proof = Proof::<Bn254> { a: alpha_g1.into(), b: beta_g2.into(), c: alpha_g1.into() };
    let mut proof_bytes = Vec::new();
    proof.serialize_uncompressed(&mut proof_bytes).unwrap();
    (vk_bytes, proof_bytes)
}

pub static DEPOSIT_PROOF: OnceLock<Vec<u8>> = OnceLock::new();
pub static TRANSFER_PROOF: OnceLock<Vec<u8>> = OnceLock::new();

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
    let (deposit_vk, deposit_proof) = create_dummy_vk(2);
    let (transfer_vk, transfer_proof) = create_dummy_vk(5);
    fs::write("vk_depo", hex::encode(&deposit_vk)).unwrap();
    fs::write("vk_trans", hex::encode(&transfer_vk)).unwrap();
    fs::write("proof_dep", hex::encode(&deposit_proof)).unwrap();
    fs::write("proof_trans", hex::encode(&transfer_proof)).unwrap();
    _ = DEPOSIT_PROOF.set(deposit_proof);
    _ = TRANSFER_PROOF.set(transfer_proof);
    crate::GenesisConfig::<Test> {
        deposit_vk,  // For deposit circuit with 2 public inputs
        transfer_vk, // For transfer circuit with 5 public inputs
        _phantom: Default::default(),
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(storage);
    // Go past block 0 to trigger on_initialize for setting up the tree
    ext.execute_with(|| System::set_block_number(1));
    ext
}
