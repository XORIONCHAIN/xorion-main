#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement},
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::{ArithmeticError, traits::UniqueSaturatedInto};
    use sp_std::prelude::*;

    // Define the Balance type from the Currency trait
    type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

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

        /// The currency type for managing balances.
        type Currency: Currency<Self::AccountId>;
    }

    /// The origin that is allowed to perform administrative actions.
    #[pallet::storage]
    #[pallet::getter(fn owner)]
    pub type Owner<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// Storage item to track if the claiming process has been activated by the owner.
    #[pallet::storage]
    #[pallet::getter(fn is_activated)]
    pub type Activated<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// Storage map from an account ID to the balance they are entitled to claim.
    #[pallet::storage]
    #[pallet::getter(fn claims)]
    pub type Claims<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    /// Storage map to maintain the set of authorized relayers.
    #[pallet::storage]
    #[pallet::getter(fn relayers)]
    pub type Relayers<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, (), OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The claims process has been activated.
        ClaimsActivated,
        /// A new claim has been added for an account. [who, amount]
        ClaimAdded { who: T::AccountId, total_amount: BalanceOf<T>, rate: u128 },
        /// An account has successfully claimed their tokens. [who, amount]
        Claimed { who: T::AccountId, amount: BalanceOf<T> },
        /// A new relayer has been added. [who]
        RelayerAdded { who: T::AccountId },
        /// A relayer has been removed. [who]
        RelayerRemoved { who: T::AccountId },
        /// Exchange Rate Updated
        ExchangeRateUpdated(u128),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The claims process has not been activated yet.
        NotActivated,
        /// The claims process has already been activated.
        AlreadyActivated,
        /// The caller is not an authorized relayer.
        NotRelayer,
        /// The specified account is already a relayer.
        RelayerAlreadyExists,
        /// The specified account is not a relayer.
        NoSuchRelayer,
        /// A claim for the specified account already exists.
        InsufficientLaunchpadBalance,
        /// The user is trying to claim more than their available balance.
        InsufficientClaim,
        /// Not Owner
        NotOwner,
        /// No Owner,
        NoOwner,
    }

    /// Storage for the funding account ---
    #[pallet::storage]
    #[pallet::getter(fn funding_source)]
    pub type FundingSource<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// USDT price per XOR (with 6 decimals for USDT precision)
    /// Example: if 1 XOR = 0.05 USDT, then store 0.05 * 1e6 = 50_000
    #[pallet::storage]
    #[pallet::getter(fn exchange_rate)]
    pub type ExchangeRate<T> = StorageValue<_, u128, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub funding_source_account: Option<T::AccountId>,
        pub owner: Option<T::AccountId>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self { funding_source_account: None, owner: None }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            if let Some(ref funding_source_account) = self.funding_source_account {
                FundingSource::<T>::put(funding_source_account.clone());
            }
            if let Some(ref owner) = self.owner {
                Owner::<T>::put(owner.clone());
            }
            ExchangeRate::<T>::put(20);
        }
    }
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Activate the claims process. Can only be called once by the owner.
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn activate(origin: OriginFor<T>) -> DispatchResult {
            Self::ensure_owner(origin)?;
            ensure!(!Self::is_activated(), Error::<T>::AlreadyActivated);

            Activated::<T>::put(true);
            Self::deposit_event(Event::ClaimsActivated);
            Ok(())
        }

        /// Add a new relayer. Can only be called by the owner.
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn add_relayer(origin: OriginFor<T>, relayer_account: T::AccountId) -> DispatchResult {
            Self::ensure_owner(origin)?;
            ensure!(
                !Relayers::<T>::contains_key(&relayer_account),
                Error::<T>::RelayerAlreadyExists
            );

            Relayers::<T>::insert(&relayer_account, ());
            Self::deposit_event(Event::RelayerAdded { who: relayer_account });
            Ok(())
        }

        /// Remove an existing relayer. Can only be called by the owner.
        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn remove_relayer(
            origin: OriginFor<T>,
            relayer_account: T::AccountId,
        ) -> DispatchResult {
            Self::ensure_owner(origin)?;
            ensure!(Relayers::<T>::contains_key(&relayer_account), Error::<T>::NoSuchRelayer);

            Relayers::<T>::remove(&relayer_account);
            Self::deposit_event(Event::RelayerRemoved { who: relayer_account });
            Ok(())
        }

        /// Add a claim for a specific account. Can only be called by an authorized relayer.
        #[pallet::call_index(3)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn add_claim(
            origin: OriginFor<T>,
            who: T::AccountId,
            usdt_amount: u128,
        ) -> DispatchResult {
            let relayer = ensure_signed(origin)?;
            ensure!(Self::relayers(&relayer).is_some(), Error::<T>::NotRelayer);

            let rate = ExchangeRate::<T>::get();
            ensure!(rate > 0, "Exchange rate not set");

            // Convert: tokens = usdt_amount / rate
            // Scale USDT (6 decimals) to 18 decimals
            let usdt_normalized = usdt_amount.saturating_mul(10u128.pow(12)); // 6 → 18 decimals

            let tokens = usdt_normalized.checked_mul(rate).ok_or(ArithmeticError::Underflow)?;
            let current = Claims::<T>::get(&who);
            let new_total = current + tokens.unique_saturated_into();

            Claims::<T>::insert(&who, new_total);
            Self::deposit_event(Event::ClaimAdded { who, total_amount: new_total, rate });
            Ok(())
        }

        /// Claim the full amount available for the caller.
        #[pallet::call_index(4)]
        #[pallet::weight(T::DbWeight::get().reads_writes(3, 1))]
        pub fn claim_full(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(Self::is_activated(), Error::<T>::NotActivated);

            // Take the claim from storage, which removes it.
            let claimable_amount = Claims::<T>::take(&who);

            // Transfer funds from the pallet's account to the claimant.
            let source_account = Self::funding_source().ok_or(Error::<T>::NotActivated)?;

            ensure!(
                T::Currency::free_balance(&source_account) > claimable_amount,
                Error::<T>::InsufficientLaunchpadBalance
            );
            T::Currency::transfer(
                &source_account,
                &who,
                claimable_amount,
                ExistenceRequirement::KeepAlive,
            )?;

            Self::deposit_event(Event::Claimed { who, amount: claimable_amount });
            Ok(())
        }

        /// Claim a specific amount.
        #[pallet::call_index(5)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn claim(origin: OriginFor<T>, amount_to_claim: BalanceOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(Self::is_activated(), Error::<T>::NotActivated);

            // Mutate the claim in storage.
            Claims::<T>::try_mutate(&who, |claim_balance| -> DispatchResult {
                let current_claim = *claim_balance;
                ensure!(amount_to_claim <= current_claim, Error::<T>::InsufficientClaim);

                // Transfer funds from the source account.
                let source_account = Self::funding_source().ok_or(Error::<T>::NotActivated)?;
                ensure!(
                    T::Currency::free_balance(&source_account) > amount_to_claim,
                    Error::<T>::InsufficientLaunchpadBalance
                );
                T::Currency::transfer(
                    &source_account,
                    &who,
                    amount_to_claim,
                    ExistenceRequirement::KeepAlive,
                )?;

                let new_claim = current_claim - amount_to_claim;

                // If the remaining balance is zero, remove the entry. Otherwise, update it.
                *claim_balance = new_claim;
                Self::deposit_event(Event::Claimed { who: who.clone(), amount: amount_to_claim });
                Ok(())
            })
        }

        /// Update exchange rate (only owner)
        #[pallet::call_index(6)]
        #[pallet::weight(T::DbWeight::get().reads_writes(0, 1))]
        pub fn set_exchange_rate(origin: OriginFor<T>, new_rate: u128) -> DispatchResult {
            Self::ensure_owner(origin)?;
            ExchangeRate::<T>::put(new_rate);
            Self::deposit_event(Event::ExchangeRateUpdated(new_rate));
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn ensure_owner(origin: T::RuntimeOrigin) -> Result<T::AccountId, DispatchError> {
            let who = ensure_signed(origin)?;
            let owner = Owner::<T>::get().ok_or(Error::<T>::NoOwner)?;
            ensure!(who == owner, Error::<T>::NotOwner);
            Ok(who)
        }
    }
}
