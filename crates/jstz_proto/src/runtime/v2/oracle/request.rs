use jstz_crypto::public_key_hash::PublicKeyHash;
use serde::{Deserialize, Serialize};

use crate::runtime::v2::fetch::http::Request;
use crate::{BlockLevel, Gas};

pub type RequestId = u64;

type UserAddress = PublicKeyHash;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct OracleRequest {
    /// Request Id
    pub id: RequestId,
    /// User that initiated the top level [`crate::operation::RunFunction`]
    pub caller: UserAddress,
    /// Gas limit allocated for processing the OracleResponse. Excludes gas
    /// for resuming execution
    pub gas_limit: Gas,
    /// Request TTL, denoted in [`BlockLevel`]
    pub timeout: BlockLevel,
    /// Request paylaod
    pub request: Request,
}
