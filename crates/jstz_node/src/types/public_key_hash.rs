use jstz_crypto::public_key_hash::PublicKeyHash as PublicKeyHashInternal;
use jstz_utils::api_map_to;
use serde::{Deserialize, Serialize};
use tezos_crypto_rs::hash::{ContractTz1Hash, ContractTz2Hash, ContractTz3Hash};
use utoipa::ToSchema;

/// Tezos Address
#[api_map_to(PublicKeyHashInternal)]
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(untagged)]
pub enum PublicKeyHash {
    #[schema(
        title = "Tz1",
        value_type = String,
        example = json!("tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU")
    )]
    Tz1(ContractTz1Hash),
    #[schema(
        title = "Tz2",
        value_type = String,
        example =  json!("tz2KDvEL9fuvytRfe1cVVDo1QfDfaBktGNkh")
    )]
    Tz2(ContractTz2Hash),
    #[schema(
        title = "Tz3",
        value_type = String,
        example = json!("tz3QxNCB8HgxJyp5V9ZmCVGcTm6BzYc14k9C")
    )]
    Tz3(ContractTz3Hash),
}
