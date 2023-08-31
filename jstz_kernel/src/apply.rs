use boa_engine::{JsResult, JsValue, Source};
use jstz_core::{
    executor::Executor,
    kv::Kv,
    realm::Module,
    runtime::{with_host_runtime, Runtime as JstzRuntime},
};
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

async fn execute(
    module: &Module,
    jstz_runtime: &mut JstzRuntime<'_>,
) -> JsResult<JsValue> {
    let promise = module.realm().eval_module(&module, jstz_runtime)?;

    jstz_runtime
        .resolve_value(&promise.into())
        .await
        .expect("Failed to evaluate top-level script");

    let result = Executor::handle_request(&module, jstz_runtime)?;

    jstz_runtime.resolve_value(&result).await
}

pub fn apply_transaction(rt: &mut (impl Runtime + 'static), tx: Transaction) {
    let Transaction {
        contract_address,
        contract_code,
    } = tx;

    debug_msg!(rt, "Evaluating: {contract_code:?}\n");

    // 1. Initialize runtime
    let mut jstz_runtime = JstzRuntime::new();

    let result: JsResult<JsValue> = with_host_runtime(rt, || {
        // 2. Initialize script
        let module =
            Module::parse(Source::from_bytes(&contract_code), None, &mut jstz_runtime)
                .expect("Failed to parse contract code");

        // 3. Initialize Apis
        module
            .realm()
            .register_api(jstz_api::ConsoleApi, &mut jstz_runtime);
        module.realm().register_api(
            jstz_api::LedgerApi {
                contract_address: contract_address.clone(),
            },
            &mut jstz_runtime,
        );
        module.realm().register_api(
            jstz_api::StorageApi {
                contract_address: contract_address,
            },
            &mut jstz_runtime,
        );
        module
            .realm()
            .register_api(jstz_api::ContractApi, &mut jstz_runtime);

        // 4. Execute
        jstz_core::future::block_on(execute(&module, &mut jstz_runtime))
    });

    debug_msg!(rt, "Result: {result:?}\n");
}
