use crate::{
    AccountId, AirdropConfig, BABE_GENESIS_EPOCH_CONFIG, BabeConfig, Balance, BalancesConfig,
    ConfidentialTransactionsConfig, RuntimeGenesisConfig, SessionConfig, SessionKeys,
    StakingConfig, UNIT, configs::MaxActiveValidators,
};
use alloc::{vec, vec::Vec};
use frame_support::build_struct_json_patch;
use serde_json::Value;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{
    crypto::{Ss58Codec, get_public_from_string_or_panic},
    sr25519,
};
use sp_genesis_builder::{self, PresetId};
use sp_keyring::Sr25519Keyring;
use sp_staking::StakerStatus;

// Returns the genesis config presets populated with given parameters.
fn testnet_genesis(
    initial_authorities: Vec<(AccountId, AccountId, SessionKeys)>,
    endowed_accounts: Vec<AccountId>,
    stakers: Vec<Staker>,
) -> Value {
    let depo = &include_str!("../../verifier_key.hex")[2..];
    let depo = hex::decode(depo).unwrap();
    let trans = include_bytes!("../../verifier_key01.hex").to_vec();
    let validator_count = initial_authorities.len() as u32;

    build_struct_json_patch!(RuntimeGenesisConfig {
        // todo set to 1 billion token for mainnet
        balances: BalancesConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                .map(|k| (k, 1u128 << 80))
                .collect::<Vec<_>>(),
        },
        session: SessionConfig {
            keys: initial_authorities.into_iter().map(|x| { (x.0, x.1, x.2.clone()) }).collect(),
        },
        airdrop: AirdropConfig {
            initial_funding: 60 * 10_u128.pow(6) * UNIT, // 60 million
            pre_funded_accounts: Default::default()
        },
        staking: StakingConfig {
            validator_count: MaxActiveValidators::get(),
            minimum_validator_count: validator_count,
            invulnerables: endowed_accounts,
            stakers
        },
        confidential_transactions: ConfidentialTransactionsConfig {
            deposit_vk: depo,
            transfer_vk: trans,
            _phantom: Default::default()
        },
        babe: BabeConfig { epoch_config: BABE_GENESIS_EPOCH_CONFIG },
    })
}

/// Return the development genesis config.
pub fn development_config_genesis() -> Value {
    let (alice_stash, alice, alice_session_keys) = authority_keys_from_seed("Alice");

    testnet_genesis(
        vec![(alice, alice_stash.clone(), alice_session_keys)],
        Sr25519Keyring::well_known().map(|key| key.to_account_id()).collect(),
        vec![validator(alice_stash)],
    )
}

/// Return the local genesis config preset.
pub fn local_config_genesis() -> Value {
    let (alice_stash, alice, alice_session_keys) = authority_keys_from_seed("Alice");
    let (bob_stash, _bob, bob_session_keys) = authority_keys_from_seed("Bob");
    testnet_genesis(
        vec![
            (alice, alice_stash.clone(), alice_session_keys),
            (bob_stash.clone(), bob_stash, bob_session_keys),
        ],
        Sr25519Keyring::well_known().map(|key| key.to_account_id()).collect(),
        vec![validator(alice_stash)],
    )
}

pub fn test_net_config_genesis() -> Value {
    let (account, stash, session_keys) = (
        AccountId::from_ss58check("5CmEbGjVRTNB6CaN2vgEyhtUZ2bfyXtaUoYfjwe8h6RzbrUB").unwrap(),
        AccountId::from_ss58check("5Epmb86Zpkx3V366R5dfH57vA5o4g1ehfEmdRHEFnxJFm3nG").unwrap(),
        SessionKeys {
            babe: BabeId::from_ss58check("5CzFDWtNknPfgMdWnvVK9JyWqXJM2kyHzuB7EGwSaAssEYVX")
                .unwrap(),
            grandpa: GrandpaId::from_ss58check("5DHAVCuwaVqhF2nVXXMJHNkRySpaSLxutGhytJv65xPgvBhM")
                .unwrap(),
            authority_discovery: AuthorityDiscoveryId::from_ss58check(
                "5GHB9FMturXHnkMwUCjGcuALC1V3MePix4BoJ7GwGajR5UEU",
            )
            .unwrap(),
        },
    );
    testnet_genesis(
        vec![(account.clone(), stash.clone(), session_keys)],
        vec![account.clone(), stash.clone()],
        vec![validator(stash)],
    )
}

pub const TEST_NET: &str = "testnet";

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
    let patch = match id.as_ref() {
        sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
        sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_config_genesis(),
        TEST_NET => test_net_config_genesis(),
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
        PresetId::from(TEST_NET),
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
