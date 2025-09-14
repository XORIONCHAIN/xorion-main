#![cfg_attr(not(feature = "std"), no_std, no_main)]
#![allow(clippy::cast_possible_truncation)]

#[ink::contract]
mod bridge {
    use ink::{prelude::vec::Vec, scale, storage::Mapping};

    #[ink(storage)]
    pub struct Bridge {
        owner: AccountId,
        /// Trusted relayers (Substrate accounts).
        is_relayer: Mapping<AccountId, bool>,
        /// Threshold of unique relayers required.
        relayer_threshold: u64,
        /// Prevent replay: track processed messages.
        processed_messages: Mapping<Hash, bool>,
        /// Track approvals: message_id -> approved relayers
        approvals: Mapping<Hash, AccountId>,
        /// Emergency pause
        paused: bool,
    }

    #[ink(event)]
    pub struct Locked {
        #[ink(topic)]
        from: AccountId,
        to: Vec<u8>,
        amount: Balance,
        #[ink(topic)]
        message_id: Hash,
    }

    #[ink(event)]
    pub struct Released {
        #[ink(topic)]
        to: AccountId,
        amount: Balance,
        #[ink(topic)]
        message_id: Hash,
    }

    #[ink(event)]
    pub struct OwnershipTransferred {
        #[ink(topic)]
        previous_owner: AccountId,
        #[ink(topic)]
        new_owner: AccountId,
    }

    impl Bridge {
        #[ink(constructor)]
        pub fn new(initial_relayers: Vec<AccountId>, relayer_threshold: u64) -> Self {
            assert!(
                relayer_threshold > 0 && relayer_threshold <= initial_relayers.len() as u64,
                "Invalid threshold"
            );
            let mut is_relayer = Mapping::default();
            for rel in &initial_relayers {
                is_relayer.insert(rel, &true);
            }
            Self {
                owner: Self::env().caller(),
                is_relayer,
                relayer_threshold,
                processed_messages: Mapping::default(),
                approvals: Mapping::default(),
                paused: false,
            }
        }

        // --- Read Methods ---

        /// Returns the owner of the contract.
        #[ink(message)]
        pub fn get_owner(&self) -> AccountId {
            self.owner
        }

        /// Returns true if the given account is a relayer.
        #[ink(message)]
        pub fn is_relayer(&self, account: AccountId) -> bool {
            self.is_relayer.get(account).unwrap_or(false)
        }

        /// Returns the current relayer threshold.
        #[ink(message)]
        pub fn get_relayer_threshold(&self) -> u64 {
            self.relayer_threshold
        }

        /// Returns true if the given message_id has been processed.
        #[ink(message)]
        pub fn is_message_processed(&self, message_id: Hash) -> bool {
            self.processed_messages.get(message_id).unwrap_or(false)
        }

        /// Returns whether the contract is paused.
        #[ink(message)]
        pub fn is_paused(&self) -> bool {
            self.paused
        }

        /// Returns the current contract balance.
        #[ink(message)]
        pub fn get_contract_balance(&self) -> Balance {
            self.env().balance()
        }

        // --- Write Methods ---

        /// Lock native tokens for release on Ethereum.
        #[ink(message, payable)]
        pub fn lock(&mut self, xorion_recipient: Vec<u8>) {
            self.ensure_not_paused();
            let caller = self.env().caller();
            let amount = self.env().transferred_value();
            assert!(amount > 0, "Amount must be > 0");

            let nonce = self.env().block_number();
            let encoded = (1u64, 1u8, amount, caller, xorion_recipient.clone(), nonce);
            let message_id = self.keccak256_encoded(&encoded);

            self.env().emit_event(Locked {
                from: caller,
                to: xorion_recipient,
                amount,
                message_id,
            });
        }

        /// Relayers call this to approve a release.
        #[ink(message)]
        pub fn approve_release(&mut self, message_id: Hash, to: AccountId, amount: Balance) {
            self.ensure_not_paused();
            let caller = self.env().caller();
            let is_rel = self.is_relayer.get(caller).unwrap_or(false);
            assert!(is_rel, "Not a relayer");
            let already_done = self.processed_messages.get(message_id).unwrap_or(false);
            assert!(!already_done, "Message already processed");

            // Mark processed and release funds
            self.processed_messages.insert(message_id, &true);
            assert!(self.env().balance() >= amount, "Insufficient funds");
            self.env().transfer(to, amount).expect("Transfer failed");
            self.env().emit_event(Released { to, amount, message_id });
        }

        /// Owner can update relayers.
        #[ink(message)]
        pub fn update_relayers(&mut self, relayers: Vec<AccountId>, status: Vec<bool>) {
            self.ensure_owner();
            assert_eq!(relayers.len(), status.len(), "Mismatched input");
            for (i, r) in relayers.iter().enumerate() {
                self.is_relayer.insert(r, &status[i]);
            }
        }

        #[ink(message)]
        pub fn set_relayer_threshold(&mut self, new_threshold: u64) {
            self.ensure_owner();
            assert!(new_threshold > 0, "Threshold must be > 0");
            self.relayer_threshold = new_threshold;
        }

        #[ink(message)]
        pub fn set_paused(&mut self, p: bool) {
            self.ensure_owner();
            self.paused = p;
        }

        /// Transfer ownership to a new account.
        #[ink(message)]
        pub fn transfer_ownership(&mut self, new_owner: AccountId) {
            self.ensure_owner();
            let prev = self.owner;
            self.owner = new_owner;
            self.env().emit_event(OwnershipTransferred { previous_owner: prev, new_owner });
        }

        #[ink(message)]
        pub fn self_destruct(&mut self, beneficiary: AccountId) {
            self.ensure_owner(); // only the owner can destroy
            self.env().terminate_contract(beneficiary);
        }

        // --- Internal Helpers ---

        fn ensure_owner(&self) {
            assert_eq!(self.env().caller(), self.owner, "Only owner");
        }

        fn ensure_not_paused(&self) {
            assert!(!self.paused, "Paused");
        }

        fn keccak256_encoded<T: scale::Encode>(&self, data: &T) -> Hash {
            use ink::env::hash::{HashOutput, Keccak256};
            let mut out = <Keccak256 as HashOutput>::Type::default();
            ink::env::hash_bytes::<Keccak256>(&data.encode(), &mut out);
            Hash::from(out)
        }
    }
}
