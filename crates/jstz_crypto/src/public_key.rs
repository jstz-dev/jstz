use crate::{error::Result, impl_bincode_for_hash, Error};
use bincode::{Decode, Encode};
use derive_more::{Deref, From};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};
use tezos_crypto_rs::{
    hash::{PublicKeyEd25519, PublicKeyP256, PublicKeySecp256k1},
    PublicKeyWithHash,
};
use utoipa::ToSchema;

// FIXME: https://linear.app/tezos/issue/JSTZ-169/support-bls-in-risc-v
// Add BLS support
/// Tezos public key
#[derive(
    From,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    ToSchema,
    Encode,
    Decode,
)]
#[serde(untagged)]
pub enum PublicKey {
    #[schema(
        title = "Ed25519",
        value_type = String,
        example = json!("edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi")
    )]
    Ed25519(Ed25519),
    #[schema(
        title = "Secp256k1",
        value_type = String,
        example = json!("sppk7aMwoVDiMGXkzwqPMrqHNE6QrZ1vAJ2CvTEeGZRLSSTM8jogmKY")
    )]
    Secp256k1(Secp256k1),
    #[schema(
        title = "P256",
        value_type = String,
        example = json!("p2pk67ArUx3aDGyFgRco8N3pTnnnbodpP2FMZLAewV6ZAVvCxKjW3Q1")
    )]
    P256(P256),
}

// Newtype wrappers
#[derive(Deref, From, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ed25519(pub PublicKeyEd25519);

#[derive(Deref, From, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Secp256k1(pub PublicKeySecp256k1);

#[derive(Deref, From, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct P256(pub PublicKeyP256);

// Bincode implementation
impl_bincode_for_hash!(Ed25519, PublicKeyEd25519);
impl_bincode_for_hash!(Secp256k1, PublicKeySecp256k1);
impl_bincode_for_hash!(P256, PublicKeyP256);

impl PublicKey {
    pub fn to_base58(&self) -> String {
        match self {
            PublicKey::Ed25519(pk) => pk.to_base58_check(),
            PublicKey::Secp256k1(pk) => pk.to_base58_check(),
            PublicKey::P256(pk) => pk.to_base58_check(),
        }
    }

    pub fn hash(&self) -> String {
        match self {
            PublicKey::Ed25519(pk) => pk.pk_hash().to_string(),
            PublicKey::Secp256k1(pk) => pk.pk_hash().to_string(),
            PublicKey::P256(pk) => pk.pk_hash().to_string(),
        }
    }

    pub fn from_base58(data: &str) -> Result<Self> {
        match &data[..4] {
            "edpk" => {
                let pk = PublicKeyEd25519::from_base58_check(data)?;
                Ok(PublicKey::Ed25519(pk.into()))
            }
            "sppk" => {
                let pk = PublicKeySecp256k1::from_base58_check(data)?;
                Ok(PublicKey::Secp256k1(pk.into()))
            }
            "p2pk" => {
                let pk = PublicKeyP256::from_base58_check(data)?;
                Ok(PublicKey::P256(pk.into()))
            }
            _ => Err(Error::InvalidPublicKey),
        }
    }
}

impl Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

#[cfg(test)]
mod test {

    use tezos_crypto_rs::hash::HashTrait;

    use crate::public_key::PublicKey;

    const TZ1: &str = "edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi";
    const TZ2: &str = "sppk7aMwoVDiMGXkzwqPMrqHNE6QrZ1vAJ2CvTEeGZRLSSTM8jogmKY";
    const TZ3: &str = "p2pk67ArUx3aDGyFgRco8N3pTnnnbodpP2FMZLAewV6ZAVvCxKjW3Q1";

    #[test]
    fn base58() {
        assert!(matches!(
            PublicKey::from_base58(TZ1).unwrap(),
            PublicKey::Ed25519(pk) if pk.to_b58check() == TZ1
        ));
        assert!(matches!(
            PublicKey::from_base58(TZ2).unwrap(),
            PublicKey::Secp256k1(tz2) if tz2.to_b58check() == TZ2
        ));
        assert!(matches!(
            PublicKey::from_base58(TZ3).unwrap(),
            PublicKey::P256(tz3) if tz3.to_b58check() == TZ3
        ));
        PublicKey::from_base58("invalid").expect_err("should fail");
        PublicKey::from_base58("edpinvalid52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi")
            .expect_err("should fail");

        assert_eq!(PublicKey::from_base58(TZ1).unwrap().to_base58(), TZ1);
        assert_eq!(PublicKey::from_base58(TZ2).unwrap().to_base58(), TZ2);
        assert_eq!(PublicKey::from_base58(TZ3).unwrap().to_base58(), TZ3);
    }

    #[test]
    fn to_string() {
        assert_eq!(PublicKey::from_base58(TZ1).unwrap().to_string(), TZ1);
        assert_eq!(PublicKey::from_base58(TZ2).unwrap().to_string(), TZ2);
        assert_eq!(PublicKey::from_base58(TZ3).unwrap().to_string(), TZ3);
    }

    #[test]
    fn hash() {
        assert_eq!(
            PublicKey::from_base58(TZ1).unwrap().hash(),
            "tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU"
        );
        assert_eq!(
            PublicKey::from_base58(TZ2).unwrap().hash(),
            "tz2KDvEL9fuvytRfe1cVVDo1QfDfaBktGNkh"
        );
        assert_eq!(
            PublicKey::from_base58(TZ3).unwrap().hash(),
            "tz3QxNCB8HgxJyp5V9ZmCVGcTm6BzYc14k9C"
        );
    }

    #[test]
    fn json_round_trip() {
        let pk = PublicKey::from_base58(TZ1).unwrap();
        let json = serde_json::to_value(&pk).unwrap();
        assert_eq!(
            json,
            serde_json::json!("edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi")
        );
        let decoded: PublicKey = serde_json::from_value(json).unwrap();
        assert_eq!(pk, decoded);
    }

    #[test]
    fn bin_round_trip() {
        let pk = PublicKey::from_base58(TZ1).unwrap();
        let bin = bincode::encode_to_vec(&pk, bincode::config::legacy()).unwrap();
        let (decoded, _): (PublicKey, _) =
            bincode::decode_from_slice(bin.as_slice(), bincode::config::legacy())
                .unwrap();
        assert_eq!(pk, decoded);
    }
}
