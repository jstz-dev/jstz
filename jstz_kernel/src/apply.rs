use jstz_core::kv::Kv;
use jstz_proto::operation::{external::Deposit, ExternalOperation, RunContract};
use tezos_smart_rollup::prelude::{debug_msg, Runtime};

pub fn apply_deposit(rt: &mut impl Runtime, deposit: Deposit) {
    let mut kv = Kv::new();
    let mut tx = kv.begin_transaction();

    jstz_proto::executor::execute_external_operation(
        rt,
        &mut tx,
        ExternalOperation::Deposit(deposit),
    )
    .expect("Failed to execute deposit");

    kv.commit_transaction(rt, tx)
        .expect("Failed to commit transaction for deposit");
}

pub fn apply_run_contract(rt: &mut (impl Runtime + 'static), run: RunContract) {
    let mut kv = Kv::new();
    let mut tx = kv.begin_transaction();

    let result = jstz_proto::executor::run_contract(rt, &mut tx, run);

    kv.commit_transaction(rt, tx)
        .expect("Failed to commit transaction");

    debug_msg!(rt, "Result: {result:?}\n");
}
