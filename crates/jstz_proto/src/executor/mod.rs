use jstz_core::{host::HostRuntime, kv::Transaction};

use crate::{
    operation::{self, ExternalOperation, Operation, SignedOperation},
    receipt::{self, Receipt},
    Result,
};

pub mod contract;
pub mod deposit;

fn execute_operation_inner(
    hrt: &mut (impl HostRuntime + 'static),
    tx: &mut Transaction,
    signed_operation: SignedOperation,
) -> Result<receipt::Content> {
    let operation = signed_operation.verify()?;
    let operation_hash = operation.hash();

    operation.verify_nonce(hrt, tx)?;
    match operation {
        Operation {
            source,
            content: operation::Content::DeployContract(deployment),
            ..
        } => {
            let result = contract::deploy::execute(hrt, tx, &source, deployment)?;

            Ok(receipt::Content::DeployContract(result))
        }

        Operation {
            content: operation::Content::RunContract(run),
            source,
            ..
        } => {
            let result = contract::run::execute(hrt, tx, &source, run, &operation_hash)?;

            Ok(receipt::Content::RunContract(result))
        }
    }
}

pub fn execute_external_operation(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    external_operation: ExternalOperation,
) -> Result<()> {
    match external_operation {
        ExternalOperation::Deposit(deposit) => deposit::execute(hrt, tx, deposit),
    }
}

pub fn execute_operation(
    hrt: &mut (impl HostRuntime + 'static),
    tx: &mut Transaction,
    signed_operation: SignedOperation,
) -> Receipt {
    let hash = signed_operation.hash();
    let inner = execute_operation_inner(hrt, tx, signed_operation);
    Receipt::new(hash, inner)
}
