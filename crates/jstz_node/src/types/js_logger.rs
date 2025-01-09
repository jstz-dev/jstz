use crate::types::operation::Address;
use jstz_api::js_log::LogLevel;
use jstz_proto::js_logger::LogRecord as LogRecordInternal;
use jstz_utils::api_map_to;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[api_map_to(LogRecordInternal)]
#[derive(Serialize, Deserialize, ToSchema)]
pub struct LogRecord {
    pub address: Address,
    pub request_id: String,
    pub level: LogLevel,
    pub text: String,
}
