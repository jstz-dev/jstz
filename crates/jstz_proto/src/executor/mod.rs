use jstz_core::{host::HostRuntime, kv::Transaction};

use crate::{
    operation::{self, ExternalOperation, Operation, SignedOperation},
    receipt::{self, Receipt},
    Result,
};

pub mod deposit;
pub mod fa_deposit;
pub mod smart_function;

fn execute_operation_inner(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    signed_operation: SignedOperation,
) -> Result<receipt::Content> {
    let operation = signed_operation.verify()?;
    let operation_hash = operation.hash();

    operation.verify_nonce(hrt, tx)?;

    match operation {
        Operation {
            source,
            content: operation::Content::DeployFunction(deployment),
            ..
        } => {
            let result = smart_function::deploy::execute(hrt, tx, &source, deployment)?;

            Ok(receipt::Content::DeployFunction(result))
        }

        Operation {
            content: operation::Content::RunFunction(run),
            source,
            ..
        } => {
            let result =
                smart_function::run::execute(hrt, tx, &source, run, operation_hash)?;

            Ok(receipt::Content::RunFunction(result))
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
) -> Receipt {
    let hash = signed_operation.hash();
    let inner = execute_operation_inner(hrt, tx, signed_operation);
    Receipt::new(hash, inner)
}
