use jstz_core::{host::HostRuntime, kv::Transaction};

use crate::{
    operation::{
        self, external::ContractOrigination, ExternalOperation, Operation,
        SignedOperation,
    },
    receipt::{self, Receipt},
    Result,
};

pub mod contract;
pub mod deposit;
pub mod origination;

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

pub fn deploy_contract(
    hrt: &mut (impl HostRuntime + 'static),
    tx: &mut Transaction,
    contract: ContractOrigination,
) -> Result<receipt::Content> {
    let result = origination::execute(hrt, tx, contract)?;
    Ok(receipt::Content::DeployContract(receipt::DeployContract {
        contract_address: result,
    }))
}
fn execute_operation_inner(
    hrt: &mut (impl HostRuntime + 'static),
    tx: &mut Transaction,
    signed_operation: SignedOperation,
) -> Result<receipt::Content> {
    let operation = signed_operation.verify()?;

    operation.verify_nonce(hrt, tx)?;
    match operation {
        Operation {
            source,
            content: operation::Content::DeployContract(deployment),
            ..
        } => deploy_contract(
            hrt,
            tx,
            ContractOrigination {
                originating_address: source,
                initial_balance: deployment.contract_credit,
                contract_code: deployment.contract_code,
            },
        ),

        Operation {
            content: operation::Content::CallContract(_),
            ..
        } => todo!(),
        Operation {
            content: operation::Content::RunContract(run),
            ..
        } => {
            let result = contract::run::execute(hrt, run.clone())?;

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
        ExternalOperation::ContractOrigination(contract) => {
            origination::execute(hrt, tx, contract).map(|_| ())
        }
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
