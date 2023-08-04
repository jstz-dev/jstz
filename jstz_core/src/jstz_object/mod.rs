use crate::host_ref::HostRef;
use boa_engine::{object::ObjectInitializer, property::Attribute, Context, JsObject};
mod storage;

use tezos_smart_rollup_host::runtime::Runtime;
pub(super) fn make_jstz<Host: Runtime + 'static>(
    context: &mut Context<'_>,
    host: &HostRef<Host>,
    contract: &impl ToString,
) -> JsObject {
    let storage_property = storage::make_storage(context, host, contract.to_string());
    ObjectInitializer::new(context)
        .property("durableStorage", storage_property, Attribute::PERMANENT)
        .build()
}
