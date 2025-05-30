use crate::{
    operation::{
        self, Content, ExternalOperation, Operation, OperationHash, SignedOperation,
    },
    receipt::{self, Receipt},
    Error, Result,
};
use jstz_core::{
    host::{Host, HostRuntime},
    kv::Transaction,
    reveal_data::RevealData,
};
use jstz_crypto::public_key::PublicKey;
use tezos_crypto_rs::hash::ContractKt1Hash;
pub mod deposit;
pub mod fa_deposit;
pub mod fa_withdraw;
pub mod smart_function;
pub mod withdraw;

fn execute_operation_inner(
    hrt: &mut Host,
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
                smart_function::run::execute(hrt, tx, &source, run, op_hash.clone())?;
            Ok((op_hash, receipt::ReceiptContent::RunFunction(result)))
        }
        operation::Content::RevealLargePayload(reveal) => {
            if op.public_key != *injector {
                return Err(Error::InvalidInjector);
            }
            let revealed_op = RevealData::reveal_and_decode::<_, SignedOperation>(
                hrt,
                &reveal.root_hash,
            )?
            .verify()?;
            revealed_op.verify_nonce(hrt, tx)?;
            if reveal.reveal_type == revealed_op.content().try_into()? {
                return execute_operation_inner(
                    hrt,
                    tx,
                    revealed_op,
                    _ticketer,
                    injector,
                );
            }
            Err(Error::RevealTypeMismatch)
        }
    }
}

pub fn execute_external_operation(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    external_operation: ExternalOperation,
) -> Receipt {
    match external_operation {
        ExternalOperation::Deposit(deposit) => deposit::execute(hrt, tx, deposit),
        ExternalOperation::FaDeposit(fa_deposit) => {
            fa_deposit::execute(hrt, tx, fa_deposit)
        }
    }
}

pub fn execute_operation(
    hrt: &mut Host,
    tx: &mut Transaction,
    signed_operation: SignedOperation,
    ticketer: &ContractKt1Hash,
    injector: &PublicKey,
) -> Receipt {
    let op_hash = signed_operation.hash();
    let op: std::result::Result<Operation, Error> = signed_operation.verify();
    let op_hash = match &op {
        // If the operation is a reveal large payload operation, use the original operation hash
        Ok(Operation {
            content: Content::RevealLargePayload(reveal),
            ..
        }) => reveal.original_op_hash.clone(),
        _ => op_hash,
    };

    op.and_then(|op| {
        op.verify_nonce(hrt, tx)?;
        execute_operation_inner(hrt, tx, op, ticketer, injector)
    })
    .map_or_else(
        |e| Receipt::new(op_hash, Err(e)),
        |(hash, content)| Receipt::new(hash, Ok(content)),
    )
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, Method, Uri};
    use jstz_core::{reveal_data::PreimageHash, BinEncodable};
    use jstz_crypto::{
        hash::Hash, public_key::PublicKey, public_key_hash::PublicKeyHash,
        secret_key::SecretKey,
    };
    use operation::RevealType;
    use tezos_crypto_rs::hash::HashTrait;
    use tezos_smart_rollup_mock::MockHost;

    use super::*;
    use crate::{
        context::account::Nonce,
        operation::{Content, DeployFunction, RevealLargePayload, RunFunction},
        receipt::ReceiptResult,
    };

    use crate::runtime::ParsedCode;

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
        let body = vec![0];
        Content::RunFunction(RunFunction {
            uri: Uri::try_from(
                "jstz://tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU/nfts?status=sold",
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
    #[test]
    fn reveals_large_payload_operation() {
        let mut mock_host = MockHost::default();
        let mut host = Host::new(&mut mock_host);
        let mut tx = Transaction::default();
        tx.begin();
        let (_, pk1, sk1) = bootstrap1();
        let (_, pk2, sk2) = bootstrap2();
        let deploy_op = make_signed_op(deploy_function_content(), pk2, sk2);
        let root_hash = make_data_available(&mut mock_host, deploy_op.clone());
        let rdc_op = signed_rdc_op(root_hash, pk1.clone(), sk1, deploy_op.hash());
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();
        let receipt = execute_operation(&mut host, &mut tx, rdc_op, &ticketer, &pk1);
        assert!(matches!(receipt.result, ReceiptResult::Success(_)));
    }
    #[test]
    fn throws_error_if_reveal_type_not_supported() {
        let mut mock_host = MockHost::default();
        let mut host = Host::new(&mut mock_host);
        let mut tx = Transaction::default();
        tx.begin();
        let (_, pk1, sk1) = bootstrap1();
        let (_, pk2, sk2) = bootstrap2();
        let run_op = make_signed_op(run_function_content(), pk2.clone(), sk2.clone());
        let root_hash = make_data_available(&mut mock_host, run_op.clone());
        let rdc_op = signed_rdc_op(root_hash, pk1.clone(), sk1.clone(), run_op.hash());
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();
        let receipt = execute_operation(&mut host, &mut tx, rdc_op, &ticketer, &pk1);
        println!("receipt: {:?}", receipt);
        assert!(matches!(
            receipt.result,
            ReceiptResult::Failed(e) if e.contains("RevealNotSupported")
        ));
    }

    #[test]
    fn throws_if_nonce_is_invalid() {
        let mut mock_host = MockHost::default();
        let mut host = Host::new(&mut mock_host);
        let mut tx = Transaction::default();
        tx.begin();
        let (_, pk, sk) = bootstrap1();
        let deploy_op = make_signed_op(deploy_function_content(), pk.clone(), sk.clone());
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();
        let receipt =
            execute_operation(&mut host, &mut tx, deploy_op.clone(), &ticketer, &pk);
        assert!(matches!(receipt.result, ReceiptResult::Success(_)));
        let receipt = execute_operation(&mut host, &mut tx, deploy_op, &ticketer, &pk);
        assert!(
            matches!(receipt.result, ReceiptResult::Failed(e) if e.contains("InvalidNonce"))
        );
    }

    #[test]
    fn throws_if_injector_is_invalid() {
        let mut mock_host = MockHost::default();
        let mut host = Host::new(&mut mock_host);
        let mut tx = Transaction::default();
        tx.begin();
        let (_, pk1, sk1) = bootstrap1();
        let (_, pk2, sk2) = bootstrap2();
        let deploy_op = make_signed_op(deploy_function_content(), pk1.clone(), sk1);
        let root_hash = make_data_available(&mut mock_host, deploy_op.clone());
        let rdc_op = signed_rdc_op(root_hash, pk2.clone(), sk2, deploy_op.hash());
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();
        let receipt =
            execute_operation(&mut host, &mut tx, rdc_op.clone(), &ticketer, &pk1);
        assert!(
            matches!(receipt.clone().result, ReceiptResult::Failed(e) if e.contains("InvalidInjector"))
        );
        assert_eq!(receipt.hash().to_string(), deploy_op.hash().to_string());
    }

    #[test]
    fn run_function_with_invalid_scheme_fails() {
        let mut mock_host = MockHost::default();
        let mut host = Host::new(&mut mock_host);
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
                body: None,
                gas_limit: 10000,
            }),
            pk1.clone(),
            sk1,
        );
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();

        let receipt = execute_operation(&mut host, &mut tx, run_op, &ticketer, &pk1);

        assert!(
            matches!(receipt.clone().result, ReceiptResult::Failed(e) if e.contains("InvalidScheme"))
        );
    }
}
