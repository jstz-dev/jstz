use std::fmt::{self, Debug, Display};

use serde::{Deserialize, Serialize};
use tezos_crypto_rs::hash::SecretKeyEd25519;

use crate::{error::Result, signature::Signature};

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum SecretKey {
    Ed25519(SecretKeyEd25519),
}

impl Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SecretKey").field(&self.to_base58()).finish()
    }
}

impl SecretKey {
    pub fn to_base58(&self) -> String {
        let Self::Ed25519(sk) = self;
        sk.to_base58_check()
    }

    pub fn from_base58(data: &str) -> Result<Self> {
        let sk = SecretKeyEd25519::from_base58_check(data)?;
        Ok(SecretKey::Ed25519(sk))
    }

    pub fn sign(&self, message: impl AsRef<[u8]>) -> Result<Signature> {
        let SecretKey::Ed25519(sk) = self;
        Ok(Signature::Ed25519(sk.sign(message)?.into()))
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

    const SK: &str = "edsk3caELE9Pmo6Zyy3rNrE1THwYGQc97FUnGz5Si5NC78d6khpW6A";

    #[test]
    fn base58_round_trip() {
        let sk = SecretKey::from_base58(SK).expect("Should not fail");
        assert_eq!(sk.to_base58(), SK);
    }

    #[test]
    fn to_string() {
        let sk = SecretKey::from_base58(SK).expect("Should not fail");
        assert_eq!(sk.to_string(), SK);
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
    }
}
