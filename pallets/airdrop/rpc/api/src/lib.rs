#![cfg_attr(not(feature = "std"), no_std)]
use codec::Codec;

// Runtime API trait that needs to be implemented in the runtime
sp_api::decl_runtime_apis! {
    pub trait AirdropApi<AccountId, Balance, BlockNumber> where
        AccountId: Codec,
        Balance: Codec,
        BlockNumber: Codec,
    {
        /// Check if an account is eligible for airdrop
        fn is_eligible_for_airdrop(who: AccountId) -> bool;

        /// Get the remaining cooldown blocks for an account
        fn get_cooldown_remaining(who: AccountId) -> BlockNumber;

        /// Get airdrop pool balance
        fn get_airdrop_pool_balance() -> Balance;
    }
}
