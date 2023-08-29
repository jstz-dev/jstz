use jstz_core::{kv::Kv, JstzRuntime};
use jstz_ledger::account::Account;
use tezos_smart_rollup::prelude::{debug_msg, Runtime};

use crate::inbox::{Deposit, Transaction};

pub fn apply_deposit(rt: &mut impl Runtime, deposit: Deposit) {
    let Deposit { amount, reciever } = deposit;

    let mut kv = Kv::new();
    let mut tx = kv.begin_transaction();

    Account::deposit(rt, &mut tx, &reciever, amount).expect("Failed to deposit");

    kv.commit_transaction(rt, tx)
        .expect("Failed to commit transaction for deposit");
}

pub fn apply_transaction(rt: &mut (impl Runtime + 'static), tx: Transaction) {
    let Transaction {
        contract_address,
        contract_code,
    } = tx;

    debug_msg!(rt, "Evaluating: {contract_code:?}\n");

    // Initialize runtime
    let mut jstz_runtime = JstzRuntime::new(rt);
    jstz_runtime.register_global_api(jstz_api::ConsoleApi);
    jstz_runtime.register_global_api(jstz_api::LedgerApi {
        contract_address: contract_address.clone(),
    });
    jstz_runtime.register_global_api(jstz_api::StorageApi { contract_address });

    // Eval
    let res = jstz_runtime.eval(contract_code);
    debug_msg!(rt, "Result: {res:?}\n");
}
