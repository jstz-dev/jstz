use std::fmt::{self, Debug, Display};

use serde::{Deserialize, Serialize};
use tezos_crypto_rs::{
    blake2b,
    hash::{
        HashTrait, P256Signature, Secp256k1Signature, SecretKeyEd25519, SecretKeyP256,
        SecretKeySecp256k1,
    },
};

use crate::{error::Result, signature::Signature, Error};

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum SecretKey {
    Ed25519(SecretKeyEd25519),
    Secp256k1(SecretKeySecp256k1),
    P256(SecretKeyP256),
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
            SecretKey::P256(sk) => sk.to_base58_check(),
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
            "p2sk" => {
                let sk = SecretKeyP256::from_base58_check(data)?;
                Ok(SecretKey::P256(sk))
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
            SecretKey::P256(sk) => sign_p256(sk, message.as_ref())?,
        })
    }
}

/// Sign message with P256 secret key
fn sign_p256(sk: &SecretKeyP256, message: &[u8]) -> Result<Signature> {
    use p256::ecdsa::signature::digest::{
        BlockInput, Digest, FixedOutput, Reset, Update,
    };
    use p256::ecdsa::signature::DigestSigner;
    use p256::ecdsa::SigningKey;
    use p256::elliptic_curve::consts::U32;

    #[derive(Default, Clone)]
    struct Blake2b256([u8; 32]);

    /// The traits below are necessary for implementing [`Digest`]
    ///
    /// Tezos P256 signatures are always applied over the 32-byte blake2b
    /// hash of the payload
    impl Update for Blake2b256 {
        fn update(&mut self, data: impl AsRef<[u8]>) {
            let data = data.as_ref();
            let data = blake2b::digest_256(data);
            self.0.copy_from_slice(&data[..32]);
        }
    }

    impl FixedOutput for Blake2b256 {
        type OutputSize = U32;

        fn finalize_into(
            self,
            out: &mut p256::elliptic_curve::generic_array::GenericArray<
                u8,
                Self::OutputSize,
            >,
        ) {
            out.copy_from_slice(&self.0[..]);
        }

        fn finalize_into_reset(
            &mut self,
            out: &mut p256::elliptic_curve::generic_array::GenericArray<
                u8,
                Self::OutputSize,
            >,
        ) {
            out.copy_from_slice(&self.0[..]);
        }
    }

    impl BlockInput for Blake2b256 {
        type BlockSize = U32;
    }

    impl Reset for Blake2b256 {
        fn reset(&mut self) {}
    }

    let key = SigningKey::from_bytes(sk.as_ref())
        .map_err(|e| Error::P256Error { source: e })?;
    let digest = Digest::chain(Blake2b256::new(), message);
    let signature = key.sign_digest(digest);
    Ok(Signature::P256(
        P256Signature::try_from_bytes(signature.as_ref())?.into(),
    ))
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
    const P2SK: &str = "p2sk2REWfVA5GbHf6cdGK74krBzHzEaS9ifLg3b1syZ821DQ5Btd3T";

    const SECRETS: [&str; 3] = [EDSK, SPSK, P2SK];

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

        SECRETS.iter().for_each(|&sk_str| {
            let sk = SecretKey::from_base58(sk_str).expect("Should not fail");
            assert_eq!(sk.to_base58(), sk_str);
        });
    }

    #[test]
    fn to_string() {
        SECRETS.iter().for_each(|&sk_str| {
            let sk = SecretKey::from_base58(sk_str).expect("Should not fail");
            assert_eq!(sk.to_string(), sk_str);
        });
    }

    #[test]
    fn json_round_trip() {
        SECRETS.iter().for_each(|&sk_str| {
            let json = format!("\"{sk_str}\"");
            let sk: SecretKey = serde_json::from_str(&json).expect("Should not fail");
            assert_eq!(sk.to_string(), sk_str);
            assert_eq!(serde_json::to_string(&sk).expect("Should not fail"), json);
        });
    }

    #[test]
    fn sign() {
        let msg = "foobar";
        let sk = SecretKey::from_base58(EDSK).unwrap();
        assert_eq!(sk.sign(msg).unwrap().to_string(), "edsigtuAVH237U81kEXt2TiqkaY7HUCf6Xx96mQ9kEL21Qa7ASYy48sd5ktjogrvmJdURz25Fcjkg19SqeNPcxfRWze9nseyVJB");

        let sk = SecretKey::from_base58(SPSK).unwrap();
        assert_eq!(sk.sign(msg).unwrap().to_string(), "spsig1CKwEgFniD7wDTQkLnWcX7YtTLiLvJnUpDWEAKNg9YAtppZiMXfWtk5JK4DqjKT38ERwK8zVYQ51npdDBAPhqFDp376pHW");

        let sk = SecretKey::from_base58(P2SK).unwrap();
        assert_eq!(sk.sign(msg).unwrap().to_string(), "p2sigbgcUvtFhWaH7crZuyULzen2V7KUaWnBCZ5gtm6F8yoxeCWBQgPdALbu94iabwrXi6k8YXvnNKCnc5LqF4GSJjNuFG46dE");
    }
}
