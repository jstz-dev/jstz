use std::fmt::{self, Debug, Display};

use serde::{Deserialize, Serialize};
use tezos_crypto_rs::{
    blake2b,
    hash::{HashTrait, Secp256k1Signature, SecretKeyEd25519, SecretKeySecp256k1},
};

use crate::{error::Result, signature::Signature, Error};

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum SecretKey {
    Ed25519(SecretKeyEd25519),
    Secp256k1(SecretKeySecp256k1),
}

impl Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SecretKey").field(&self.to_base58()).finish()
    }
}

impl SecretKey {
    pub fn to_base58(&self) -> String {
        match self {
            SecretKey::Ed25519(sk) => sk.to_base58_check(),
            SecretKey::Secp256k1(sk) => sk.to_base58_check(),
        }
    }

    pub fn from_base58(data: &str) -> Result<Self> {
        if data.len() < 4 {
            return Err(Error::InvalidSecretKey);
        }
        match &data[..4] {
            "edsk" => {
                let sk = SecretKeyEd25519::from_base58_check(data)?;
                Ok(SecretKey::Ed25519(sk))
            }
            "spsk" => {
                let sk = SecretKeySecp256k1::from_base58_check(data)?;
                Ok(SecretKey::Secp256k1(sk))
            }
            _ => Err(Error::InvalidSecretKey),
        }
    }

    pub fn sign(&self, message: impl AsRef<[u8]>) -> Result<Signature> {
        Ok(match self {
            SecretKey::Ed25519(sk) => Signature::Ed25519(sk.sign(message)?.into()),
            SecretKey::Secp256k1(sk) => {
                // tezos_crypto_rs does not implement signing with spsk
                let key = libsecp256k1::SecretKey::parse_slice(sk.as_ref())
                    .map_err(|e| Error::Libsecp256k1Error { source: e })?;
                let msg_hash =
                    blake2b::digest(message.as_ref(), libsecp256k1::util::MESSAGE_SIZE)?;
                let (sig, _) = libsecp256k1::sign(
                    &libsecp256k1::Message::parse_slice(&msg_hash)
                        .map_err(|e| Error::Libsecp256k1Error { source: e })?,
                    &key,
                );
                Signature::Secp256k1(
                    Secp256k1Signature::try_from_bytes(&sig.serialize())?.into(),
                )
            }
        })
    }
}

impl Display for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

#[cfg(test)]
mod test {
    use super::SecretKey;

    const EDSK: &str = "edsk3caELE9Pmo6Zyy3rNrE1THwYGQc97FUnGz5Si5NC78d6khpW6A";
    const SPSK: &str = "spsk1ppL4ohtyZeighKZehzfGr2p6dL51kwQqEV2N1sNT7rx9cg5jG";

    #[test]
    fn base58_round_trip() {
        // key too short
        assert_eq!(
            SecretKey::from_base58("aaa").unwrap_err().to_string(),
            "InvalidSecretKey"
        );

        // key with unknown prefix
        assert_eq!(
            SecretKey::from_base58("aaaaaaa").unwrap_err().to_string(),
            "InvalidSecretKey"
        );

        let sk = SecretKey::from_base58(EDSK).expect("Should not fail");
        assert_eq!(sk.to_base58(), EDSK);

        let sk = SecretKey::from_base58(SPSK).expect("Should not fail");
        assert_eq!(sk.to_base58(), SPSK);
    }

    #[test]
    fn to_string() {
        let sk = SecretKey::from_base58(EDSK).expect("Should not fail");
        assert_eq!(sk.to_string(), EDSK);

        let sk = SecretKey::from_base58(SPSK).expect("Should not fail");
        assert_eq!(sk.to_string(), SPSK);
    }

    #[test]
    fn json_round_trip() {
        let json = "\"edsk3YuM4VFTRxq4LmWzf293iEdgramaDhgVnx3ij3CzgQTeDRcb1Q\"";
        let sk: SecretKey = serde_json::from_str(json).expect("Should not fail");
        assert_eq!(
            sk.to_string(),
            "edsk3YuM4VFTRxq4LmWzf293iEdgramaDhgVnx3ij3CzgQTeDRcb1Q"
        );
        assert_eq!(serde_json::to_string(&sk).expect("Should not fail"), json);

        let json = format!("\"{SPSK}\"");
        let sk: SecretKey = serde_json::from_str(&json).expect("Should not fail");
        assert_eq!(sk.to_string(), SPSK);
        assert_eq!(serde_json::to_string(&sk).expect("Should not fail"), json);
    }

    #[test]
    fn sign() {
        let msg = "foobar";
        let sk = SecretKey::from_base58(EDSK).unwrap();
        assert_eq!(sk.sign(msg).unwrap().to_string(), "edsigtuAVH237U81kEXt2TiqkaY7HUCf6Xx96mQ9kEL21Qa7ASYy48sd5ktjogrvmJdURz25Fcjkg19SqeNPcxfRWze9nseyVJB");

        let sk = SecretKey::from_base58(SPSK).unwrap();
        assert_eq!(sk.sign(msg).unwrap().to_string(), "spsig1CKwEgFniD7wDTQkLnWcX7YtTLiLvJnUpDWEAKNg9YAtppZiMXfWtk5JK4DqjKT38ERwK8zVYQ51npdDBAPhqFDp376pHW");
    }
}
