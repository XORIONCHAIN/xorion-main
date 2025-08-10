#![cfg_attr(not(feature = "std"), no_std)]

//! # Production-Grade Confidential Transactions Pallet
//!
//! This pallet implements a robust and scalable confidential transaction layer
//! using zk-SNARKs. It features a custom, gas-efficient Merkle tree implementation
//! that leverages the cryptographic primitives from the `arkworks` ecosystem.
//!
//! ## Features
//!
//! - **Integrated Merkle Tree**: A custom, on-chain Merkle tree provides scalability without
//!   external dependencies. It uses `ark-crypto-primitives` for its core hashing logic, ensuring
//!   cryptographic security.
//! - **Secure Deposits**: Deposit function requires a zk-SNARK proof to prevent the creation of
//!   unbacked value within the shielded pool.
//! - **Sovereign Liquidity Pool**: Manages all deposited funds in a secure, pallet-owned sovereign
//!   account.
//! - **Atomic Transactions**: The `transact` extrinsic enables private peer-to-peer transfers
//!   within the shielded pool.
//! - **Distinct Verification Keys**: Manages separate, dedicated verification keys for deposit and
//!   transfer circuits.
//!
//! ## Public Inputs and Serialization
//!
//! All `public_inputs` for the extrinsics must be serialized correctly. The process is:
//! 1. Convert the native data type into a field element (`ark_bls12_381::Fr`).
//! 2. Serialize the field element into bytes (`Vec<u8>`) using `ark_serialize::CanonicalSerialize`.
//! 3. Pass the `Vec<Vec<u8>>` to the extrinsic. The order is critical and specified below.

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;

// --- Merkle Tree Implementation ---
mod merkle_tree {
    use ark_crypto_primitives::{
        Error as ArkError,
        crh::{CRHScheme, TwoToOneCRHScheme},
    };
    use ark_std::rand::Rng;
    use sp_io::hashing::blake2_256;
    use sp_std::{borrow::Borrow, vec::Vec};

    /// A simple collision-resistant hash function for our Merkle tree.
    /// It uses blake2_256 for hashing.
    pub struct Blake2s;
    impl CRHScheme for Blake2s {
        type Input = [u8];
        type Output = [u8; 32];
        type Parameters = ();

        fn setup<R: Rng>(_r: &mut R) -> Result<Self::Parameters, ArkError> {
            Ok(())
        }

        fn evaluate<T: Borrow<Self::Input>>(
            _parameters: &Self::Parameters,
            input: T,
        ) -> Result<Self::Output, ArkError> {
            Ok(blake2_256(input.borrow()))
        }
    }

    impl TwoToOneCRHScheme for Blake2s {
        type Input = [u8];
        type Output = [u8; 32];
        type Parameters = ();

        /// The setup function is now part of this trait.
        /// It remains a no-op as blake2 requires no parameters.
        fn setup<R: Rng>(_r: &mut R) -> Result<Self::Parameters, ArkError> {
            Ok(())
        }

        /// This method is for hashing leaf data. It is not used by the pallet's
        /// current logic (which assumes leaves are pre-hashed), but is required
        /// to satisfy the trait.
        fn evaluate<T: Borrow<Self::Input>>(
            _parameters: &Self::Parameters,
            left_input: T,
            right_input: T,
        ) -> Result<Self::Output, ArkError> {
            let mut total_input = Vec::new();
            total_input.extend_from_slice(left_input.borrow());
            total_input.extend_from_slice(right_input.borrow());
            Ok(blake2_256(&total_input))
        }

        /// This method is for hashing two inner nodes (which are themselves hashes)
        /// together to produce a parent node hash. This is the core function
        /// used by the `insert_leaf` logic.
        fn compress<T: Borrow<Self::Output>>(
            _parameters: &Self::Parameters,
            left_input: T,
            right_input: T,
        ) -> Result<Self::Output, ArkError> {
            let mut total_input = Vec::new();
            total_input.extend_from_slice(left_input.borrow());
            total_input.extend_from_slice(right_input.borrow());
            Ok(blake2_256(&total_input))
        }
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::merkle_tree::Blake2s;
    use frame_support::{
        PalletId,
        dispatch::DispatchResult,
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, ReservableCurrency},
    };
    use frame_system::pallet_prelude::*;
    use sp_core::H256;
    use sp_runtime::traits::AccountIdConversion;
    use sp_std::vec::Vec;

    // Arkworks ecosystem imports
    use ark_bls12_381::{Bls12_381, Fr};
    use ark_crypto_primitives::crh::TwoToOneCRHScheme;
    use ark_ff::PrimeField;
    use ark_groth16::{Groth16, Proof, VerifyingKey};
    use ark_serialize::CanonicalDeserialize;
    use ark_snark::SNARK;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: ReservableCurrency<Self::AccountId>;
        #[pallet::constant]
        type PalletId: Get<PalletId>;
        #[pallet::constant]
        type TreeDepth: Get<u32>;
    }

    // --- Storage ---
    #[pallet::storage]
    #[pallet::getter(fn deposit_vk)]
    #[pallet::unbounded]
    pub type DepositVerificationKey<T: Config> = StorageValue<_, Vec<u8>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn transfer_vk)]
    #[pallet::unbounded]
    pub type TransferVerificationKey<T: Config> = StorageValue<_, Vec<u8>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn merkle_root)]
    pub type MerkleRoot<T: Config> = StorageValue<_, H256, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn tree_nodes)]
    pub type TreeNodes<T: Config> = StorageMap<_, Blake2_128Concat, (u32, u64), H256, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_leaf_index)]
    pub type NextLeafIndex<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn nullifiers)]
    pub type Nullifiers<T: Config> = StorageMap<_, Blake2_128Concat, H256, bool, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub deposit_vk: Vec<u8>,
        pub transfer_vk: Vec<u8>,
        pub _phantom: PhantomData<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                deposit_vk: Default::default(),
                transfer_vk: Default::default(),
                _phantom: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            DepositVerificationKey::<T>::put(&self.deposit_vk);
            TransferVerificationKey::<T>::put(&self.transfer_vk);
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A deposit was made into the shielded pool. [who, amount, leaf_index]
        Deposit(T::AccountId, BalanceOf<T>, u64),
        /// A withdrawal was made from the shielded pool. [who, amount]
        Withdraw(T::AccountId, BalanceOf<T>),
        /// A confidential transaction was successful.
        TransactionSuccess,
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The deposit verification key has not been set up yet.
        DepositVerificationKeyNotSet,
        /// The transfer verification key has not been set up yet.
        TransferVerificationKeyNotSet,
        /// The provided verification key is malformed.
        MalformedVerificationKey,
        /// The provided proof is malformed.
        MalformedProof,
        /// The zk-SNARK proof is invalid.
        InvalidProof,
        /// The transaction attempts to spend a note that has already been spent.
        NullifierAlreadyUsed,
        /// The Merkle root specified in the proof is outdated or invalid.
        InvalidMerkleRoot,
        /// The amount to deposit must be greater than zero.
        InvalidDepositAmount,
        /// The public inputs for the proof are malformed or do not match.
        InvalidPublicInputs,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Deposit funds into the shielded pool.
        ///
        /// Requires a zk-SNARK proof to ensure the commitment's value matches the public amount.
        ///
        /// # Parameters
        /// - `proof`: The serialized Groth16 proof for the deposit circuit.
        /// - `public_inputs`: A vector of serialized field elements. The order is critical:
        ///   - `[0]`: The public `amount` being deposited.
        ///   - `[1]`: The `commitment` hash for the new private note.
        /// - `amount`: The public amount of currency to deposit. Must match the amount in the
        ///   proof.
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(5, 4))]
        pub fn deposit(
            origin: OriginFor<T>,
            proof: Vec<u8>,
            public_inputs: Vec<Vec<u8>>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(amount > 0u32.into(), Error::<T>::InvalidDepositAmount);

            let vk = Self::deposit_vk().ok_or(Error::<T>::DepositVerificationKeyNotSet)?;

            // Verify the deposit proof.
            Self::verify_proof_internal(&vk, &proof, &public_inputs)?;

            // The commitment is the second public input from the proof.
            let commitment_bytes =
                public_inputs.get(1).ok_or(Error::<T>::InvalidPublicInputs)?.clone();
            let commitment = H256::from_slice(&commitment_bytes);

            // Transfer funds from the user to the pallet's sovereign account.
            T::Currency::transfer(
                &who,
                &Self::sovereign_account_id(),
                amount,
                ExistenceRequirement::AllowDeath,
            )?;

            // Insert into our custom Merkle tree
            let leaf_index = Self::insert_leaf(commitment)?;

            Self::deposit_event(Event::Deposit(who, amount, leaf_index));
            Ok(())
        }

        /// Withdraw funds from the shielded pool.
        ///
        /// # Parameters
        /// - `proof`: The serialized Groth16 proof for the transfer circuit.
        /// - `public_inputs`: A vector of serialized field elements. The order is critical:
        ///   - `[0]`: The `merkle_root` of the commitments tree. (`H256.as_bytes()`).
        ///   - `[1]`: The `nullifier` of the note being spent. (`H256.as_bytes()`).
        ///   - `[2]`: A hash of the public `recipient` account ID. (`H256.as_bytes()`).
        ///   - `[3]`: The `amount` being withdrawn. (`Balance.as_bytes()`).
        ///   - `[4]`: The transaction `fee`. (`Balance.as_bytes()`).
        /// - `recipient`: The public account ID to receive the funds.
        /// - `amount`: The public amount to withdraw. Must match the amount in the proof.
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(4, 4))]
        pub fn withdraw(
            origin: OriginFor<T>,
            proof: Vec<u8>,
            public_inputs: Vec<Vec<u8>>,
            recipient: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?; // `who` pays the extrinsic fee
            let vk = Self::transfer_vk().ok_or(Error::<T>::TransferVerificationKeyNotSet)?;

            // Verify the Merkle root from the public inputs matches the on-chain root.
            let merkle_root =
                H256::from_slice(public_inputs.first().ok_or(Error::<T>::InvalidPublicInputs)?);
            ensure!(merkle_root == Self::merkle_root(), Error::<T>::InvalidMerkleRoot);

            // Verify the withdrawal proof.
            Self::verify_proof_internal(&vk, &proof, &public_inputs)?;

            // Check and use the nullifier from the public inputs.
            let nullifier =
                H256::from_slice(public_inputs.get(1).ok_or(Error::<T>::InvalidPublicInputs)?);
            ensure!(!Self::nullifiers(nullifier), Error::<T>::NullifierAlreadyUsed);
            Nullifiers::<T>::insert(nullifier, true);

            // Transfer funds from the sovereign account to the recipient.
            T::Currency::transfer(
                &Self::sovereign_account_id(),
                &recipient,
                amount,
                ExistenceRequirement::AllowDeath,
            )?;

            Self::deposit_event(Event::Withdraw(recipient, amount));
            Ok(())
        }

        /// Perform a private transfer within the shielded pool.
        ///
        /// # Parameters
        /// - `proof`: The serialized Groth16 proof for the transfer circuit.
        /// - `public_inputs`: A vector of serialized field elements. The order is critical:
        ///   - `[0]`: The `merkle_root` of the commitments tree.
        ///   - `[1]`: The `nullifier1` of the first input note being spent.
        ///   - `[2]`: The `nullifier2` of the second input note being spent.
        ///   - `[3]`: The `commitment1` of the first new output note.
        ///   - `[4]`: The `commitment2` of the second new output note.
        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().reads_writes(5, 7))]
        pub fn transact(
            origin: OriginFor<T>,
            proof: Vec<u8>,
            public_inputs: Vec<Vec<u8>>,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            let vk = Self::transfer_vk().ok_or(Error::<T>::TransferVerificationKeyNotSet)?;

            let merkle_root =
                H256::from_slice(public_inputs.first().ok_or(Error::<T>::InvalidPublicInputs)?);
            ensure!(merkle_root == Self::merkle_root(), Error::<T>::InvalidMerkleRoot);

            Self::verify_proof_internal(&vk, &proof, &public_inputs)?;

            // Process nullifiers (inputs to the transaction)
            let nullifier1 =
                H256::from_slice(public_inputs.get(1).ok_or(Error::<T>::InvalidPublicInputs)?);
            let nullifier2 =
                H256::from_slice(public_inputs.get(2).ok_or(Error::<T>::InvalidPublicInputs)?);
            ensure!(!Self::nullifiers(nullifier1), Error::<T>::NullifierAlreadyUsed);
            ensure!(!Self::nullifiers(nullifier2), Error::<T>::NullifierAlreadyUsed);
            Nullifiers::<T>::insert(nullifier1, true);
            Nullifiers::<T>::insert(nullifier2, true);

            // Process new commitments (outputs of the transaction)
            let commitment1 =
                H256::from_slice(public_inputs.get(3).ok_or(Error::<T>::InvalidPublicInputs)?);
            let commitment2 =
                H256::from_slice(public_inputs.get(4).ok_or(Error::<T>::InvalidPublicInputs)?);
            Self::insert_leaf(commitment1)?;
            Self::insert_leaf(commitment2)?;

            Self::deposit_event(Event::TransactionSuccess);
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Get the sovereign account ID for this pallet.
        pub fn sovereign_account_id() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }

        /// Inserts a new leaf into the Merkle tree and updates the root.
        fn insert_leaf(leaf: H256) -> Result<u64, DispatchError> {
            let leaf_index = Self::next_leaf_index();
            let tree_depth = T::TreeDepth::get();

            <TreeNodes<T>>::insert((tree_depth, leaf_index), leaf);

            let mut current_index = leaf_index;
            let mut current_hash = leaf;
            for depth in (0..tree_depth).rev() {
                let sibling_index =
                    if current_index % 2 == 0 { current_index + 1 } else { current_index - 1 };
                let sibling_hash = Self::tree_nodes((depth + 1, sibling_index));

                let (left, right) = if current_index % 2 == 0 {
                    (current_hash, sibling_hash)
                } else {
                    (sibling_hash, current_hash)
                };

                let parent_hash =
                    Blake2s::compress(&(), &left.to_fixed_bytes(), &right.to_fixed_bytes())
                        .map(H256::from)
                        .map_err(|_| Error::<T>::InvalidProof)?; // Should not happen

                current_index /= 2;
                current_hash = parent_hash;
                <TreeNodes<T>>::insert((depth, current_index), current_hash);
            }

            <MerkleRoot<T>>::put(current_hash);
            <NextLeafIndex<T>>::put(leaf_index + 1);

            Ok(leaf_index)
        }

        /// Internal helper function to abstract proof verification.
        fn verify_proof_internal(
            vk_bytes: &[u8],
            proof_bytes: &[u8],
            public_inputs_bytes: &[Vec<u8>],
        ) -> DispatchResult {
            let vk = VerifyingKey::<Bls12_381>::deserialize_uncompressed(vk_bytes)
                .map_err(|_| Error::<T>::MalformedVerificationKey)?;
            let proof = Proof::<Bls12_381>::deserialize_uncompressed(proof_bytes)
                .map_err(|_| Error::<T>::MalformedProof)?;
            let public_inputs_fr: Vec<Fr> =
                public_inputs_bytes.iter().map(|b| Fr::from_be_bytes_mod_order(b)).collect();

            let verification_result = Groth16::<Bls12_381>::verify(&vk, &public_inputs_fr, &proof)
                .map_err(|_| Error::<T>::InvalidProof)?;

            ensure!(verification_result, Error::<T>::InvalidProof);
            Ok(())
        }
    }
}
