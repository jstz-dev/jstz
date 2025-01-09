use jstz_proto::context::account::{
    Nonce as NonceInternal, ParsedCode as ParsedCodeInternal,
};
use jstz_utils::api_map_to;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[api_map_to(NonceInternal)]
#[derive(
    Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema,
)]
pub struct Nonce(pub u64);

// Invariant: if code is present it parses successfully
#[api_map_to(ParsedCodeInternal)]
#[derive(Default, PartialEq, Eq, Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(
    format = "javascript",
    example = "export default (request) => new Response('Hello world!')"
)]
pub struct ParsedCode(pub String);
