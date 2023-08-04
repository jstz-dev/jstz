mod console;
mod host_ref;
mod jstz_object;

use boa_engine::{property::Attribute, Context, JsError, JsValue, Source};
use getrandom::{register_custom_getrandom, Error};
use jstz_serde::InboxMessage;
use std::num::NonZeroU32;
use tezos_smart_rollup_host::runtime::Runtime;

use host_ref::HostRef;

// custom getrandom
const GETRANDOM_ERROR_CODE: u32 = Error::CUSTOM_START + 42;
fn always_fail(_: &mut [u8]) -> Result<(), Error> {
    let code = NonZeroU32::new(GETRANDOM_ERROR_CODE).unwrap();
    Err(Error::from(code))
}

register_custom_getrandom!(always_fail);

pub fn make_context<Host: Runtime + 'static>(
    host: &HostRef<Host>,
) -> Result<Context, JsError> {
    let mut context = Context::default();
    let console_property = console::make_console(&mut context, host);
    let jstz_property = jstz_object::make_jstz(&mut context, host, &"my_contract");
    context.register_global_property(
        "console",
        console_property,
        Attribute::PERMANENT,
    )?;
    context.register_global_property("JsTz", jstz_property, Attribute::PERMANENT)?;
    Ok(context)
}

fn evaluate_js<Host: Runtime + 'static>(
    host: HostRef<Host>,
    source: impl AsRef<[u8]>,
) -> Result<JsValue, JsError> {
    make_context(&host)?.eval(Source::from_bytes(&source))
}
// JS eval function
pub fn evaluate_from_bytes(
    host: &mut (impl Runtime + 'static),
    source: impl AsRef<[u8]>,
) -> String {
    #[allow(irrefutable_let_patterns)]
    if let InboxMessage::RunJs { code } = source.as_ref().try_into().unwrap() {
        // HostRef fools rust into thinking it has a static lifetime so unsafe
        // as we return a String (which can't keep the reference alive)
        // and declare host first the lifetime is safe
        let host = unsafe { HostRef::new(host) };
        match evaluate_js(host, code) {
            Ok(js_val) => js_val.display().to_string(),
            Err(err) => format!("Uncaught error: {:?}", err),
        }
    } else {
        "not code".to_string()
    }
}
