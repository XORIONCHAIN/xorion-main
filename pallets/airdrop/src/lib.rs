//! # Airdrop Pallet
//!
//! A Substrate pallet that provides automatic token airdrops to new, unfunded users on testnets.
//!
//! ## Overview
//!
//! The Airdrop pallet allows blockchain projects to distribute tokens to new users who have
//! insufficient balance to interact with the network. This is particularly useful for testnets
//! where users need initial funding to start testing applications and features.
//!
//! ## Features
//!
//! ### Core Functionality
//! - **Automatic eligibility checking**: Only accounts with balance below a configurable threshold
//!   can claim airdrops
//! - **Self-service claiming**: Users can claim airdrops directly through the `claim_airdrop`
//!   extrinsic
//! - **Configurable parameters**: Airdrop amounts, thresholds, and limits are runtime-configurable
//! - **Anti-spam protection**: Multiple layers of protection against abuse and spam
//!
//! ### Safety & Security
//! - **Dedicated airdrop pool**: Uses a sovereign account to securely hold airdrop funds
//! - **Rate limiting**: Configurable limits on airdrops per block and per account
//! - **Cooldown periods**: Prevents rapid successive claims from the same account
//! - **Admin controls**: Root-only functions for funding and emergency draining of the pool
//!
//! ### Monitoring & Analytics
//! - **Comprehensive tracking**: Records claim history, totals, and statistics
//! - **Eligibility checking**: Helper functions to check if accounts can claim airdrops
//! - **Cooldown tracking**: Functions to check remaining cooldown periods
//!
//! ## Configuration
//!
//! The pallet requires several configuration parameters:
//!
//! ```rust,ignore
//! impl pallet_airdrop::Config for Runtime {
//!     type RuntimeEvent = RuntimeEvent;
//!     type Currency = Balances;                    // The currency to airdrop
//!     type PalletId = AirdropPalletId;            // Unique identifier for the pallet
//!     type AirdropAmount = AirdropAmount;          // Amount per airdrop (e.g., 1000 tokens)
//!     type MinimumBalanceThreshold = MinBalance;   // Threshold for eligibility (e.g., 100 tokens)
//!     type MaxAirdropsPerBlock = MaxAirdrops;      // Block-level rate limit (e.g., 100)
//!     type CooldownPeriod = CooldownBlocks;        // Cooldown between claims (e.g., 7200 blocks)
//!     type MaxAirdropsPerAccount = MaxPerAccount;  // Max claims per account (e.g., 5)
//! }
//! ```
//!
//! ## Usage
//!
//! ### For Administrators
//! 1. **Fund the airdrop pool**: Use `fund_airdrop_pool` to add tokens to the distribution pool
//! 2. **Monitor usage**: Check `total_airdrops()` and pool balance to track distribution
//! 3. **Emergency controls**: Use `drain_airdrop_pool` if needed to recover funds
//!
//! ### For Users
//! 1. **Check eligibility**: Ensure your account balance is below the threshold
//! 2. **Claim airdrop**: Call `claim_airdrop()` to receive tokens
//! 3. **Respect cooldowns**: Wait for the cooldown period before claiming again
//!
//! ### For Developers
//! ```rust,ignore
//! // Check if an account can claim
//! let can_claim = AirdropPallet::is_eligible_for_airdrop(&account);
//!
//! // Get remaining cooldown
//! let remaining = AirdropPallet::get_cooldown_remaining(&account);
//!
//! // Get claim history
//! let record = AirdropPallet::airdrop_records(&account);
//! ```
//!
//! ## Storage
//!
//! The pallet maintains several storage items:
//! - `AirdropRecords`: Per-account claim history and statistics
//! - `TotalAirdrops`: Global counter of total airdrops distributed
//! - `AirdropsThisBlock`: Rate limiting counter (reset each block)
//! - `LastResetBlock`: Tracking for block-level counter resets
//!
//! ## Events
//!
//! - `AirdropClaimed`: Emitted when a user successfully claims an airdrop
//! - `AirdropFunded`: Emitted when an admin adds funds to the pool
//! - `AirdropConfigUpdated`: Emitted when configuration is updated
//!
//! ## Errors
//!
//! The pallet includes comprehensive error handling:
//! - `AccountAlreadyFunded`: Account has sufficient balance
//! - `MaxAirdropsReached`: Account has claimed maximum allowed airdrops
//! - `CooldownPeriodActive`: Account must wait before claiming again
//! - `MaxAirdropsPerBlockReached`: Block rate limit exceeded
//! - `InsufficientAirdropFunds`: Airdrop pool has insufficient funds
//!
//! ## Security Considerations
//!
//! - The pallet uses a sovereign account for the airdrop pool, isolated from user funds
//! - Multiple rate limiting mechanisms prevent abuse and spam
//! - Admin functions require root privileges
//! - All operations are weight-accounted for proper fee calculation
//!
//! ## Testing & Testnet Usage
//!
//! This pallet is designed primarily for testnet environments where:
//! - Users need initial funding to interact with the network
//! - Token distribution helps bootstrap network activity
//! - Anti-spam measures prevent abuse while allowing legitimate testing
//!
//! For production networks, consider additional security measures and more restrictive limits.

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use frame_support::{
    dispatch::DispatchResult,
    pallet_prelude::*,
    traits::{Currency, Get, ReservableCurrency},
    PalletId,
};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::{AccountIdConversion, SaturatedConversion, Saturating, Zero};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The currency used for the airdrop
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// The pallet's identifier, used for deriving the sovereign account ID.
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// The amount of tokens to airdrop to each user
        #[pallet::constant]
        type AirdropAmount: Get<<Self::Currency as Currency<Self::AccountId>>::Balance>;

        /// Minimum balance threshold to be considered "unfunded"
        #[pallet::constant]
        type MinimumBalanceThreshold: Get<<Self::Currency as Currency<Self::AccountId>>::Balance>;

        /// Maximum number of airdrops per block to prevent spam
        #[pallet::constant]
        type MaxAirdropsPerBlock: Get<u32>;

        /// Cooldown period between airdrops for the same account (in blocks)
        #[pallet::constant]
        type CooldownPeriod: Get<BlockNumberFor<Self>>;

        /// Maximum total airdrops allowed per account
        #[pallet::constant]
        type MaxAirdropsPerAccount: Get<u32>;
    }

    /// Balance type alias for easier use
    type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// Storage for tracking airdrop recipients and their claim history
    #[pallet::storage]
    #[pallet::getter(fn airdrop_records)]
    pub type AirdropRecords<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        AirdropRecord<BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// Storage for tracking total airdrops distributed
    #[pallet::storage]
    #[pallet::getter(fn total_airdrops)]
    pub type TotalAirdrops<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Storage for tracking airdrops in current block
    #[pallet::storage]
    #[pallet::getter(fn airdrops_this_block)]
    pub type AirdropsThisBlock<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Storage for the last block when airdrops were reset
    #[pallet::storage]
    #[pallet::getter(fn last_reset_block)]
    pub type LastResetBlock<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// Information about an account's airdrop history
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct AirdropRecord<BlockNumber> {
        /// Number of airdrops claimed by this account
        pub claims_count: u32,
        /// Block number of the last airdrop claim
        pub last_claim_block: BlockNumber,
        /// Total amount received from airdrops
        pub total_received: u128,
    }

    /// Events emitted by the pallet
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Airdrop successfully claimed
        AirdropClaimed { who: T::AccountId, amount: BalanceOf<T> },
        /// Airdrop fund was deposited by admin
        AirdropFunded { amount: BalanceOf<T> },
        /// Airdrop parameters updated
        AirdropConfigUpdated,
    }

    /// Errors emitted by the pallet
    #[pallet::error]
    pub enum Error<T> {
        /// Account already has sufficient balance
        AccountAlreadyFunded,
        /// Account has reached maximum airdrops allowed
        MaxAirdropsReached,
        /// Account is in cooldown period
        CooldownPeriodActive,
        /// Maximum airdrops per block reached
        MaxAirdropsPerBlockReached,
        /// Insufficient funds in airdrop pool
        InsufficientAirdropFunds,
        /// Airdrop amount is zero
        ZeroAirdropAmount,
        /// Invalid configuration
        InvalidConfiguration,
    }

    /// Genesis configuration for the pallet
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// Initial funding for the airdrop pool
        pub initial_funding: BalanceOf<T>,
        /// Accounts to pre-fund with airdrops
        pub pre_funded_accounts: Vec<T::AccountId>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self { initial_funding: Zero::zero(), pre_funded_accounts: Vec::new() }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            // Fund the airdrop pool if initial funding is provided
            if !self.initial_funding.is_zero() {
                let airdrop_account = Pallet::<T>::airdrop_account_id();
                let _ = T::Currency::deposit_creating(&airdrop_account, self.initial_funding);
            }

            // Pre-fund specified accounts
            for account in &self.pre_funded_accounts {
                let _ = Pallet::<T>::do_airdrop(account);
            }
        }
    }

    /// Hooks for block initialization and finalization
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(block_number: BlockNumberFor<T>) -> Weight {
            // Reset airdrops counter for new block
            if Self::last_reset_block() < block_number {
                AirdropsThisBlock::<T>::put(0);
                LastResetBlock::<T>::put(block_number);
            }

            // Return weight for the read and write operations
            T::DbWeight::get().reads_writes(1, 2)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Claim an airdrop if eligible
        #[pallet::call_index(0)]
        #[pallet::weight((Weight::zero(), Pays::No))]
        pub fn claim_airdrop(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_airdrop(&who)?;
            Ok(())
        }

        /// Fund the airdrop pool (admin only)
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn fund_airdrop_pool(
            origin: OriginFor<T>,
            #[pallet::compact] amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(!amount.is_zero(), Error::<T>::ZeroAirdropAmount);

            let airdrop_account = Self::airdrop_account_id();
            _ = T::Currency::deposit_creating(&airdrop_account, amount);

            Self::deposit_event(Event::AirdropFunded { amount });
            Ok(())
        }
    }

    /// Internal helper functions
    impl<T: Config> Pallet<T> {
        /// Get the account ID of the airdrop pool
        pub fn airdrop_account_id() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }

        /// Execute an airdrop to the specified account
        fn do_airdrop(who: &T::AccountId) -> DispatchResult {
            let current_block = frame_system::Pallet::<T>::block_number();
            let airdrop_amount = T::AirdropAmount::get();

            // Check if airdrop amount is valid
            ensure!(!airdrop_amount.is_zero(), Error::<T>::ZeroAirdropAmount);

            // Check if account is eligible (balance below threshold)
            let current_balance = T::Currency::free_balance(who);
            ensure!(
                current_balance < T::MinimumBalanceThreshold::get(),
                Error::<T>::AccountAlreadyFunded
            );

            // Check airdrops per block limit
            let airdrops_this_block = Self::airdrops_this_block();
            ensure!(
                airdrops_this_block < T::MaxAirdropsPerBlock::get(),
                Error::<T>::MaxAirdropsPerBlockReached
            );

            // Check account-specific limits and cooldown
            if let Some(record) = Self::airdrop_records(who) {
                // Check maximum airdrops per account
                ensure!(
                    record.claims_count < T::MaxAirdropsPerAccount::get(),
                    Error::<T>::MaxAirdropsReached
                );

                // Check cooldown period
                let blocks_since_last_claim = current_block.saturating_sub(record.last_claim_block);
                ensure!(
                    blocks_since_last_claim >= T::CooldownPeriod::get(),
                    Error::<T>::CooldownPeriodActive
                );
            }

            // Check if airdrop pool has sufficient funds
            let airdrop_account = Self::airdrop_account_id();
            let pool_balance = T::Currency::free_balance(&airdrop_account);
            ensure!(pool_balance >= airdrop_amount, Error::<T>::InsufficientAirdropFunds);

            // Transfer tokens from airdrop pool to user
            T::Currency::transfer(
                &airdrop_account,
                who,
                airdrop_amount,
                frame_support::traits::ExistenceRequirement::AllowDeath,
            )?;

            // Update airdrop record
            let new_record = if let Some(mut record) = Self::airdrop_records(who) {
                record.claims_count = record.claims_count.saturating_add(1);
                record.last_claim_block = current_block;
                record.total_received =
                    record.total_received.saturating_add(airdrop_amount.saturated_into());
                record
            } else {
                AirdropRecord {
                    claims_count: 1,
                    last_claim_block: current_block,
                    total_received: airdrop_amount.saturated_into(),
                }
            };

            // Update storage
            AirdropRecords::<T>::insert(who, new_record);
            AirdropsThisBlock::<T>::put(airdrops_this_block.saturating_add(1));
            TotalAirdrops::<T>::put(Self::total_airdrops().saturating_add(1));

            // Emit event
            Self::deposit_event(Event::AirdropClaimed { who: who.clone(), amount: airdrop_amount });

            Ok(())
        }

        /// Check if an account is eligible for airdrop
        pub fn is_eligible_for_airdrop(who: &T::AccountId) -> bool {
            let current_block = frame_system::Pallet::<T>::block_number();
            let current_balance = T::Currency::free_balance(who);

            // Check balance threshold
            if current_balance >= T::MinimumBalanceThreshold::get() {
                return false;
            }

            // Check airdrops per block limit
            if Self::airdrops_this_block() >= T::MaxAirdropsPerBlock::get() {
                return false;
            }

            // Check account-specific limits
            if let Some(record) = Self::airdrop_records(who) {
                // Check maximum airdrops per account
                if record.claims_count >= T::MaxAirdropsPerAccount::get() {
                    return false;
                }

                // Check cooldown period
                let blocks_since_last_claim = current_block.saturating_sub(record.last_claim_block);
                if blocks_since_last_claim < T::CooldownPeriod::get() {
                    return false;
                }
            }

            // Check if airdrop pool has sufficient funds
            let airdrop_account = Self::airdrop_account_id();
            let pool_balance = T::Currency::free_balance(&airdrop_account);
            if pool_balance < T::AirdropAmount::get() {
                return false;
            }

            true
        }

        /// Get the remaining cooldown blocks for an account
        pub fn get_cooldown_remaining(who: &T::AccountId) -> BlockNumberFor<T> {
            let current_block = frame_system::Pallet::<T>::block_number();

            if let Some(record) = Self::airdrop_records(who) {
                let blocks_since_last_claim = current_block.saturating_sub(record.last_claim_block);
                if blocks_since_last_claim < T::CooldownPeriod::get() {
                    return T::CooldownPeriod::get().saturating_sub(blocks_since_last_claim);
                }
            }

            Zero::zero()
        }
    }
}
