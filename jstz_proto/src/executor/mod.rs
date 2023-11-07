use jstz_core::{host::HostRuntime, kv::Transaction};

use crate::{
    context::account::Account,
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

    operation.verify_nonce(hrt, tx)?;
    let Operation {
        source, content, ..
    } = operation;
    let receipt = match content {
        operation::Content::DeployContract(deployment) => {
            let result = contract::deploy::execute(hrt, tx, &source, deployment)?;

            receipt::Content::DeployContract(result)
        }

        operation::Content::RunContract(run) => {
            let result = contract::run::execute(hrt, tx, &source, run)?;

            receipt::Content::RunContract(result)
        }
    };
    Account::nonce(hrt, tx, &source)?.increment();
    Ok(receipt)
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
