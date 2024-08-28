use jstz_core::{host::HostRuntime, kv::Transaction};
use tezos_crypto_rs::hash::ContractKt1Hash;

use crate::{
    operation::{self, ExternalOperation, Operation, SignedOperation},
    receipt::{self, Receipt},
    Result,
};

pub mod deposit;
pub mod fa_deposit;
pub mod smart_function;
pub mod withdraw;

pub const JSTZ_HOST: &str = "jstz";

fn execute_operation_inner(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    signed_operation: SignedOperation,
    ticketer: &ContractKt1Hash,
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
            let result = match run.uri.host() {
                Some(JSTZ_HOST) => {
                    smart_function::jstz_run::execute(hrt, tx, &source, run, ticketer)?
                }
                _ => smart_function::run::execute(hrt, tx, &source, run, operation_hash)?,
            };
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
    ticketer: &ContractKt1Hash,
) -> Receipt {
    let hash = signed_operation.hash();
    let inner = execute_operation_inner(hrt, tx, signed_operation, ticketer);
    Receipt::new(hash, inner)
}
