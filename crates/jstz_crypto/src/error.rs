use derive_more::{Display, Error, From};

use tezos_crypto_rs::{
    base58::FromBase58CheckError, blake2b::Blake2bError, hash::FromBytesError,
    CryptoError,
};

#[derive(Display, Debug, Error, From)]
pub enum Error {
    TezosFromBase58Error { source: FromBase58CheckError },
    TezosFromBytesError { source: FromBytesError },
    TezosCryptoError { source: CryptoError },
    TezosBlake2bError { source: Blake2bError },
    InvalidSignature,
    InvalidPublicKeyHash,
    InvalidPublicKey,
    InvalidSmartFunctionHash,
}

pub type Result<T> = std::result::Result<T, Error>;
