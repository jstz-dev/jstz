#[cfg(feature = "v2_runtime")]
use crate::runtime::v2::fetch::http::Response;
use crate::runtime::ParsedCode;
use crate::{
    context::account::{Account, Address, Amount, Nonce},
    Error, HttpBody, Result,
};
use bincode::{Decode, Encode};
use derive_more::{Deref, Display, From};
use http::{HeaderMap, Method, Uri};

#[cfg(feature = "v2_runtime")]
use crate::runtime::v2::oracle::request::RequestId;

use jstz_core::{host::HostRuntime, kv::Transaction, reveal_data::PreimageHash};
use jstz_crypto::{
    hash::Blake2b, public_key::PublicKey, public_key_hash::PublicKeyHash,
    signature::Signature,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema, Encode, Decode, Clone,
)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    /// The public key of the account which was used to sign the operation
    pub public_key: PublicKey,
    #[bincode(with_serde)]
    /// Nonce is used to avoid replay attacks.
    pub nonce: Nonce,
    /// The content of the operation
    pub content: Content,
}

pub type OperationHash = Blake2b;

impl Operation {
    /// Returns the source of the operation
    pub fn source(&self) -> PublicKeyHash {
        (&self.public_key).into()
    }

    /// Returns the nonce of the operation
    pub fn nonce(&self) -> &Nonce {
        &self.nonce
    }

    pub fn content(&self) -> &Content {
        &self.content
    }

    /// Verify the nonce of the operation
    /// Returns the operation's
    pub fn verify_nonce(
        &self,
        rt: &impl HostRuntime,
        tx: &mut Transaction,
    ) -> Result<()> {
        let mut next_nonce = Account::nonce(rt, tx, &self.source())?;

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
            public_key,
            nonce,
            content,
        } = self;
        match content {
            Content::DeployFunction(DeployFunction {
                function_code,
                account_credit,
            }) => Blake2b::from(
                format!("{public_key}{nonce}{function_code}{account_credit}").as_bytes(),
            ),
            Content::RunFunction(RunFunction {
                uri,
                method,
                headers,
                body,
                ..
            }) => Blake2b::from(
                format!("{public_key}{nonce}{uri}{method}{headers:?}{body:?}").as_bytes(),
            ),
            Content::RevealLargePayload(RevealLargePayload {
                root_hash,
                reveal_type,
                original_op_hash,
            }) => Blake2b::from(
                format!("{public_key}{nonce}{root_hash}{reveal_type}{original_op_hash}",)
                    .as_bytes(),
            ),
            #[cfg(feature = "v2_runtime")]
            Content::OracleResponse(OracleResponse {
                request_id,
                response,
            }) => Blake2b::from(
                format!("{}{}{}{:?}", public_key, nonce, request_id, response).as_bytes(),
            ),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct RunFunction {
    /// Smart function URI in the form jstz://{smart_function_address}/rest/of/path
    #[serde(with = "http_serde::uri")]
    #[schema(
            value_type = String,
            format = Uri,
            examples("jstz://tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU/nfts?status=sold"),
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
    #[schema(schema_with = openapi::request_headers)]
    pub headers: HeaderMap,
    pub body: HttpBody,
    /// Maximum amount of gas that is allowed for the execution of this operation
    pub gas_limit: usize,
}

#[derive(Debug, PartialEq, Eq, Clone, ToSchema, Serialize, Deserialize, Display)]
pub enum RevealType {
    DeployFunction,
}

impl TryFrom<&Content> for RevealType {
    type Error = Error;
    fn try_from(value: &Content) -> Result<Self> {
        match *value {
            Content::DeployFunction(_) => Ok(RevealType::DeployFunction),
            _ => Err(Error::RevealNotSupported),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, ToSchema, Serialize, Deserialize)]
#[schema(
    description = "An operation to reveal an operation with a large payload of type `RevealType`. \
            The root hash is the hash of the SignedOperation and the data is assumed to be available."
)]
#[serde(rename_all = "camelCase")]
pub struct RevealLargePayload {
    /// The root hash of the preimage of the operation used to reveal the operation data.
    #[schema(value_type = String)]
    pub root_hash: PreimageHash,
    /// The type of operation being revealed.
    #[schema(value_type = String, example = "DeployFunction")]
    pub reveal_type: RevealType,
    /// The original operation hash that is being revealed.
    pub original_op_hash: OperationHash,
}

#[cfg(feature = "v2_runtime")]
#[derive(Debug, PartialEq, Eq, Clone, ToSchema, Serialize, Deserialize)]
#[schema(description = "Response to an OracleRequest sent by the enshrined Oracle node")]
#[serde(rename_all = "camelCase")]
pub struct OracleResponse {
    /// The request id of the OracleRequest that is being responded to
    pub request_id: RequestId,
    /// The response to the OracleRequest
    #[schema(value_type = String)]
    pub response: Response,
}

#[derive(
    Debug, From, Serialize, Deserialize, PartialEq, Eq, Clone, ToSchema, Encode, Decode,
)]
#[serde(tag = "_type")]
pub enum Content {
    #[schema(title = "DeployFunction")]
    DeployFunction(#[bincode(with_serde)] DeployFunction),
    #[schema(title = "RunFunction")]
    RunFunction(#[bincode(with_serde)] RunFunction),
    #[schema(title = "RevealLargePayload")]
    RevealLargePayload(#[bincode(with_serde)] RevealLargePayload),
    #[cfg(feature = "v2_runtime")]
    #[schema(title = "OracleResponse")]
    OracleResponse(#[bincode(with_serde)] OracleResponse),
}

impl Content {
    pub fn new_reveal_large_payload(
        root_hash: PreimageHash,
        reveal_type: RevealType,
        original_op_hash: OperationHash,
    ) -> Self {
        Content::RevealLargePayload(RevealLargePayload {
            root_hash,
            reveal_type,
            original_op_hash,
        })
    }
}

#[derive(
    Debug, Deref, Serialize, Deserialize, PartialEq, Eq, ToSchema, Encode, Decode, Clone,
)]
pub struct SignedOperation {
    signature: Signature,
    #[deref]
    inner: Operation,
}

impl SignedOperation {
    pub fn new(signature: Signature, inner: Operation) -> Self {
        Self { signature, inner }
    }

    pub fn hash(&self) -> Blake2b {
        self.inner.hash()
    }

    pub fn verify(&self) -> Result<()> {
        let hash = self.inner.hash();
        Ok(self
            .signature
            .verify(&self.inner.public_key, hash.as_ref())?)
    }

    pub fn verify_ref(&self) -> Result<&Operation> {
        let hash = self.inner.hash();
        self.signature
            .verify(&self.inner.public_key, hash.as_ref())?;

        Ok(&self.inner)
    }
}

impl From<SignedOperation> for Operation {
    fn from(value: SignedOperation) -> Self {
        value.inner
    }
}

pub mod internal {
    use tezos_smart_rollup::michelson::ticket::TicketHash;

    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
    pub struct InboxId {
        // L1 inbox message level
        pub l1_level: u32,
        // Unique id of inbox message (per level)
        pub l1_message_id: u32,
    }

    impl InboxId {
        pub fn to_bytes(&self) -> [u8; 8] {
            let mut buf = [0u8; 8];
            buf[..4].copy_from_slice(&self.l1_level.to_be_bytes());
            buf[4..].copy_from_slice(&self.l1_message_id.to_be_bytes());
            buf
        }

        pub fn hash(&self) -> OperationHash {
            let seed = self.to_bytes();
            Blake2b::from(seed.as_slice())
        }
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
    pub struct Deposit {
        // Inbox message id
        pub inbox_id: InboxId,
        // Amount to deposit
        pub amount: Amount,
        // Receiver address
        pub receiver: Address,
        /// Source of the deposit message. Must be a user address
        pub source: PublicKeyHash,
    }

    impl Deposit {
        pub fn hash(&self) -> OperationHash {
            self.inbox_id.hash()
        }
    }

    #[derive(Debug, PartialEq, Eq, Clone)]
    pub struct FaDeposit {
        // Inbox message id
        pub inbox_id: InboxId,
        // Amount to deposit
        pub amount: Amount,
        // Final deposit receiver address
        pub receiver: Address,
        // Optional proxy contract
        pub proxy_smart_function: Option<Address>,
        // Ticket hash
        pub ticket_hash: TicketHash,
        /// Source of the deposit message. Must be a user address
        pub source: PublicKeyHash,
    }

    impl FaDeposit {
        fn json(&self) -> serde_json::Value {
            serde_json::json!({
                "receiver": self.receiver,
                "amount": self.amount,
                "ticketHash": self.ticket_hash.to_string(),
                "source": self.source,
            })
        }

        pub fn to_http_body(&self) -> HttpBody {
            self.json().into()
        }

        pub fn hash(&self) -> OperationHash {
            self.inbox_id.hash()
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InternalOperation {
    Deposit(internal::Deposit),
    FaDeposit(internal::FaDeposit),
}

pub mod openapi {
    use utoipa::{
        openapi::{schema::AdditionalProperties, Object, ObjectBuilder, RefOr, Schema},
        schema,
    };

    use crate::executor::smart_function::{X_JSTZ_AMOUNT, X_JSTZ_TRANSFER};

    fn http_headers(
        properties: Vec<(impl Into<String>, impl Into<RefOr<Schema>>)>,
    ) -> Object {
        let mut builder = ObjectBuilder::new();
        for (property_name, component) in properties {
            builder = builder.property(property_name, component);
        }
        builder
            .additional_properties(Some(AdditionalProperties::FreeForm(true)))
            .description(Some("Any valid HTTP headers"))
            .build()
    }

    pub fn request_headers() -> Object {
        http_headers(vec![(
            X_JSTZ_TRANSFER,
            schema!(u64)
                .minimum(Some(1))
                .description(Some("Amount in mutez to transfer on request")),
        )])
    }

    pub fn response_headers() -> Object {
        http_headers(vec![(
            X_JSTZ_AMOUNT,
            schema!(u64)
                .minimum(Some(1))
                .read_only(Some(true))
                .description(Some("Amount in mutez that was transferred on response")),
        )])
    }
}

#[cfg(test)]
mod test {
    use super::{Content, DeployFunction, RevealLargePayload, RevealType, RunFunction};
    use super::{Operation, SignedOperation};
    use crate::context::account::{Account, Address, Nonce};
    use crate::operation::internal::FaDeposit;
    use crate::operation::OperationHash;
    use crate::runtime::ParsedCode;
    use crate::HttpBody;
    use http::{HeaderMap, Method, Uri};
    use jstz_core::reveal_data::PreimageHash;
    use jstz_core::{kv::Transaction, BinEncodable};
    use jstz_crypto::hash::Hash;
    use jstz_crypto::{public_key::PublicKey, public_key_hash::PublicKeyHash};
    use jstz_mock::host::JstzMockHost;
    #[cfg(feature = "v2_runtime")]
    use jstz_utils::{test_util::alice_keys, KeyPair};
    use serde_json::json;
    use tezos_smart_rollup::michelson::ticket::TicketHash;

    fn run_function_content() -> Content {
        let body = HttpBody::from_string(r#"{"value":1"}"#.to_string());
        Content::RunFunction(RunFunction {
            uri: Uri::try_from(
                "jstz://tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU/nfts?status=sold",
            )
            .unwrap(),
            method: Method::POST,
            headers: HeaderMap::new(),
            body,
            gas_limit: 10000,
        })
    }

    fn deploy_function_content() -> Content {
        let raw_code =
            r#"export default () => new Response("hello world!");"#.to_string();
        let function_code = ParsedCode::try_from(raw_code).unwrap();
        let account_credit = 100000;
        Content::DeployFunction(DeployFunction {
            function_code,
            account_credit,
        })
    }

    fn dummy_content() -> Content {
        // Simply picks one the existing test content we have
        run_function_content()
    }

    #[test]
    fn test_encoding_run_function_json_round_trip() {
        let run_function = run_function_content();
        let json = serde_json::to_value(&run_function).unwrap();
        assert_eq!(
            json,
            json!({
                "_type":"RunFunction",
                "body":"eyJ2YWx1ZSI6MSJ9",
                "gasLimit":10000,
                "headers":{},
                "method":"POST",
                "uri":"jstz://tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU/nfts?status=sold"
            })
        );
        let decoded = serde_json::from_value::<Content>(json).unwrap();
        assert_eq!(run_function, decoded);
    }

    #[test]
    fn test_run_function_bin_round_trip() {
        let run_function = run_function_content();
        let binary = run_function.encode().unwrap();
        let bin_decoded = Content::decode(binary.as_slice()).unwrap();
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
                "accountCredit":100000,
                "functionCode":"export default () => new Response(\"hello world!\");"
            })
        );
        let decoded = serde_json::from_value::<Content>(json).unwrap();
        assert_eq!(deploy_function, decoded);
    }

    #[test]
    fn test_deploy_function_bin_round_trip() {
        let deploy_function = deploy_function_content();
        let binary = deploy_function.encode().unwrap();
        let bin_decoded = Content::decode(binary.as_slice()).unwrap();
        assert_eq!(deploy_function, bin_decoded);
    }

    fn mock_hrt_tx_with_nonces<'a>(
        nonces: impl IntoIterator<Item = &'a (PublicKeyHash, Nonce)>,
    ) -> (JstzMockHost, Transaction) {
        let mut hrt = JstzMockHost::default();
        let mut tx = Transaction::default();
        tx.begin();

        for (address, nonce) in nonces {
            let mut stored_nonce = Account::nonce(hrt.rt(), &mut tx, address).unwrap();
            *stored_nonce = *nonce;
        }

        (hrt, tx)
    }

    fn dummy_operation(public_key: PublicKey, nonce: Nonce) -> Operation {
        Operation {
            public_key,
            nonce,
            content: dummy_content(),
        }
    }

    #[test]
    fn test_verify_nonce_checks_and_increments_nonce() {
        let nonce = Nonce(42);
        let (mut hrt, mut tx) = mock_hrt_tx_with_nonces(&[(jstz_mock::pkh1(), nonce)]);

        let operation = dummy_operation(jstz_mock::pk1(), nonce);
        assert!(operation.verify_nonce(hrt.rt(), &mut tx).is_ok());

        let updated_nonce =
            Account::nonce(hrt.rt(), &mut tx, &jstz_mock::pkh1()).unwrap();
        assert_eq!(*updated_nonce, nonce.next());
    }

    #[test]
    fn test_verify_nonce_incorrect() {
        let (mut hrt, mut tx) =
            mock_hrt_tx_with_nonces(&[(jstz_mock::pkh1(), Nonce(1337))]);

        let operation = dummy_operation(jstz_mock::pk1(), Nonce(42));
        assert!(operation.verify_nonce(hrt.rt(), &mut tx).is_err());
    }

    #[test]
    fn test_verify_nonce_prevents_replay() {
        let (mut hrt, mut tx) = mock_hrt_tx_with_nonces(&[(jstz_mock::pkh1(), Nonce(7))]);

        let operation = dummy_operation(jstz_mock::pk1(), Nonce(7));

        assert!(operation.verify_nonce(hrt.rt(), &mut tx).is_ok());

        // Replaying the operation fails
        assert!(operation.verify_nonce(hrt.rt(), &mut tx).is_err());
    }

    #[test]
    fn test_verify_signed_op_is_ok_for_valid_signature() {
        let operation = dummy_operation(jstz_mock::pk1(), Nonce::default());

        let hash = operation.hash();
        let signature = jstz_mock::sk1().sign(hash).unwrap();
        let signed_operation = SignedOperation::new(signature, operation);

        assert!(signed_operation.verify().is_ok())
    }

    #[test]
    fn test_verify_signed_op_is_err_with_bad_sig() {
        let operation = dummy_operation(jstz_mock::pk1(), Nonce::default());

        let signature = jstz_mock::sk1().sign(b"badsig").unwrap();
        let signed_operation = SignedOperation::new(signature, operation);

        assert!(signed_operation.verify().is_err())
    }

    #[test]
    fn test_verify_signed_op_is_err_when_signed_by_other() {
        let operation = dummy_operation(jstz_mock::pk1(), Nonce::default());

        let signature = jstz_mock::sk2().sign(operation.hash()).unwrap();
        let signed_operation = SignedOperation::new(signature, operation);

        assert!(signed_operation.verify().is_err())
    }

    #[test]
    fn test_verify_signed_op_is_err_with_tampered_op() {
        let mut operation = dummy_operation(jstz_mock::pk1(), Nonce::default());

        let hash = operation.hash();
        let signature = jstz_mock::sk1().sign(hash).unwrap();

        // Be evil, say the operation is from someone else
        operation.public_key = jstz_mock::pk2();

        let signed_operation = SignedOperation::new(signature, operation);
        assert!(signed_operation.verify().is_err())
    }

    #[test]
    fn test_reveal_large_payload_operation_json_round_trip() {
        let reveal_large_payload_operation =
            Content::RevealLargePayload(RevealLargePayload {
                root_hash: PreimageHash::default(),
                reveal_type: RevealType::DeployFunction,
                original_op_hash: OperationHash::default(),
            });

        let json = serde_json::to_value(&reveal_large_payload_operation).unwrap();

        // Check the structure without hardcoding the exact serialization of root_hash
        let json_obj = json.as_object().unwrap();
        assert_eq!(json_obj.get("_type").unwrap(), "RevealLargePayload");
        assert_eq!(json_obj.get("revealType").unwrap(), "DeployFunction");
        assert!(json_obj.contains_key("rootHash"));

        let decoded = serde_json::from_value::<Content>(json).unwrap();
        assert_eq!(reveal_large_payload_operation, decoded);
    }

    #[test]
    fn test_reveal_large_payload_operation_bin_round_trip() {
        let reveal_large_payload_operation =
            Content::RevealLargePayload(RevealLargePayload {
                root_hash: PreimageHash::default(),
                reveal_type: RevealType::DeployFunction,
                original_op_hash: OperationHash::default(),
            });

        let binary = reveal_large_payload_operation.encode().unwrap();
        let bin_decoded = Content::decode(binary.as_slice()).unwrap();
        assert_eq!(reveal_large_payload_operation, bin_decoded);
    }

    #[cfg(feature = "v2_runtime")]
    #[test]
    fn test_oracle_response_signed_operation_json_round_trip() {
        use http::HeaderValue;

        use crate::runtime::v2::fetch::http::{convert_header_map, Response};

        use super::OracleResponse;
        let mut header_map = HeaderMap::new();
        header_map.append("test1", HeaderValue::from_str("value1").unwrap());
        header_map.append("test2", HeaderValue::from_str("value2").unwrap());
        header_map.append("test2", HeaderValue::from_str("value3").unwrap());
        let KeyPair(alice_pk, alice_sk) = alice_keys();
        let headers = convert_header_map(header_map);
        let op = Operation {
            public_key: alice_pk,
            nonce: 21943045950.into(),
            content: Content::OracleResponse(OracleResponse {
                request_id: 284958,
                response: Response {
                    status: 404,
                    status_text: "Not Found".into(),
                    headers: headers,
                    body: vec![].into(),
                },
            }),
        };
        let signature = alice_sk.sign(op.hash()).unwrap();
        let signed_op = SignedOperation {
            signature,
            inner: op,
        };
        let json = serde_json::to_vec(&signed_op).unwrap();
        let decoded: SignedOperation = serde_json::from_slice(json.as_slice()).unwrap();

        assert_eq!(signed_op, decoded)
    }

    #[test]
    fn fa_deposit_json() {
        let d = FaDeposit {
            inbox_id: 1,
            amount: 10,
            source: PublicKeyHash::from_base58("tz1ia78UBMgdmVf8b2vu5y8Rd148p9e2yn2h")
                .unwrap(),
            receiver: Address::from_base58("tz1W8rEphWEjMcD1HsxEhsBFocfMeGsW7Qxg")
                .unwrap(),
            proxy_smart_function: None,
            ticket_hash: TicketHash::try_from(
                "0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            )
            .unwrap(),
        };

        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&d.to_http_body().unwrap())
                .unwrap(),
            serde_json::json!({
                "receiver": "tz1W8rEphWEjMcD1HsxEhsBFocfMeGsW7Qxg",
                "amount": 10,
                "ticketHash": "0000000000000000000000000000000000000000000000000000000000000000",
                "source": "tz1ia78UBMgdmVf8b2vu5y8Rd148p9e2yn2h",
            })
        );
    }
}
