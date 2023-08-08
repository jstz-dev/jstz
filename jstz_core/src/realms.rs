use std::collections::HashMap;

use boa_engine::{realm::Realm, Context, JsResult, property::Attribute};
use jstz_serde::{Address, Contract};
use tezos_smart_rollup_host::runtime::Runtime;


use super::{console, jstz_object};
use crate::host::{HostRef, storage::StorageMap};




pub struct Realms {
  data : HashMap<Address,Realm>
}


pub fn make_context<Host: Runtime + 'static>(
    host: &HostRef<Host>,
    address: &Address,
) -> JsResult<Context<'static>> {
    let mut context = Context::default();

    let console_property = console::make_console::<Host>(&mut context, host, &address);
    let jstz_property = jstz_object::make_jstz(&mut context, host, &address);
    context.register_global_property(
        "console",
        console_property,
        Attribute::PERMANENT,
    )?;
    context.register_global_property("JsTz", jstz_property, Attribute::PERMANENT)?;
    Ok(context)
}



pub struct RunningContext<'a, Host> {
    contracts : StorageMap<Host, Contract>,
    realms: HashMap<Address, Realm>,
    address: Address,
    context: Context<'a>
}
