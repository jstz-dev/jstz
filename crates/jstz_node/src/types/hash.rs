use jstz_crypto::hash::Blake2b as Blake2bInternal;
use jstz_utils::api_map_to;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[api_map_to(Blake2bInternal)]
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    Default,
    ToSchema,
)]
pub struct Blake2b(pub [u8; 32]);
