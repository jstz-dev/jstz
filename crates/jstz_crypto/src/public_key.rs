use crate::{error::Result, Error};
use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};
use tezos_crypto_rs::{
    hash::{PublicKeyEd25519, PublicKeyP256, PublicKeySecp256k1},
    PublicKeyWithHash,
};
use utoipa::ToSchema;

// FIXME: https://linear.app/tezos/issue/JSTZ-169/support-bls-in-risc-v
// Add BLS support
/// Tezos public key
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, ToSchema)]
#[serde(untagged)]
pub enum PublicKey {
    #[schema(
        title = "Ed25519",
        value_type = String,
        example = json!("edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi")
    )]
    Ed25519(PublicKeyEd25519),
    #[schema(
        title = "Secp256k1",
        value_type = String,
        example = json!("sppk7aMwoVDiMGXkzwqPMrqHNE6QrZ1vAJ2CvTEeGZRLSSTM8jogmKY")
    )]
    Secp256k1(PublicKeySecp256k1),
    #[schema(
        title = "P256",
        value_type = String,
        example = json!("p2pk67ArUx3aDGyFgRco8N3pTnnnbodpP2FMZLAewV6ZAVvCxKjW3Q1")
    )]
    P256(PublicKeyP256),
}

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
                Ok(PublicKey::Ed25519(pk))
            }
            "sppk" => {
                let pk = PublicKeySecp256k1::from_base58_check(data)?;
                Ok(PublicKey::Secp256k1(pk))
            }
            "p2pk" => {
                let pk = PublicKeyP256::from_base58_check(data)?;
                Ok(PublicKey::P256(pk))
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
    #[ignore = "Fails because deserialization cannot handle untagged crypto enums"]
    // FIXME: https://linear.app/tezos/issue/JSTZ-272/fix-binary-round-trip-for-tezos-cryptos
    fn bin_round_trip() {
        let pk = PublicKey::from_base58(TZ1).unwrap();
        let bin = bincode::serialize(&pk).unwrap();
        // Error message:
        //      Result::unwrap()` on an `Err` value: DeserializeAnyNotSupported
        let decoded = bincode::deserialize::<PublicKey>(bin.as_ref()).unwrap();
        assert_eq!(pk, decoded);
    }
}
