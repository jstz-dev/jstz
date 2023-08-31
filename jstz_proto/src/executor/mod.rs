use jstz_core::{host::HostRuntime, kv::Transaction};

use crate::{
    operation::{self, ExternalOperation, SignedOperation},
    receipt::{self, Receipt},
    Result,
};

pub mod contract;
pub mod deposit;

pub fn run_contract(
    hrt: &mut (impl HostRuntime + 'static),
    _tx: &mut Transaction,
    run: operation::RunContract,
) -> Result<receipt::Content> {
    let result = contract::run::execute(hrt, run)?;

    Ok(receipt::Content::RunContract(receipt::RunContract {
        result,
    }))
}

fn execute_operation_inner(
    hrt: &mut (impl HostRuntime + 'static),
    tx: &mut Transaction,
    signed_operation: SignedOperation,
) -> Result<receipt::Content> {
    let operation = signed_operation.verify()?;

    operation.verify_nonce(hrt, tx)?;

    match operation.content {
        operation::Content::DeployContract(_) => todo!(),
        operation::Content::CallContract(_) => todo!(),
        operation::Content::RunContract(run) => {
            let result = contract::run::execute(hrt, run)?;

            Ok(receipt::Content::RunContract(receipt::RunContract {
                result,
            }))
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
