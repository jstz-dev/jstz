mod request;
use jstz_crypto::public_key_hash::PublicKeyHash;
pub use request::*;

mod oracle;
pub use oracle::*;

type UserAddress = PublicKeyHash;
