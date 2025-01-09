use crate::types::account::{Nonce, ParsedCode};
use crate::types::hash::Blake2b;
use crate::types::public_key::PublicKey;
use crate::types::public_key_hash::PublicKeyHash;
use crate::types::signature::Signature;
use http::{HeaderMap, Method, Uri};
use jstz_api::http::body::HttpBody;
use jstz_proto::context::account::Amount;
use jstz_proto::operation::{
    Content as ContentInternal, DeployFunction as DeployFunctionInternal,
    Operation as OperationInternal, RunFunction as RunFunctionInternal,
    SignedOperation as SignedOperationInternal,
};
use jstz_utils::api_map_to;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub type OperationHash = Blake2b;
pub type Address = PublicKeyHash;

#[api_map_to(OperationInternal)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Operation {
    pub source: Address,
    pub nonce: Nonce,
    pub content: Content,
}

#[api_map_to(DeployFunctionInternal)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, ToSchema)]
pub struct DeployFunction {
    /// Smart function code
    pub function_code: ParsedCode,
    /// Amount of tez to credit to the smart function account, debited from the sender
    pub account_credit: Amount,
}

#[api_map_to(RunFunctionInternal)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, ToSchema)]
#[schema(description = "Request used to run a smart function. \
    The target smart function is given by the host part of the uri. \
    The rest of the attributes will be handled by the smart function itself.")]
pub struct RunFunction {
    /// Smart function URI in the form tezos://{smart_function_address}/rest/of/path
    #[serde(with = "http_serde::uri")]
    #[schema(
            value_type = String,
            format = Uri,
            examples("tezos://tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU/nfts?status=sold"),
        )]
    pub uri: Uri,
    /// Any valid HTTP method
    #[serde(with = "http_serde::method")]
    #[schema(
            value_type = String,
            examples("GET", "POST", "PUT", "UPDATE", "DELETE"),
        )]
    pub method: Method,
    /// Any valid HTTP headers
    #[serde(with = "http_serde::header_map")]
    #[schema(schema_with= openapi::http_headers)]
    pub headers: HeaderMap,
    #[schema(schema_with = openapi::http_body_schema)]
    pub body: HttpBody,
    /// Maximum amount of gas that is allowed for the execution of this operation
    pub gas_limit: usize,
}

#[api_map_to(ContentInternal)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, ToSchema)]
#[serde(tag = "_type")]
pub enum Content {
    #[schema(title = "DeployFunction")]
    DeployFunction(DeployFunction),
    #[schema(title = "RunFunction")]
    RunFunction(RunFunction),
}

#[api_map_to(SignedOperationInternal)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct SignedOperation {
    pub public_key: PublicKey,
    pub signature: Signature,
    pub inner: Operation,
}

pub mod openapi {
    use utoipa::{
        openapi::{schema::AdditionalProperties, Array, Object, ObjectBuilder},
        schema,
    };

    pub fn http_body_schema() -> Array {
        schema!(Option<Vec<u8>>).build()
    }

    pub fn http_headers() -> Object {
        ObjectBuilder::new()
            .additional_properties(Some(AdditionalProperties::FreeForm(true)))
            .description(Some("Any valid HTTP headers"))
            .build()
    }
}
