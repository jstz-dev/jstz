use derive_more::{Display, Error, From};

use tezos_crypto_rs::{base58::FromBase58CheckError, hash::FromBytesError, CryptoError};

#[derive(Display, Debug, Error, From)]
pub enum Error {
    TezosFromBase58Error { source: FromBase58CheckError },
    TezosFromBytesError { source: FromBytesError },
    TezosCryptoError { source: CryptoError },
    InvalidSignature,
    InvalidPublicKeyHash,
    InvalidPublicKey,
}

pub type Result<T> = std::result::Result<T, Error>;
