use jstz_core::kv::Kv;
use jstz_proto::{
    operation::{external, ExternalOperation, RunContract},
    Result,
};
use tezos_smart_rollup::prelude::{debug_msg, Runtime};

use crate::inbox::{ContractOrigination, Deposit, Transaction};

pub fn apply_deposit(rt: &mut impl Runtime, deposit: Deposit) -> Result<()> {
    let Deposit { amount, reciever } = deposit;

    let mut kv = Kv::new();
    let mut tx = kv.begin_transaction();

    jstz_proto::executor::execute_external_operation(
        rt,
        &mut tx,
        ExternalOperation::Deposit(external::Deposit { amount, reciever }),
    )?;

    kv.commit_transaction(rt, tx)?;
    Ok(())
}

pub fn apply_transaction(
    rt: &mut (impl Runtime + 'static),
    tx: Transaction,
) -> Result<()> {
    let Transaction {
        contract_address,
        contract_code,
    } = tx;

    let mut kv = Kv::new();
    let mut tx = kv.begin_transaction();

    let result = jstz_proto::executor::run_contract(
        rt,
        &mut tx,
        RunContract {
            contract_address,
            contract_code,
        },
    )?;

    kv.commit_transaction(rt, tx)?;

    debug_msg!(rt, "Result: {result:?}\n");
    Ok(())
}
pub fn apply_deploy_contract(
    rt: &mut (impl Runtime + 'static),
    origination: ContractOrigination,
) {
    let mut kv = Kv::new();
    let mut tx = kv.begin_transaction();
    let _ = jstz_proto::executor::deploy_contract(rt, &mut tx, origination);
    kv.commit_transaction(rt, tx)
        .expect("Failed to commit transaction");
}
