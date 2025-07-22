use bincode::{Decode, Encode};
use jstz_core::event::Event;
use serde::{Deserialize, Serialize};

use crate::runtime::v2::fetch::http::Request;
use crate::{BlockLevel, Gas};

use super::UserAddress;

pub type RequestId = u64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
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
    #[bincode(with_serde)]
    pub request: Request,
}

const ORACLE_PREFIX: &str = "ORACLE";

impl Event for OracleRequest {
    fn tag() -> &'static str {
        ORACLE_PREFIX
    }
}
