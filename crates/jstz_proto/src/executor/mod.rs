use crate::{
    operation::{self, ExternalOperation, Operation, OperationHash, SignedOperation},
    receipt::{self, Receipt},
    Error, Result,
};
use jstz_core::{host::HostRuntime, kv::Transaction, reveal_data::RevealData};
use tezos_crypto_rs::hash::ContractKt1Hash;
pub mod deposit;
pub mod fa_deposit;
pub mod fa_withdraw;
pub mod smart_function;
pub mod withdraw;
pub const JSTZ_HOST: &str = "jstz";

fn verify_signed_op(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    signed_op: SignedOperation,
) -> Result<Operation> {
    signed_op.verify().and_then(|op| {
        op.verify_nonce(hrt, tx)?;
        Ok(op)
    })
}

fn execute_operation_inner(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    op: Operation,
    ticketer: &ContractKt1Hash,
) -> Result<(OperationHash, receipt::ReceiptContent)> {
    let op_hash = op.hash();
    let source = op.source();

    match op.content {
        operation::Content::DeployFunction(deployment) => {
            let result = smart_function::deploy::execute(hrt, tx, &source, deployment)?;
            Ok((op_hash, receipt::ReceiptContent::DeployFunction(result)))
        }
        operation::Content::RunFunction(run) => {
            let result = match run.uri.host() {
                Some(JSTZ_HOST) => {
                    smart_function::jstz_run::execute(hrt, tx, ticketer, &source, run)?
                }
                _ => {
                    smart_function::run::execute(hrt, tx, &source, run, op_hash.clone())?
                }
            };
            Ok((op_hash, receipt::ReceiptContent::RunFunction(result)))
        }
        operation::Content::RevealLargePayload(reveal) => {
            let revealed_op = RevealData::reveal_and_decode::<_, SignedOperation>(
                hrt,
                &reveal.root_hash,
            )?;
            let revealed_op = verify_signed_op(hrt, tx, revealed_op)?;
            if reveal.reveal_type == revealed_op.content().try_into()? {
                return execute_operation_inner(hrt, tx, revealed_op, ticketer);
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
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    signed_operation: SignedOperation,
    ticketer: &ContractKt1Hash,
) -> Receipt {
    let operation_hash = signed_operation.hash();
    let result = verify_signed_op(hrt, tx, signed_operation)
        .and_then(|op| execute_operation_inner(hrt, tx, op, ticketer));
    match result {
        Ok((hash, content)) => Receipt::new(hash, Ok(content)),
        Err(e) => Receipt::new(operation_hash, Err(e)),
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
    use operation::RevealType;
    use tezos_crypto_rs::hash::HashTrait;
    use tezos_smart_rollup_mock::MockHost;

    use super::*;
    use crate::{
        context::account::{Nonce, ParsedCode},
        operation::{Content, DeployFunction, RevealLargePayload, RunFunction},
        receipt::ReceiptResult,
    };

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

    fn make_signed_op(content: Content) -> SignedOperation {
        let (_, pk, sk) = bootstrap2();
        let deploy_op = Operation {
            public_key: pk,
            nonce: Nonce(0),
            content,
        };
        let sig = sk.sign(deploy_op.hash()).unwrap();
        SignedOperation::new(sig, deploy_op)
    }

    fn signed_rdc_op(root_hash: PreimageHash) -> SignedOperation {
        let rdc_op = RevealLargePayload {
            root_hash,
            reveal_type: RevealType::DeployFunction,
        };
        let rdc_op_content = rdc_op;
        let (_, pk, sk) = bootstrap1();
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
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();
        let deploy_op = make_signed_op(deploy_function_content());
        let root_hash = make_data_available(&mut host, deploy_op);
        let rdc_op = signed_rdc_op(root_hash);
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();
        let receipt = execute_operation(&mut host, &mut tx, rdc_op, &ticketer);
        assert!(matches!(receipt.result, ReceiptResult::Success(_)));
    }
    #[test]
    fn throws_error_if_reveal_type_not_supported() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();
        let run_op = make_signed_op(run_function_content());
        let root_hash = make_data_available(&mut host, run_op);
        let rdc_op = signed_rdc_op(root_hash);
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();
        let receipt = execute_operation(&mut host, &mut tx, rdc_op, &ticketer);
        assert!(matches!(
            receipt.result,
            ReceiptResult::Failed(e) if e.contains("RevealNotSupported")
        ));
    }

    #[test]
    fn throws_for_invalid_nonce() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();
        let deploy_op = make_signed_op(deploy_function_content());
        let ticketer = ContractKt1Hash::try_from_bytes(&[0; 20]).unwrap();
        let receipt = execute_operation(&mut host, &mut tx, deploy_op.clone(), &ticketer);
        assert!(matches!(receipt.result, ReceiptResult::Success(_)));
        let receipt = execute_operation(&mut host, &mut tx, deploy_op, &ticketer);
        assert!(
            matches!(receipt.result, ReceiptResult::Failed(e) if e.contains("InvalidNonce"))
        );
    }
}
