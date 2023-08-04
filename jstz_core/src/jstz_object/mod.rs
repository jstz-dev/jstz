use crate::host_ref::HostRef;
use boa_engine::{object::ObjectInitializer, property::Attribute, Context, JsObject};
use jstz_serde::Address;
mod storage;

use tezos_smart_rollup_host::runtime::Runtime;
pub(super) fn make_jstz<Host: Runtime + 'static>(
    context: &mut Context<'_>,
    host: &HostRef<Host>,
    contract: &Address,
) -> JsObject {
    let storage_property = storage::make_storage(context, host, contract.clone());
    ObjectInitializer::new(context)
        .property("durableStorage", storage_property, Attribute::PERMANENT)
        .build()
}
