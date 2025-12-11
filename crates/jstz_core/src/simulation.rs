use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
#[derive(
    Debug, Serialize, Deserialize, PartialEq, Eq, Encode, ToSchema, Decode, Clone,
)]
pub struct SimulationRequest {
    request_id: u32,
}

impl SimulationRequest {
    pub fn new(request_id: u32) -> Self {
        Self { request_id }
    }
}
