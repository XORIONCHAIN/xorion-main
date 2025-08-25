#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;
const MAX_RELAYERS: u32 = 100;
#[frame_support::pallet]
pub mod pallet {
    use super::MAX_RELAYERS;
    use frame_support::{
        PalletId,
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement::AllowDeath},
    };
    use frame_system::pallet_prelude::*;
    use sp_core::{H160, keccak_256};
    use sp_io::crypto::secp256k1_ecdsa_recover;
    use sp_runtime::traits::{AccountIdConversion, SaturatedConversion, Saturating};
    use sp_std::vec::Vec;

    /// Locked message info stored per message id
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct LockedInfo<AccountId, Balance> {
        pub owner: AccountId,     // who locked the funds on Substrate
        pub amount: Balance,      // amount locked (native token)
        pub relayer_fee: Balance, // relayer fee attached to this lock (may be zero)
        pub eth_recipient: H160,  // Ethereum recipient address originally provided
        pub nonce: u64,           // nonce provided by locker (to avoid collisions)
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Native currency (pallet-balances).
        type Currency: Currency<Self::AccountId>;

        /// Pallet id -> used to derive sovereign account that stores locked funds.
        #[pallet::constant]
        type BridgePalletId: Get<PalletId>;

        /// Relayer signatures threshold (K-of-N).
        #[pallet::constant]
        type RelayerThreshold: Get<u32>;

        /// Maximum number of signatures accepted in a single release call (to bound weight).
        #[pallet::constant]
        type MaxSignatures: Get<u32>;
    }

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    // Pallet storage
    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// Mapping of relayer Ethereum addresses (H160). Root-settable.
    #[pallet::storage]
    #[pallet::getter(fn relayers)]
    pub(super) type Relayers<T: Config> =
        StorageValue<_, BoundedVec<H160, ConstU32<{ MAX_RELAYERS }>>, ValueQuery>;

    /// Mapping message_id -> LockedInfo (only for Substrate->Ethereum locks).
    #[pallet::storage]
    #[pallet::getter(fn locked)]
    pub(super) type LockedMessages<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        [u8; 32],
        LockedInfo<T::AccountId, BalanceOf<T>>,
        OptionQuery,
    >;

    /// Processed message ids (prevents replays for releases coming from Ethereum side).
    #[pallet::storage]
    #[pallet::getter(fn processed)]
    pub(super) type ProcessedMessages<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], bool, ValueQuery>;

    /// Total amount of native assets locked for bridging to Ethereum.
    #[pallet::storage]
    #[pallet::getter(fn total_locked)]
    pub(super) type TotalLocked<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// Total amount of native assets released on this chain from Ethereum.
    #[pallet::storage]
    #[pallet::getter(fn total_released)]
    pub(super) type TotalReleased<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// Paused flag (owner can pause emergency).
    #[pallet::storage]
    #[pallet::getter(fn paused)]
    pub(super) type Paused<T: Config> = StorageValue<_, bool, ValueQuery>;

    // Events
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Funds locked by a user for bridging to ETH.
        /// (who, amount, relayer_fee, eth_recipient, nonce, message_id)
        Locked(T::AccountId, BalanceOf<T>, BalanceOf<T>, H160, u64, [u8; 32]),

        /// Funds released on Substrate (recipient got amount).
        /// (recipient, amount, message_id, number of valid signatures)
        /// (note: message_id is 32-byte hash of message on Ethereum side, not the
        /// canonicalized message id emitted by Ethereum
        Released(T::AccountId, BalanceOf<T>, [u8; 32], u32),

        /// Relayer reimbursed for finalizing a release.
        /// (relayer, amount)
        RelayerReimbursed(T::AccountId, BalanceOf<T>),

        /// Relayers list updated
        RelayersUpdated(Vec<H160>),

        /// Relayer fund topped up
        RelayerFundToppedUp(BalanceOf<T>),

        /// Emergency withdraw executed by admin
        EmergencyWithdraw(T::AccountId, BalanceOf<T>),

        /// Paused/unpaused toggles
        PausedSet(bool),
    }

    // Errors
    #[pallet::error]
    pub enum Error<T> {
        /// Not enough free balance to lock.
        InsufficientBalance,
        /// No locked entry found for message id / recipient.
        NoLockedEntry,
        /// Requested release amount exceeds locked amount.
        InsufficientLockedAmount,
        /// Message already processed (replay).
        MessageAlreadyProcessed,
        /// Provided signatures did not meet threshold or invalid.
        ThresholdNotMet,
        /// Signature malformed or recovery failed.
        InvalidSignature,
        /// Paused
        Paused,
        /// Too many signatures provided
        TooManySignatures,
        /// Overflow on arithmetic (should not happen with saturating ops).
        Overflow,
        /// Caller not root for admin action
        NotAuthorized,
        /// Relayer fund insufficient
        RelayerFundInsufficient,
        /// TooManyRelayers
        TooManyRelayers,
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub relayers: Vec<H160>,
        pub _phantom: PhantomData<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self { relayers: Default::default(), _phantom: Default::default() }
        }
    }
    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            let bounded_relayers: BoundedVec<H160, ConstU32<MAX_RELAYERS>> =
                self.relayers.clone().try_into().unwrap();

            Relayers::<T>::put(&bounded_relayers);
        }
    }

    // Dispatchable functions
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// User locks native tokens for bridging to Ethereum.
        /// `amount` is the native token amount to lock.
        /// `eth_recipient` is the 20-byte ethereum recipient (H160).
        /// `relayer_fee` is the portion reserved to reimburse the relayer (may be zero).
        /// `nonce` is any user-chosen nonce to avoid message collisions (recommended).
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(10,3))]
        pub fn lock(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
            relayer_fee: BalanceOf<T>,
            eth_recipient: H160,
            nonce: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!Self::is_paused(), Error::<T>::Paused);
            ensure!(amount > Zero::zero(), Error::<T>::InsufficientBalance);

            // Ensure caller has enough free balance for amount + relayer_fee
            let total = amount.saturating_add(relayer_fee);
            let free = T::Currency::free_balance(&who);
            ensure!(free >= total, Error::<T>::InsufficientBalance);

            // Transfer total into pallet account
            let pallet_acct = Self::account_id();
            T::Currency::transfer(&who, &pallet_acct, total, AllowDeath)?;

            // Compute canonical message id:
            // keccak256(chain_id || direction || amount_u128 || substrate_sender_scale ||
            // eth_recipient || nonce) Use chain_id = 1, direction = 0 for
            // Substrate->Ethereum per earlier convention.
            let chain_id: u64 = 1u64;
            let direction: u8 = 0u8;
            let amount_u128 = Self::balance_to_u128(&amount)?;
            let mut enc: Vec<u8> = Vec::new();
            enc.extend_from_slice(&chain_id.to_be_bytes());
            enc.extend_from_slice(&direction.to_be_bytes());
            enc.extend_from_slice(&amount_u128.to_be_bytes());
            enc.extend_from_slice(&who.encode());
            enc.extend_from_slice(eth_recipient.as_bytes());
            enc.extend_from_slice(&nonce.to_be_bytes());
            let id = keccak_256(&enc);

            // Store locked info; if entry exists with same id, fail to avoid overwrite
            ensure!(!LockedMessages::<T>::contains_key(id), Error::<T>::Overflow);

            let li = LockedInfo { owner: who.clone(), amount, relayer_fee, eth_recipient, nonce };
            LockedMessages::<T>::insert(id, li);

            TotalLocked::<T>::mutate(|total| *total = total.saturating_add(amount));

            Self::deposit_event(Event::Locked(who, amount, relayer_fee, eth_recipient, nonce, id));
            Ok(())
        }

        /// Release locked native tokens on Substrate after verifying K-of-N relayer signatures over
        /// the message id. `message_id` is the 32-byte message identifier (as emitted by
        /// Ethereum or canonicalized on ETH side). `recipient` will receive the unlocked
        /// native tokens. `amount` expected amount to release (must be <= locked amount).
        /// `signatures` Vec<Vec<u8>> — each signature is 65 bytes r||s||v (v = 27/28 or 0/1).
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_all(10_000) + T::DbWeight::get().reads_writes(2,3))]
        pub fn release(
            origin: OriginFor<T>,
            message_id: [u8; 32],
            recipient: T::AccountId,
            amount: BalanceOf<T>,
            signatures: Vec<Vec<u8>>,
        ) -> DispatchResult {
            let _submitter = ensure_signed(origin)?;
            ensure!(!Self::is_paused(), Error::<T>::Paused);

            // Check processed
            ensure!(!ProcessedMessages::<T>::get(message_id), Error::<T>::MessageAlreadyProcessed);
            // Validate number of signatures
            let sig_count = signatures.len() as u32;
            ensure!(sig_count <= T::MaxSignatures::get(), Error::<T>::TooManySignatures);

            // Verify signatures: recover H160 and count unique valid relayers
            let relayers = Relayers::<T>::get();
            let thresh = T::RelayerThreshold::get();
            let mut seen: Vec<H160> = Vec::new();
            let mut valid: u32 = 0;

            for sig in signatures.iter() {
                // signature must be 65 bytes
                if sig.len() != 65 {
                    continue;
                }
                match Self::ecdsa_recover_h160(sig.as_slice(), &message_id) {
                    Ok(addr) =>
                        if relayers.contains(&addr) && !seen.contains(&addr) {
                            seen.push(addr);
                            valid = valid.saturating_add(1);
                        },
                    Err(_) => {
                        // ignore invalid signature and continue; final check below ensures
                        // threshold
                        continue;
                    },
                }
            }

            ensure!(valid >= thresh, Error::<T>::ThresholdNotMet);

            // Transfer amount from pallet account to recipient
            let pallet_acct = Self::account_id();

            // Double-check that pallet account has balance (should, since locked was previously
            // transferred)
            let pallet_balance = T::Currency::free_balance(&pallet_acct);
            ensure!(pallet_balance >= amount, Error::<T>::InsufficientLockedAmount);

            T::Currency::transfer(&pallet_acct, &recipient, amount, AllowDeath)?;

            // mark processed to avoid replays
            ProcessedMessages::<T>::insert(message_id, true);

            // total released amount
            TotalReleased::<T>::mutate(|total| *total = total.saturating_add(amount));

            Self::deposit_event(Event::Released(recipient.clone(), amount, message_id, valid));

            Ok(())
        }

        /// Admin: set relayer list (root)
        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1,3))]
        pub fn set_relayers(origin: OriginFor<T>, relayers: Vec<H160>) -> DispatchResult {
            ensure_root(origin)?;
            let bounded_relayers: BoundedVec<H160, ConstU32<MAX_RELAYERS>> =
                relayers.clone().try_into().map_err(|_| Error::<T>::TooManyRelayers)?;

            Relayers::<T>::put(&bounded_relayers);
            Self::deposit_event(Event::RelayersUpdated(relayers));
            Ok(())
        }

        /// Admin: top up the RelayerFund (owner/root) by transferring from caller to pallet account
        #[pallet::call_index(3)]
        #[pallet::weight(T::DbWeight::get().reads_writes(2,3))]
        pub fn top_up_relayer_fund(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(amount > Zero::zero(), Error::<T>::InsufficientBalance);
            let pallet_acct = Self::account_id();
            T::Currency::transfer(&who, &pallet_acct, amount, AllowDeath)?;
            Self::deposit_event(Event::RelayerFundToppedUp(amount));
            Ok(())
        }

        /// Admin: emergency withdraw some native tokens from the pallet account to an address
        /// (root)
        #[pallet::call_index(4)]
        #[pallet::weight(T::DbWeight::get().reads_writes(2,23))]
        pub fn emergency_withdraw(
            origin: OriginFor<T>,
            to: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let pallet_acct = Self::account_id();
            // ensure pallet has enough
            let bal = T::Currency::free_balance(&pallet_acct);
            ensure!(bal >= amount, Error::<T>::InsufficientBalance);
            T::Currency::transfer(&pallet_acct, &to, amount, AllowDeath)?;

            Self::deposit_event(Event::EmergencyWithdraw(to, amount));
            Ok(())
        }

        /// Admin: pause/unpause bridge operations (root)
        #[pallet::weight(T::DbWeight::get().reads_writes(1,13))]
        #[pallet::call_index(5)]
        pub fn set_paused(origin: OriginFor<T>, paused: bool) -> DispatchResult {
            ensure_root(origin)?;
            Paused::<T>::put(paused);
            Self::deposit_event(Event::PausedSet(paused));
            Ok(())
        }
    }

    // Implementation details
    impl<T: Config> Pallet<T> {
        /// Derived pallet account id.
        pub fn account_id() -> T::AccountId {
            T::BridgePalletId::get().into_account_truncating()
        }

        /// Convenience: check paused flag
        pub fn is_paused() -> bool {
            Paused::<T>::get()
        }

        /// Convert BalanceOf<T> -> u128 for canonical hashing / encoding.
        /// Assumes Balance fits within u128 (common). If your runtime uses larger types adapt
        /// accordingly.
        pub fn balance_to_u128(b: &BalanceOf<T>) -> Result<u128, Error<T>> {
            // saturated_into will not panic; we treat values > u128::MAX as overflow error
            let v: u128 = (*b).saturated_into::<u128>();
            Ok(v)
        }

        /// Recover Ethereum-style ECDSA signer H160 from signature and message id (32 bytes).
        /// Expects a 65-byte signature (r||s||v) where v is 27/28 or 0/1.
        pub fn ecdsa_recover_h160(sig: &[u8], message_id: &[u8; 32]) -> Result<H160, Error<T>> {
            if sig.len() != 65 {
                return Err(Error::<T>::InvalidSignature);
            }
            // 1. Construct the prefixed message
            let mut prefixed_message = Vec::new();
            prefixed_message.extend_from_slice(b"\x19Ethereum Signed Message:\n32");
            prefixed_message.extend_from_slice(message_id);

            // 2. Hash the prefixed message
            let final_hash = keccak_256(&prefixed_message);

            let mut sig_arr = [0u8; 65];
            sig_arr.copy_from_slice(&sig[0..65]);
            // Note: secp256k1_ecdsa_recover expects a 32-byte message. We pass the raw message_id.
            match secp256k1_ecdsa_recover(&sig_arr, &final_hash) {
                Ok(pubkey) => {
                    let hash = keccak_256(&pubkey);
                    let mut h160 = H160::default();
                    h160.as_bytes_mut().copy_from_slice(&hash[12..32]);
                    Ok(h160)
                },
                Err(_) => Err(Error::<T>::InvalidSignature),
            }
        }
    }
}
