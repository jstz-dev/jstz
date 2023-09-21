use jstz_core::kv::Kv;
use jstz_proto::{
    operation::{external::Deposit, ExternalOperation, RunContract},
    Result,
};
use tezos_smart_rollup::prelude::{debug_msg, Runtime};

use crate::inbox::ContractOrigination;

pub fn apply_deposit(rt: &mut impl Runtime, deposit: Deposit) -> Result<()> {
    let mut kv = Kv::new();
    let mut tx = kv.begin_transaction();

    jstz_proto::executor::execute_external_operation(
        rt,
        &mut tx,
        ExternalOperation::Deposit(deposit),
    )
    .expect("Failed to execute deposit");

    kv.commit_transaction(rt, tx)?;
    Ok(())
}

pub fn apply_run_contract(
    rt: &mut (impl Runtime + 'static),
    run: RunContract,
) -> Result<()> {
    let mut kv = Kv::new();
    let mut tx = kv.begin_transaction();

    let result = jstz_proto::executor::run_contract(rt, &mut tx, run);

    kv.commit_transaction(rt, tx)?;

    debug_msg!(rt, "Result: {result:?}\n");
    Ok(())
}
// TODO ⚰️ Deprecate will not be part of the CLI
pub fn apply_transaction(
    rt: &mut (impl Runtime + 'static),
    tx: crate::inbox::Transaction,
) -> Result<()> {
    use http::{HeaderMap, Method};
    use jstz_api::http::body::HttpBody;

    let crate::inbox::Transaction {
        contract_address,
        contract_code,
    } = tx;

    let mut kv = Kv::new();
    let mut tx = kv.begin_transaction();
    let uri = format!("jstz://{}", contract_address.to_base58())
        .parse()
        .expect("");

    let result = jstz_proto::executor::run_contract(
        rt,
        &mut tx,
        RunContract {
            contract_code,
            headers: HeaderMap::default(),
            method: Method::default(),
            body: HttpBody::default(),
            uri,
        },
    )?;

    kv.commit_transaction(rt, tx)?;

    debug_msg!(rt, "Result: {result:?}\n");
    Ok(())
}
pub fn apply_deploy_contract(
    rt: &mut (impl Runtime + 'static),
    origination: ContractOrigination,
) -> Result<()> {
    let mut kv = Kv::new();
    let mut tx = kv.begin_transaction();
    jstz_proto::executor::deploy_contract(rt, &mut tx, origination)?;
    kv.commit_transaction(rt, tx)?;
    Ok(())
}
