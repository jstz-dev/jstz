use crate::{
    context::{
        account::{Account, Amount, Nonce, ParsedCode},
        new_account::NewAddress,
    },
    Error, Result,
};
use http::{HeaderMap, Method, Uri};
use jstz_api::http::body::HttpBody;
use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::{
    hash::Blake2b, public_key::PublicKey, public_key_hash::PublicKeyHash,
    signature::Signature,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Operation {
    pub source: PublicKeyHash,
    pub nonce: Nonce,
    pub content: Content,
}

pub type OperationHash = Blake2b;

impl Operation {
    /// Returns the source of the operation
    pub fn source(&self) -> &PublicKeyHash {
        &self.source
    }

    /// Returns the nonce of the operation
    pub fn nonce(&self) -> &Nonce {
        &self.nonce
    }

    /// Verify the nonce of the operation
    /// Returns the operation's
    pub fn verify_nonce(
        &self,
        rt: &impl HostRuntime,
        tx: &mut Transaction,
    ) -> Result<()> {
        let next_nonce = Account::nonce(rt, tx, &NewAddress::User(self.source.clone()))?;

        if self.nonce == *next_nonce {
            next_nonce.increment();
            Ok(())
        } else {
            Err(Error::InvalidNonce)
        }
    }

    /// Computes the operation hash.
    /// This is the hash which the client should sign
    pub fn hash(&self) -> OperationHash {
        let Operation {
            source,
            nonce,
            content,
        } = self;
        match content {
            Content::DeployFunction(DeployFunction {
                function_code,
                account_credit,
            }) => Blake2b::from(
                format!("{}{}{}{}", source, nonce, function_code, account_credit)
                    .as_bytes(),
            ),
            Content::RunFunction(RunFunction {
                uri,
                method,
                headers,
                body,
                ..
            }) => Blake2b::from(
                format!(
                    "{}{}{}{}{:?}{:?}",
                    source, nonce, uri, method, headers, body
                )
                .as_bytes(),
            ),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, ToSchema)]
pub struct DeployFunction {
    /// Smart function code
    pub function_code: ParsedCode,
    /// Amount of tez to credit to the smart function account, debited from the sender
    pub account_credit: Amount,
}

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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, ToSchema)]
#[serde(tag = "_type")]
pub enum Content {
    #[schema(title = "DeployFunction")]
    DeployFunction(DeployFunction),
    #[schema(title = "RunFunction")]
    RunFunction(RunFunction),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct SignedOperation {
    pub public_key: PublicKey,
    signature: Signature,
    inner: Operation,
}

impl SignedOperation {
    pub fn new(public_key: PublicKey, signature: Signature, inner: Operation) -> Self {
        Self {
            public_key,
            signature,
            inner,
        }
    }

    pub fn hash(&self) -> Blake2b {
        self.inner.hash()
    }

    pub fn verify(self) -> Result<Operation> {
        // FIXME: Adding signature verification kills to the rollup???!??!?!?!
        let hash = self.inner.hash();
        self.signature.verify(&self.public_key, hash.as_ref())?;

        Ok(self.inner)
    }
}

pub mod external {
    use tezos_smart_rollup::michelson::ticket::TicketHash;

    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Deposit {
        // Inbox message id is unique to each message and
        // suitable as a nonce
        pub inbox_id: u32,
        // Amount to deposit
        pub amount: Amount,
        // Receiver address
        pub receiver: NewAddress,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct FaDeposit {
        // Inbox message id is unique to each message and
        // suitable as a nonce
        pub inbox_id: u32,
        // Amount to deposit
        pub amount: Amount,
        // Final deposit receiver address
        pub receiver: NewAddress,
        // Optional proxy contract
        pub proxy_smart_function: Option<NewAddress>,
        // Ticket hash
        pub ticket_hash: TicketHash,
    }

    impl FaDeposit {
        fn json(&self) -> serde_json::Value {
            serde_json::json!({
                "receiver": self.receiver,
                "amount": self.amount,
                "ticketHash": self.ticket_hash.to_string(),
            })
        }

        pub fn to_http_body(&self) -> HttpBody {
            let body = self.json();
            Some(String::as_bytes(&body.to_string()).to_vec())
        }

        pub fn hash(&self) -> OperationHash {
            let seed = self.inbox_id.to_be_bytes();
            Blake2b::from(seed.as_slice())
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExternalOperation {
    Deposit(external::Deposit),
    FaDeposit(external::FaDeposit),
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

#[cfg(test)]
mod test {
    use super::{DeployFunction, RunFunction};
    use crate::{context::account::ParsedCode, operation::Content};
    use http::{HeaderMap, Method, Uri};
    use serde_json::json;

    fn run_function_content() -> Content {
        let body = r#""value":1""#.to_string().into_bytes();
        Content::RunFunction(RunFunction {
            uri: Uri::try_from(
                "tezos://tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU/nfts?status=sold",
            )
            .unwrap(),
            method: Method::POST,
            headers: HeaderMap::new(),
            body: Some(body),
            gas_limit: 10000,
        })
    }

    fn deploy_function_content() -> Content {
        let raw_code =
            r#"export default handler = () => new Response("hello world!");"#.to_string();
        let function_code = ParsedCode::try_from(raw_code).unwrap();
        let account_credit = 100000;
        Content::DeployFunction(DeployFunction {
            function_code,
            account_credit,
        })
    }

    #[test]
    fn test_encoding_run_function_json_round_trip() {
        let run_function = run_function_content();
        let json = serde_json::to_value(&run_function).unwrap();
        assert_eq!(
            json,
            json!({
                "_type":"RunFunction",
                "body":[34,118,97,108,117,101,34,58,49,34],
                "gas_limit":10000,
                "headers":{},
                "method":"POST",
                "uri":"tezos://tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU/nfts?status=sold"
            })
        );
        let decoded = serde_json::from_value::<Content>(json).unwrap();
        assert_eq!(run_function, decoded);
    }

    #[test]
    #[ignore = "Fails because deserialization cannot handle untagged crypto enums"]
    // FIXME: https://linear.app/tezos/issue/JSTZ-272/fix-binary-round-trip-for-tezos-cryptos
    fn test_run_function_bin_round_trip() {
        let run_function = run_function_content();
        let binary = bincode::serialize(&run_function).unwrap();
        let bin_decoded = bincode::deserialize::<Content>(binary.as_ref()).unwrap();
        assert_eq!(run_function, bin_decoded);
    }

    #[test]
    fn test_deploy_function_json_round_trip() {
        let deploy_function = deploy_function_content();
        let json = serde_json::to_value(&deploy_function).unwrap();
        assert_eq!(
            json,
            json!({
                "_type":"DeployFunction",
                "account_credit":100000,
                "function_code":"export default handler = () => new Response(\"hello world!\");"
            })
        );
        let decoded = serde_json::from_value::<Content>(json).unwrap();
        assert_eq!(deploy_function, decoded);
    }

    #[test]
    #[ignore = "Fails because deserialization cannot handle untagged crypto enums"]
    // FIXME: https://linear.app/tezos/issue/JSTZ-272/fix-binary-round-trip-for-tezos-cryptos
    fn test_deploy_function_bin_round_trip() {
        let deploy_function = deploy_function_content();
        let binary = bincode::serialize(&deploy_function).unwrap();
        let bin_decoded = bincode::deserialize::<Content>(binary.as_ref()).unwrap();
        assert_eq!(deploy_function, bin_decoded);
    }
}
