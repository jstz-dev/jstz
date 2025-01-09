use jstz_api::KvValue as KvValueInternal;
use jstz_utils::api_map_to;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A value stored in the Key-Value store. Always valid JSON.
#[api_map_to(KvValueInternal)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(try_from = "String", into = "String")]
#[schema(value_type = String, format = "json")]
pub struct KvValue(pub serde_json::Value);

impl From<KvValue> for String {
    fn from(val: KvValue) -> Self {
        val.0.to_string()
    }
}

impl TryFrom<String> for KvValue {
    type Error = serde_json::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Ok(Self(serde_json::from_str(&value)?))
    }
}
