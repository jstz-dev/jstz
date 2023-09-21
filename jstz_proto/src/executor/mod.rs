use jstz_core::{host::HostRuntime, kv::Transaction};

use crate::{
    context::account::Account,
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
    tx: &mut Transaction,
    run: operation::RunContract,
) -> Result<receipt::RunContract> {
    contract::run::execute(hrt, tx, run)
}

pub fn deploy_contract(
    hrt: &impl HostRuntime,
    tx: &mut Transaction,
    contract: ContractOrigination,
) -> Result<receipt::DeployContract> {
    let nonce = Account::nonce(hrt, tx, &contract.originating_address)?;
    nonce.increment();
    deploy_contract_inner(hrt, tx, contract)
}
// this function does not increment the nonce.
// For an externally deployed contract the nonce used for address creation should
// match the signed nonce of the operation.
fn deploy_contract_inner(
    hrt: &impl HostRuntime,
    tx: &mut Transaction,
    contract: ContractOrigination,
) -> Result<receipt::DeployContract> {
    Ok(receipt::DeployContract {
        contract_address: origination::execute(hrt, tx, contract)?,
    })
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
        } => deploy_contract_inner(
            hrt,
            tx,
            ContractOrigination {
                originating_address: source,
                initial_balance: deployment.contract_credit,
                contract_code: deployment.contract_code,
            },
        )
        .map(receipt::Content::DeployContract),

        Operation {
            content: operation::Content::CallContract(_),
            ..
        } => todo!(),
        Operation {
            content: operation::Content::RunContract(run),
            ..
        } => {
            let result = contract::run::execute(hrt, tx, run.clone())?;

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
