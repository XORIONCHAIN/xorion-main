#![no_std]
extern crate alloc;

pub mod dev_accounts;
use alloc::vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use core::fmt::{Debug, Display};
use crystals_dilithium::dilithium3;
pub use crystals_dilithium::dilithium3::Keypair as XorionKeypair;
use scale_info::TypeInfo;
use sp_core::{sha2_256, RuntimeDebug};
use sp_runtime::traits::{IdentifyAccount, Lazy};

/// A fully Ethereum-compatible `AccountId`.
/// Conforms to H160 address and ECDSA key standards.
/// Alternative to H256->H160 mapping.
#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
)]
pub struct AccountId([u8; dilithium3::PUBLICKEYBYTES]);
impl_serde::impl_fixed_hash_serde!(AccountId, dilithium3::PUBLICKEYBYTES);

impl core::str::FromStr for AccountId {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let result = bs58::decode(s).into_vec().map_err(|_| "Invalid account id")?;
        if result[0] != 56 {
            Err("Invalid account id")
        } else {
            Self::try_from(&result[1..]).map_err(|_| "invalid account id")
        }
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut r = vec![56];
        r.extend_from_slice(self.as_ref());
        write!(f, "{}", bs58::encode(r).into_string())
    }
}

impl Debug for AccountId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <AccountId as Display>::fmt(self, f)
    }
}
impl From<[u8; dilithium3::PUBLICKEYBYTES]> for AccountId {
    fn from(value: [u8; dilithium3::PUBLICKEYBYTES]) -> Self {
        Self(value)
    }
}

impl<'a> TryFrom<&'a [u8]> for AccountId {
    type Error = ();
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        Ok(Self(
            dilithium3::PublicKey::from_bytes(value)
                .map_err(|e| log::error!("Invalid public key bytes {e}"))?
                .bytes,
        ))
    }
}

impl AsRef<[u8]> for AccountId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for AccountId {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl AccountId {
    pub fn from_seed(seed: &str) -> Self {
        Self(
            dilithium3::Keypair::generate(Some(&sha2_256(seed.as_bytes())))
                .unwrap()
                .public
                .bytes,
        )
    }
}

impl sp_runtime::traits::IdentifyAccount for AccountId {
    type AccountId = Self;

    fn into_account(self) -> Self::AccountId {
        self
    }
}
#[derive(
    Clone,
    Eq,
    PartialEq,
    RuntimeDebug,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
)]
pub struct Signature(dilithium3::Signature);

impl sp_runtime::traits::Verify for Signature {
    type Signer = AccountId;

    fn verify<L: Lazy<[u8]>>(
        &self,
        mut msg: L,
        signer: &<Self::Signer as IdentifyAccount>::AccountId,
    ) -> bool {
        dilithium3::PublicKey::from_bytes(&signer.0)
            .map(|public| public.verify(msg.get(), self.0.as_slice()))
            .unwrap_or_default()
    }
}
