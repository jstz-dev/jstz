use crate::{error::Result, Error};
use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};
use tezos_crypto_rs::hash::{PublicKeyEd25519, PublicKeyP256, PublicKeySecp256k1};

// FIXME: https://linear.app/tezos/issue/JSTZ-169/support-bls-in-risc-v
// Add BLS support
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum PublicKey {
    Ed25519(PublicKeyEd25519),
    Secp256k1(PublicKeySecp256k1),
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
}
