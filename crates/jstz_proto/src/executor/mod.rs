#[cfg(feature = "v2_runtime")]
use crate::{
    operation::OracleResponse,
    receipt::{OracleResponseReceipt, ReceiptContent},
    runtime::PROTOCOL_CONTEXT,
};

use crate::{
    operation::{
        self, Content, InternalOperation, Operation, OperationHash, SignedOperation,
    },
    receipt::{self, Receipt},
    Error, Result,
};
use futures::future::FutureExt;
use jstz_core::{host::HostRuntime, kv::Transaction, reveal_data::RevealData};
use jstz_crypto::{hash::Blake2b, public_key::PublicKey};
use tezos_crypto_rs::hash::ContractKt1Hash;
pub mod deposit;
pub mod fa_deposit;
pub mod fa_withdraw;
pub mod smart_function;
pub mod withdraw;

async fn execute_operation_inner(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    op: Operation,
    _ticketer: &ContractKt1Hash,
    injector: &PublicKey,
) -> Result<(OperationHash, receipt::ReceiptContent)> {
    let op_hash = op.hash();
    let source = op.source();

    match op.content {
        operation::Content::DeployFunction(deployment) => {
            let result = smart_function::deploy::execute(hrt, tx, &source, deployment)?;
            Ok((op_hash, receipt::ReceiptContent::DeployFunction(result)))
        }
        operation::Content::RunFunction(run) => {
            let result =
                smart_function::run::execute(hrt, tx, &source, run, op_hash.clone())
                    .await?;
            Ok((op_hash, receipt::ReceiptContent::RunFunction(result)))
        }
        operation::Content::RevealLargePayload(reveal) => {
            if op.public_key != *injector {
                return Err(Error::InvalidInjector);
            }
            let signed_op = RevealData::reveal_and_decode::<_, SignedOperation>(
                hrt,
                &reveal.root_hash,
            )?;
            signed_op.verify()?;
            signed_op.verify_nonce(hrt, tx)?;
            let revealed_op: Operation = signed_op.into();
            if reveal.reveal_type == revealed_op.content().try_into()? {
                return execute_operation_inner(
                    hrt,
                    tx,
                    revealed_op,
                    _ticketer,
                    injector,
                )
                .boxed_local()
                .await;
            }
            Err(Error::RevealTypeMismatch)
        }
        #[cfg(feature = "v2_runtime")]
        operation::Content::OracleResponse(OracleResponse {
            request_id,
            response,
        }) => {
            let oracle_ctx = PROTOCOL_CONTEXT
                .get()
                .expect("Protocol context should be initialized")
                .oracle();
            let mut oracle = oracle_ctx.lock();
            if &op.public_key != oracle.public_key() {
                // [execute_operation] verifies SignedOperation signature
                // so we only need to check pk equality
                return Err(Error::InvalidOracleKey);
            }
            oracle
                .respond(hrt, request_id.clone(), response)
                .map_err(|e| Error::V2Error(e.into()))?;

            Ok((
                op_hash.clone(),
                ReceiptContent::OracleResponse(OracleResponseReceipt { request_id }),
            ))
        }
    }
}

pub async fn execute_internal_operation(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    internal_operation: InternalOperation,
) -> Receipt {
    match internal_operation {
        InternalOperation::Deposit(deposit) => deposit::execute(hrt, tx, deposit),
        InternalOperation::FaDeposit(fa_deposit) => {
            fa_deposit::execute(hrt, tx, fa_deposit).await
        }
    }
}

pub async fn execute_operation(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    signed_operation: SignedOperation,
    ticketer: &ContractKt1Hash,
    injector: &PublicKey,
) -> Receipt {
    let validity = signed_operation
        .verify()
        .and_then(|_| signed_operation.verify_nonce(hrt, tx));
    let op = signed_operation.into();
    let op_hash = resolve_operation_hash(&op);
    let result = match validity {
        Ok(_) => execute_operation_inner(hrt, tx, op, ticketer, injector).await,
        Err(err) => Err(err),
    };
    result.map_or_else(
        |e| Receipt::new(op_hash, Err(e)),
        |(hash, content)| Receipt::new(hash, Ok(content)),
    )
}

fn resolve_operation_hash(op: &Operation) -> Blake2b {
    match &op {
        // If the operation is a reveal large payload operation, use the original operation hash
        Operation {
            content: Content::RevealLargePayload(reveal),
            ..
        } => reveal.original_op_hash.clone(),
        _ => op.hash(),
    }
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, Method, Uri};
    use jstz_core::{reveal_data::PreimageHash, BinEncodable};
    use jstz_crypto::{
        hash::Hash, public_key::PublicKey, public_key_hash::PublicKeyHash,
        secret_key::SecretKey,
    };
    #[cfg(feature = "v2_runtime")]
    use jstz_utils::{test_util::alice_keys, KeyPair};
    use operation::RevealType;
    use tezos_crypto_rs::hash::HashTrait;
    use tezos_smart_rollup_mock::MockHost;

    use super::*;
    #[cfg(feature = "v2_runtime")]
    use crate::runtime::v2::fetch::http::Request;
    use crate::{
        context::account::Nonce,
        operation::{Content, DeployFunction, RevealLargePayload, RunFunction},
        receipt::{ReceiptContent, ReceiptResult},
        HttpBody,
    };

    use crate::runtime::ParsedCode;
    #[cfg(feature = "v2_runtime")]
    mod response_test_utils {
        use super::*;
        use crate::operation::OracleResponse;
        use crate::runtime::{
            v2::fetch::http::{Body, Response},
            ProtocolContext,
        };

        pub fn host_and_tx() -> (MockHost, Transaction) {
            let mut host = MockHost::default();
            ProtocolContext::init_global(&mut host, 0).unwrap();
            let tx = Transaction::default();
            tx.begin();
            (host, tx)
        }

        pub fn oracle_keys() -> (PublicKey, SecretKey) {
            (
                PublicKey::from_base58(
                    "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
                )
                .unwrap(),
                SecretKey::from_base58(
                    "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
                )
                .unwrap(),
            )
        }

        pub fn empty_ok_response() -> Response {
            Response {
                status: 200,
                status_text: "OK".into(),
                headers: Vec::new(),
                body: Body::zero_capacity(),
            }
        }

        pub fn signed_oracle_response_op(
            request_id: u64,
            resp: Response,
            pk: &PublicKey,
            sk: &SecretKey,
        ) -> SignedOperation {
            let response_op = Operation {
                public_key: pk.clone(),
                nonce: 0.into(),
                content: OracleResponse {
                    request_id,
                    response: resp,
                }
                .into(),
            };
            SignedOperation::new(sk.sign(response_op.hash()).unwrap(), response_op)
        }

        pub fn dummy_ticketer() -> ContractKt1Hash {
            ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap()
        }
    }

    fn bootstrap1() -> (PublicKeyHash, PublicKey, SecretKey) {
        (
            PublicKeyHash::from_base58("tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx").unwrap(),
            PublicKey::from_base58(
                "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
            )
            .unwrap(),
            SecretKey::from_base58(
                "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
            )
            .unwrap(),
        )
    }

    fn bootstrap2() -> (PublicKeyHash, PublicKey, SecretKey) {
        (
            PublicKeyHash::from_base58("tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN").unwrap(),
            PublicKey::from_base58(
                "edpktzNbDAUjUk697W7gYg2CRuBQjyPxbEg8dLccYYwKSKvkPvjtV9",
            )
            .unwrap(),
            SecretKey::from_base58(
                "edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo",
            )
            .unwrap(),
        )
    }

    fn run_function_content() -> Content {
        let body = HttpBody::from_bytes(vec![0]);
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
        let account_credit = 0;
        Content::DeployFunction(DeployFunction {
            function_code,
            account_credit,
        })
    }

    fn make_signed_op(content: Content, pk: PublicKey, sk: SecretKey) -> SignedOperation {
        let deploy_op = Operation {
            public_key: pk,
            nonce: Nonce(0),
            content,
        };
        let sig = sk.sign(deploy_op.hash()).unwrap();
        SignedOperation::new(sig, deploy_op)
    }

    fn signed_rdc_op(
        root_hash: PreimageHash,
        pk: PublicKey,
        sk: SecretKey,
        original_op_hash: OperationHash,
    ) -> SignedOperation {
        let rdc_op = RevealLargePayload {
            root_hash,
            reveal_type: RevealType::DeployFunction,
            original_op_hash,
        };
        let rdc_op_content = rdc_op;
        let rdc_op: Operation = Operation {
            public_key: pk,
            nonce: Nonce(0),
            content: Content::RevealLargePayload(rdc_op_content),
        };
        let sig = sk.sign(rdc_op.hash()).unwrap();
        SignedOperation::new(sig, rdc_op)
    }

    fn make_data_available<T>(host: &mut MockHost, data: T) -> PreimageHash
    where
        T: BinEncodable + Clone + PartialEq + Eq + std::fmt::Debug,
    {
        RevealData::encode_and_prepare_preimages(&data, |_, page| {
            host.set_preimage(page);
        })
        .expect("should prepare preimages")
    }

    #[tokio::test]
    async fn reveals_large_payload_operation() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();
        let (_, pk1, sk1) = bootstrap1();
        let (_, pk2, sk2) = bootstrap2();
        let deploy_op = make_signed_op(deploy_function_content(), pk2, sk2);
        let root_hash = make_data_available(&mut host, deploy_op.clone());
        let rdc_op = signed_rdc_op(root_hash, pk1.clone(), sk1, deploy_op.hash());
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();
        let receipt =
            execute_operation(&mut host, &mut tx, rdc_op, &ticketer, &pk1).await;
        assert!(matches!(receipt.result, ReceiptResult::Success(_)));
    }

    #[tokio::test]
    async fn throws_error_if_reveal_type_not_supported() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();
        let (_, pk1, sk1) = bootstrap1();
        let (_, pk2, sk2) = bootstrap2();
        let run_op = make_signed_op(run_function_content(), pk2.clone(), sk2.clone());
        let root_hash = make_data_available(&mut host, run_op.clone());
        let rdc_op = signed_rdc_op(root_hash, pk1.clone(), sk1.clone(), run_op.hash());
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();
        let receipt =
            execute_operation(&mut host, &mut tx, rdc_op, &ticketer, &pk1).await;
        assert!(matches!(
            receipt.result,
            ReceiptResult::Failed(e) if e.contains("RevealNotSupported")
        ));
    }

    #[tokio::test]
    async fn throws_if_nonce_is_invalid() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();
        let (_, pk, sk) = bootstrap1();
        let deploy_op = make_signed_op(deploy_function_content(), pk.clone(), sk.clone());
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();
        let receipt =
            execute_operation(&mut host, &mut tx, deploy_op.clone(), &ticketer, &pk)
                .await;
        assert!(matches!(receipt.result, ReceiptResult::Success(_)));
        let receipt =
            execute_operation(&mut host, &mut tx, deploy_op, &ticketer, &pk).await;
        assert!(
            matches!(receipt.result, ReceiptResult::Failed(e) if e.contains("InvalidNonce"))
        );
    }

    #[tokio::test]
    async fn throws_if_injector_is_invalid() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();
        let (_, pk1, sk1) = bootstrap1();
        let (_, pk2, sk2) = bootstrap2();
        let deploy_op = make_signed_op(deploy_function_content(), pk1.clone(), sk1);
        let root_hash = make_data_available(&mut host, deploy_op.clone());
        let rdc_op = signed_rdc_op(root_hash, pk2.clone(), sk2, deploy_op.hash());
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();
        let receipt =
            execute_operation(&mut host, &mut tx, rdc_op.clone(), &ticketer, &pk1).await;
        assert!(
            matches!(receipt.clone().result, ReceiptResult::Failed(e) if e.contains("InvalidInjector"))
        );
        assert_eq!(receipt.hash().to_string(), deploy_op.hash().to_string());
    }

    #[tokio::test]
    async fn run_function_with_invalid_scheme_fails() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();
        let (_, pk1, sk1) = bootstrap1();
        let run_op = make_signed_op(
            Content::RunFunction(RunFunction {
                uri: Uri::try_from(
                    "tezos://tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU/nfts?status=sold",
                )
                .unwrap(),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: HttpBody::empty(),
                gas_limit: 10000,
            }),
            pk1.clone(),
            sk1,
        );
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();

        let receipt =
            execute_operation(&mut host, &mut tx, run_op, &ticketer, &pk1).await;

        if cfg!(feature = "v2_runtime") {
            if let ReceiptResult::Success(ReceiptContent::RunFunction(r)) =
                receipt.clone().result
            {
                assert_eq!(r.status_code, 500);
                assert_eq!(
                    String::from_utf8(r.body.unwrap()).unwrap(),
                    "{\"class\":\"TypeError\",\"message\":\"Unsupported scheme 'tezos'\"}"
                );
            } else {
                unreachable!()
            }
        } else {
            assert!(
                matches!(receipt.clone().result, ReceiptResult::Failed(e) if e.contains("InvalidScheme"))
            );
        }
    }

    #[cfg(feature = "v2_runtime")]
    #[tokio::test]
    async fn operation_response_successful() {
        use response_test_utils as utils;

        let (mut host, mut tx) = utils::host_and_tx();
        let caller = PublicKeyHash::digest(&[0u8; 20]).unwrap();
        let request = Request {
            method: "GET".into(),
            url: "http://example.com".parse().unwrap(),
            headers: Vec::new(),
            body: None,
        };
        let rx = {
            let oracle_ctx = PROTOCOL_CONTEXT.get().unwrap().oracle();
            let mut oracle = oracle_ctx.lock();
            oracle
                .send_request(&mut host, &mut tx, &caller, request)
                .unwrap()
        };

        let (oracle_pk, oracle_sk) = utils::oracle_keys();
        let resp = utils::empty_ok_response();
        let signed_resp_op =
            utils::signed_oracle_response_op(0, resp.clone(), &oracle_pk, &oracle_sk);

        let ticketer = utils::dummy_ticketer();
        let injector = oracle_pk.clone();
        let receipt =
            execute_operation(&mut host, &mut tx, signed_resp_op, &ticketer, &injector)
                .await;
        let received_resp = rx.await.unwrap();
        assert_eq!(resp, received_resp);
        assert_eq!(format!("{:?}", receipt), "Receipt { hash: Blake2b([43, 173, 229, 86, 39, 53, 239, 75, 89, 125, 160, 162, 17, 118, 230, 15, 219, 184, 198, 23, 222, 64, 225, 230, 221, 14, 103, 28, 175, 82, 199, 222]), result: Success(OracleResponse(OracleResponseReceipt { request_id: 0 })) }")
    }

    #[cfg(feature = "v2_runtime")]
    #[tokio::test]
    async fn operation_response_invalid_request_id() {
        use response_test_utils as utils;

        let (mut host, mut tx) = utils::host_and_tx();
        let (oracle_pk, oracle_sk) = utils::oracle_keys();
        let resp = utils::empty_ok_response();
        let signed_resp_op =
            utils::signed_oracle_response_op(21, resp.clone(), &oracle_pk, &oracle_sk);

        let ticketer = utils::dummy_ticketer();
        let injector = oracle_pk.clone();
        let receipt =
            execute_operation(&mut host, &mut tx, signed_resp_op, &ticketer, &injector)
                .await;
        assert!(matches!(
            receipt,
            Receipt {
                result: ReceiptResult::Failed(s),
                ..
            }
            if s == "Request Id does not exist or has expired".to_string()
        ));
    }

    #[cfg(feature = "v2_runtime")]
    #[tokio::test]
    async fn operation_response_invalid_pk() {
        use response_test_utils as utils;

        let (mut host, mut tx) = utils::host_and_tx();

        let KeyPair(invalid_pk, invalid_sk) = alice_keys();

        let resp = utils::empty_ok_response();
        let signed_resp_op =
            utils::signed_oracle_response_op(0, resp.clone(), &invalid_pk, &invalid_sk);

        let ticketer = utils::dummy_ticketer();
        let injector = utils::oracle_keys().0; // valid oracle pk
        let receipt =
            execute_operation(&mut host, &mut tx, signed_resp_op, &ticketer, &injector)
                .await;
        assert!(matches!(
            receipt,
            Receipt {
                result: ReceiptResult::Failed(s),
                ..
            }
            if s == "InvalidOracleKey".to_string()
        ));
    }

    #[cfg(feature = "v2_runtime")]
    #[tokio::test]
    async fn operation_response_invalid_signature() {
        use response_test_utils as utils;

        let (mut host, mut tx) = utils::host_and_tx();

        let (oracle_pk, _oracle_sk) = utils::oracle_keys();
        let KeyPair(_, invalid_sk) = alice_keys();
        let resp = utils::empty_ok_response();
        let signed_resp_op =
            utils::signed_oracle_response_op(0, resp.clone(), &oracle_pk, &invalid_sk);

        let ticketer = utils::dummy_ticketer();
        let injector = oracle_pk.clone();
        let receipt =
            execute_operation(&mut host, &mut tx, signed_resp_op, &ticketer, &injector)
                .await;
        assert!(matches!(
            receipt,
            Receipt {
                result: ReceiptResult::Failed(s),
                ..
            }
            if s.contains("Ed25519 error: signature error")
        ));
    }
}
