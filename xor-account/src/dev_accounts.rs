use crate::AccountId;
use alloc::{vec, vec::Vec};

pub enum DevAccounts {
    Alice,
    Bob,
    Dave,
    Eve,
    Ferdie,
    Charlie,
    AliceStash,
    BobStash,
}

impl DevAccounts {
    pub fn to_account_id(self) -> AccountId {
        match self {
            DevAccounts::Alice => AccountId::from_seed("Alice"),
            DevAccounts::Bob => AccountId::from_seed("Bob"),
            DevAccounts::Dave => AccountId::from_seed("Dave"),
            DevAccounts::Eve => AccountId::from_seed("Eve"),
            DevAccounts::Ferdie => AccountId::from_seed("Ferdie"),
            DevAccounts::Charlie => AccountId::from_seed("Charlie"),
            DevAccounts::AliceStash => AccountId::from_seed("Alice//stash"),
            DevAccounts::BobStash => AccountId::from_seed("Bob//stash"),
        }
    }

    pub fn all() -> Vec<DevAccounts> {
        vec![
            DevAccounts::Alice,
            DevAccounts::Bob,
            DevAccounts::Dave,
            DevAccounts::Eve,
            DevAccounts::Ferdie,
            DevAccounts::Charlie,
            DevAccounts::AliceStash,
            DevAccounts::BobStash,
        ]
    }
}
