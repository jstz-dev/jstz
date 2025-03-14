use jstz_core::{host::HostRuntime, kv::Transaction, reveal_data::RevealData};
use tezos_crypto_rs::hash::ContractKt1Hash;

use crate::{
    operation::{
        self, ExternalOperation, Operation, OperationHash, RevealType, SignedOperation,
    },
    receipt::{self, Receipt},
    Error, Result,
};

pub mod deposit;
pub mod fa_deposit;
pub mod fa_withdraw;
pub mod smart_function;
pub mod withdraw;
pub const JSTZ_HOST: &str = "jstz";

fn execute_operation_inner(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    signed_operation: SignedOperation,
    ticketer: &ContractKt1Hash,
) -> Result<(OperationHash, receipt::ReceiptContent)> {
    let operation = signed_operation.clone().verify()?;
    let operation_hash = operation.hash();

    operation.verify_nonce(hrt, tx)?;

    match operation {
        Operation {
            source,
            content: operation::Content::DeployFunction(deployment),
            ..
        } => {
            let result = smart_function::deploy::execute(hrt, tx, &source, deployment)?;

            Ok((
                signed_operation.hash(),
                receipt::ReceiptContent::DeployFunction(result),
            ))
        }

        Operation {
            content: operation::Content::RunFunction(run),
            source,
            ..
        } => {
            let result = match run.uri.host() {
                Some(JSTZ_HOST) => {
                    smart_function::jstz_run::execute(hrt, tx, ticketer, &source, run)?
                }
                _ => smart_function::run::execute(hrt, tx, &source, run, operation_hash)?,
            };
            Ok((
                signed_operation.hash(),
                receipt::ReceiptContent::RunFunction(result),
            ))
        }

        Operation {
            content: operation::Content::RevealLargePayloadOperation(reveal),
            ..
        } => {
            let result = RevealData::reveal_and_decode::<_, SignedOperation>(
                hrt,
                &reveal.root_hash,
            )
            .map_err(|s| Error::RevealDataError {
                message: s.to_string(),
            })?;

            match (reveal.reveal_type, result.clone().verify()) {
                (
                    RevealType::DeployFunction,
                    Ok(Operation {
                        source,
                        content: operation::Content::DeployFunction(deployment),
                        ..
                    }),
                ) => {
                    let receipt =
                        smart_function::deploy::execute(hrt, tx, &source, deployment)?;

                    return Ok((
                        result.hash(),
                        receipt::ReceiptContent::DeployFunction(receipt),
                    ));
                }
                (_, _) => {
                    todo!()
                }
            }
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
    // let hash = signed_operation.hash();
    let (hash, inner) =
        execute_operation_inner(hrt, tx, signed_operation, ticketer).unwrap();
    Receipt::new(hash, Ok(inner))
}
