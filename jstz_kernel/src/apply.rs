use jstz_core::kv::Kv;
use jstz_proto::{
    operation::{external::Deposit, ExternalOperation, RunContract},
    Error, Result,
};
use tezos_smart_rollup::prelude::{debug_msg, Runtime};

use crate::inbox::{ContractOrigination, Transaction};

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

// TODO: Reintroduce once apply_transaction is deprecated
pub fn apply_run_contract(
    _rt: &mut (impl Runtime + 'static),
    _run: RunContract,
) -> Result<()> {
    // let mut kv = Kv::new();
    // let mut tx = kv.begin_transaction();

    // let result = jstz_proto::executor::run_contract(rt, &mut tx, run);

    // kv.commit_transaction(rt, tx)?;

    // debug_msg!(rt, "Result: {result:?}\n");
    // Ok(())
    todo!("Reintroduce once apply_transaction is deprecated")
}

// TODO: Deprecate will not be part of the CLI
pub fn apply_transaction(
    rt: &mut (impl Runtime + 'static),
    tx: Transaction,
) -> Result<()> {
    use http::{HeaderMap, Method};
    use jstz_api::http::body::HttpBody;

    let Transaction { referer, url } = tx;

    let mut kv = Kv::new();
    let mut tx = kv.begin_transaction();
    let uri = url.parse().map_err(|_| Error::InvalidAddress)?;

    let result = jstz_proto::executor::run_contract(
        rt,
        &mut tx,
        &referer,
        RunContract {
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
