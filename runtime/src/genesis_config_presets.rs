use crate::{
    AccountId, BabeConfig, Balance, BalancesConfig, RuntimeGenesisConfig, SessionConfig,
    SessionKeys, StakingConfig, SudoConfig, BABE_GENESIS_EPOCH_CONFIG,
};
use alloc::{vec, vec::Vec};
use frame_support::build_struct_json_patch;
use serde_json::Value;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{crypto::get_public_from_string_or_panic, sr25519};
use sp_genesis_builder::{self, PresetId};
use sp_keyring::Sr25519Keyring;
use sp_staking::StakerStatus;

// Returns the genesis config presets populated with given parameters.
fn testnet_genesis(
    initial_authorities: Vec<(AccountId, AccountId, SessionKeys)>,
    endowed_accounts: Vec<AccountId>,
    root: AccountId,
    stakers: Vec<Staker>,
) -> Value {
    let validator_count = initial_authorities.len() as u32;

    build_struct_json_patch!(RuntimeGenesisConfig {
        balances: BalancesConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                .map(|k| (k, 1u128 << 60))
                .collect::<Vec<_>>(),
        },
        session: SessionConfig {
            keys: initial_authorities
                .iter()
                .map(|x| { (x.0.clone(), x.1.clone(), x.2.clone()) })
                .collect(),
        },
        staking: StakingConfig {
            validator_count,
            minimum_validator_count: validator_count,
            invulnerables: endowed_accounts,
            stakers
        },
        babe: BabeConfig { epoch_config: BABE_GENESIS_EPOCH_CONFIG },
        sudo: SudoConfig { key: Some(root) },
    })
}

/// Return the development genesis config.
pub fn development_config_genesis() -> Value {
    let (alice_stash, _alice, alice_session_keys) = authority_keys_from_seed("Alice");

    testnet_genesis(
        vec![(alice_stash.clone(), alice_stash.clone(), alice_session_keys)],
        vec![
            Sr25519Keyring::Alice.to_account_id(),
            Sr25519Keyring::Bob.to_account_id(),
            Sr25519Keyring::AliceStash.to_account_id(),
            Sr25519Keyring::BobStash.to_account_id(),
        ],
        sp_keyring::Sr25519Keyring::Alice.to_account_id(),
        vec![validator(alice_stash.clone())],
    )
}

/// Return the local genesis config preset.
pub fn local_config_genesis() -> Value {
    let (alice_stash, _alice, alice_session_keys) = authority_keys_from_seed("Alice");
    let (bob_stash, _bob, bob_session_keys) = authority_keys_from_seed("Bob");
    testnet_genesis(
        vec![
            (alice_stash.clone(), alice_stash.clone(), alice_session_keys),
            (bob_stash.clone(), bob_stash.clone(), bob_session_keys),
        ],
        Sr25519Keyring::iter()
            .filter(|v| v != &Sr25519Keyring::One && v != &Sr25519Keyring::Two)
            .map(|v| v.to_account_id())
            .collect::<Vec<_>>(),
        Sr25519Keyring::Alice.to_account_id(),
        vec![validator(alice_stash.clone())],
    )
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
    let patch = match id.as_ref() {
        sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
        sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_config_genesis(),
        _ => return None,
    };
    Some(
        serde_json::to_string(&patch)
            .expect("serialization to json is expected to work. qed.")
            .into_bytes(),
    )
}

/// List of supported presets.
pub fn preset_names() -> Vec<PresetId> {
    vec![
        PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
        PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
    ]
}

/// The staker type as supplied at the Staking config.
pub type Staker = (AccountId, AccountId, Balance, StakerStatus<AccountId>);

/// Sets up the `account` to be a staker of validator variant as supplied to the
/// staking config.
pub fn validator(account: AccountId) -> Staker {
    // validator, controller, stash, staker status
    (account.clone(), account, 1u128 << 50, StakerStatus::Validator)
}

pub fn session_keys(
    grandpa: GrandpaId,
    babe: BabeId,
    authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
    SessionKeys { grandpa, babe, authority_discovery }
}

pub fn session_keys_from_seed(seed: &str) -> SessionKeys {
    session_keys(
        get_public_from_string_or_panic::<GrandpaId>(seed),
        get_public_from_string_or_panic::<BabeId>(seed),
        get_public_from_string_or_panic::<AuthorityDiscoveryId>(seed),
    )
}

/// Helper function to generate stash, controller and session key from seed.
///
/// Note: `//` is prepended internally.
pub fn authority_keys_from_seed(seed: &str) -> (AccountId, AccountId, SessionKeys) {
    (
        get_public_from_string_or_panic::<sr25519::Public>(&alloc::format!("{seed}//stash")).into(),
        get_public_from_string_or_panic::<sr25519::Public>(seed).into(),
        session_keys_from_seed(seed),
    )
}
