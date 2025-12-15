use derive_more::{Display, Error, From};

use tezos_crypto_rs::{
    base58::FromBase58CheckError, blake2b::Blake2bError, hash::FromBytesError,
    CryptoError,
};

#[derive(Display, Debug, Error, From)]
pub enum Error {
    TezosFromBase58Error {
        source: FromBase58CheckError,
    },
    TezosFromBytesError {
        source: FromBytesError,
    },
    TezosCryptoError {
        source: CryptoError,
    },
    TezosBlake2bError {
        source: Blake2bError,
    },
    InvalidSignature,
    UnrecognizedSignature,
    InvalidPublicKeyHash,
    InvalidPublicKey,
    InvalidSecretKey,
    #[display(fmt = "libsecp256k1 error: {source}")]
    Libsecp256k1Error {
        source: libsecp256k1::Error,
    },
    #[display(fmt = "p256 error: {source}")]
    P256Error {
        source: p256::ecdsa::signature::Error,
    },
    #[display(fmt = "invalid smart function hash")]
    InvalidSmartFunctionHash,
    #[display(fmt = "PasskeyError: {source}")]
    PasskeyError {
        source: crate::verifier::passkey::PasskeyError,
    },
    InvalidVerifier,
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::Error;
    use tezos_crypto_rs::{
        base58::FromBase58CheckError, blake2b::Blake2bError, hash::FromBytesError,
        CryptoError,
    };

    #[test]
    fn display() {
        let tests = [
            (
                Error::TezosFromBase58Error {
                    source: FromBase58CheckError::InvalidBase58,
                },
                "invalid base58",
            ),
            (
                Error::TezosFromBytesError {
                    source: FromBytesError::InvalidSize,
                },
                "invalid hash size",
            ),
            (
                Error::TezosCryptoError {
                    source: CryptoError::InvalidPublicKey,
                },
                "Failed to construct public key",
            ),
            (
                Error::TezosBlake2bError {
                    source: Blake2bError::InvalidLength,
                },
                "Output digest length must be between 16 and 64 bytes.",
            ),
            (
                Error::InvalidSmartFunctionHash,
                "invalid smart function hash",
            ),
            (Error::InvalidSecretKey, "InvalidSecretKey"),
            (
                Error::Libsecp256k1Error {
                    source: libsecp256k1::Error::InvalidSecretKey,
                },
                "libsecp256k1 error: Invalid secret key",
            ),
        ];
        for (e, expected) in tests {
            assert_eq!(format!("{e}"), expected);
        }
    }
}
