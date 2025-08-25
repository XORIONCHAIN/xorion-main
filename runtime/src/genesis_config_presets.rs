use crate::{
    AccountId, BABE_GENESIS_EPOCH_CONFIG, BabeConfig, Balance, BalancesConfig,
    ConfidentialTransactionsConfig, EthereumBridgeConfig, GRAND, LaunchClaimConfig,
    RuntimeGenesisConfig, SessionConfig, SessionKeys, StakingConfig, UNIT, VestingConfig,
    configs::MaxActiveValidators,
};
use alloc::{vec, vec::Vec};
use frame_support::{PalletId, build_struct_json_patch};
use serde_json::Value;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{
    H160,
    crypto::{Ss58Codec, get_public_from_string_or_panic},
    sr25519,
};
use sp_genesis_builder::{self, PresetId};
use sp_keyring::Sr25519Keyring;
use sp_runtime::traits::AccountIdConversion;
use sp_staking::StakerStatus;

const STAKING_PALLET_ID: PalletId = PalletId(*b"xor/stk ");

// Returns the genesis config presets populated with given parameters.
fn net_genesis(
    initial_authorities: Vec<(AccountId, AccountId, SessionKeys)>,
    endowed_accounts: Vec<AccountId>,
    stakers: Vec<Staker>,
) -> Value {
    let depo = &include_str!("../../verifier_key.hex")[2..];
    let depo = hex::decode(depo).unwrap();
    let trans = include_bytes!("../../verifier_key01.hex").to_vec();
    let validator_count = initial_authorities.len() as u32;

    build_struct_json_patch!(RuntimeGenesisConfig {
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

    net_genesis(
        vec![(alice, alice_stash.clone(), alice_session_keys)],
        Sr25519Keyring::well_known().map(|key| key.to_account_id()).collect(),
        vec![validator(alice_stash)],
    )
}

/// Return the local genesis config preset.
pub fn local_config_genesis() -> Value {
    let (alice_stash, alice, alice_session_keys) = authority_keys_from_seed("Alice");
    let (bob_stash, _bob, bob_session_keys) = authority_keys_from_seed("Bob");
    net_genesis(
        vec![
            (alice, alice_stash.clone(), alice_session_keys),
            (bob_stash.clone(), bob_stash, bob_session_keys),
        ],
        Sr25519Keyring::well_known().map(|key| key.to_account_id()).collect(),
        vec![validator(alice_stash)],
    )
}

pub fn mainnet_config_genesis() -> Value {
    let depo = &include_str!("../../verifier_key.hex")[2..];
    let depo = hex::decode(depo).unwrap();
    let trans = include_bytes!("../../verifier_key01.hex").to_vec();

    let session_keys = SessionKeys {
        babe: BabeId::from_ss58check("5GVuKRzQHoeSB2GdChqivZrjzWXxGcGPvTDuoNK3o67X6Hxg").unwrap(),
        grandpa: GrandpaId::from_ss58check("5DsiHAGZ33Psf9mnKTthRPzkHB43ZETqsoN7zKJ8sYyoVREo")
            .unwrap(),
        authority_discovery: AuthorityDiscoveryId::from_ss58check(
            "5EUcgHc8rqCaHhfmVHtuwewdWb3f3KSwgqZBvArj36ggU2uQ",
        )
        .unwrap(),
    };
    let session_keys_2 = SessionKeys {
        babe: BabeId::from_ss58check("5Ehz9iqnJHB9jFdHv2u3RApkAHjVt4H4mQqXp28u7UNCBMsc").unwrap(),
        grandpa: GrandpaId::from_ss58check("5FUUCvWLaxPucyiLKEcSLP375aLWmW6rjVvwUZUURJceZ8kA")
            .unwrap(),
        authority_discovery: AuthorityDiscoveryId::from_ss58check(
            "5ECKhzrkb9bneuJcRBhME1vMfUPE4KTKSxw3WKGdBeyeS9TK",
        )
        .unwrap(),
    };

    // 5% unlocked at the Token Generation Event (TGE), with the remaining 95% vesting linearly over
    // 24 months.
    let thirty_five_percent_account =
        AccountId::from_ss58check("5E1UShyFSmbm2ocCgzLSuWKdcKXZPeXphYYdUeZJbWtduoex").unwrap();

    let thirty_five_percent_account_total_bal = 350_000_000 * UNIT;

    let twenty_percent_account =
        AccountId::from_ss58check("5CY5FuoSBX1rowaVqQDzZYBhq3RW31MQrhiFsmZHXjxgZ1NR").unwrap();
    let twenty_percent_account_total_bal = 200_000_000 * UNIT;

    let fifteen_percent_account =
        AccountId::from_ss58check("5CDB73ww5cBbsdk9BBu8oDpZZ5YGooeFUyARai38ug4btfHE").unwrap();
    let fifteen_percent_account_total_bal = 150_000_000 * UNIT;

    let validator_rewards_account = STAKING_PALLET_ID.into_account_truncating();
    let validator_rewards = 100_000_000 * UNIT;

    let ten_percent_account =
        AccountId::from_ss58check("5GL2PxADUxncYrbM9rBH4zYyCsEpez9K7QitrBoNAkcSNC4t").unwrap();
    let ten_percent_account_total_bal = 100_000_000 * UNIT;
    let launch_pad_account =
        AccountId::from_ss58check("5H9V5nTeVwEKykvUDvfGQKA4mVqLNeGUDNL9kaA4Qt1EPYdj").unwrap();
    let launch_pad_total_bal = 50_000_000 * UNIT;
    let future_use_account =
        AccountId::from_ss58check("5Ft5w1myw1GhkJq6CJb6MnqeCGd57gExmzv2DzBXiwejeoGR").unwrap();
    let future_use_total_bal = 50_000_000 * UNIT;

    fn from_str(input: &str) -> H160 {
        let input = input.strip_prefix("0x").unwrap_or(input);
        let mut iter = rustc_hex::FromHexIter::new(input);
        let mut result = H160::zero();
        for byte in result.as_mut() {
            *byte = iter.next().unwrap().unwrap();
        }
        if iter.next().is_some() {
            panic!("Self::Err::InvalidHexLength")
        }
        result
    }
    build_struct_json_patch!(RuntimeGenesisConfig {
        balances: BalancesConfig {
            balances: vec![
                (thirty_five_percent_account.clone(), thirty_five_percent_account_total_bal),
                (twenty_percent_account.clone(), twenty_percent_account_total_bal),
                (fifteen_percent_account.clone(), fifteen_percent_account_total_bal),
                (ten_percent_account.clone(), ten_percent_account_total_bal),
                (launch_pad_account.clone(), launch_pad_total_bal),
                (validator_rewards_account, validator_rewards),
                (future_use_account, future_use_total_bal),
            ]
        },
        session: SessionConfig {
            keys: vec![
                (
                    thirty_five_percent_account.clone(),
                    thirty_five_percent_account.clone(),
                    session_keys
                ),
                (fifteen_percent_account.clone(), fifteen_percent_account.clone(), session_keys_2)
            ]
        },
        staking: StakingConfig {
            validator_count: MaxActiveValidators::get(),
            minimum_validator_count: 2,
            stakers: vec![(
                thirty_five_percent_account.clone(),
                thirty_five_percent_account.clone(),
                10 * GRAND,
                StakerStatus::Validator
            )]
        },
        confidential_transactions: ConfidentialTransactionsConfig {
            deposit_vk: depo,
            transfer_vk: trans,
            _phantom: Default::default()
        },
        launch_claim: LaunchClaimConfig {
            funding_source_account: Some(launch_pad_account.clone()),
            owner: Some(launch_pad_account.clone()),
        },
        ethereum_bridge: EthereumBridgeConfig {
            relayers: vec![
                from_str("0x43a603f19fa345eCE0b94F3E759C424b2892F540"),
                from_str("0xF553da68d83cd57DD1804BB0aAd7d4D591024FFE"),
                from_str("0x0c2752f5fc2204982D18F285cB6A55f39Db22A57"),
            ],
            _phantom: Default::default()
        },
        babe: BabeConfig { epoch_config: BABE_GENESIS_EPOCH_CONFIG },
        vesting: VestingConfig {
            vesting: vec![
                (
                    thirty_five_percent_account, // who
                    0,                           // start block
                    10_519_200,                  // length ~24 months from start
                    (thirty_five_percent_account_total_bal * 5) / 100  /* liquid - Number of
                                                  * units which
                                                  * can be spent before
                                                  * vesting
                                                  * begins. */
                ),
                (
                    twenty_percent_account, // who
                    5_259_600,              // start block
                    15_778_800,             /* duration from start The 36-month vesting duration
                                             * in blocks. */
                    0 // There is no liquid portion. The full amount is vested.
                ),
                (
                    fifteen_percent_account, // who
                    0,                       // start block
                    10_519_200,              // length ~24 months from start
                    (fifteen_percent_account_total_bal * 5) / 100  /* liquid - Number of
                                              * units which
                                              * can be spent before
                                              * vesting
                                              * begins. */
                ),
                (
                    ten_percent_account, // who
                    2_629_800,           // start block 6 months
                    7_889_400,           /* duration from start The 18-month vesting duration
                                          * in blocks. */
                    0 // There is no liquid portion. The full amount is vested.
                ),
                (
                    launch_pad_account, // who
                    0,                  // start block
                    2_629_800,          // length ~6 months from start
                    launch_pad_total_bal / 2  /* liquid - Number of
                                         * units which
                                         * can be spent before
                                         * vesting
                                         * begins. */
                ),
            ]
        }
    })
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
    net_genesis(
        vec![(account.clone(), stash.clone(), session_keys)],
        vec![account.clone(), stash.clone()],
        vec![validator(stash)],
    )
}

pub const TEST_NET: &str = "testnet";
pub const MAIN_NET: &str = "mainnet";

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
    let patch = match id.as_ref() {
        sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
        sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_config_genesis(),
        TEST_NET => test_net_config_genesis(),
        MAIN_NET => mainnet_config_genesis(),
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
