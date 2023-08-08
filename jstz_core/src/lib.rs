mod console;
mod host;
mod jstz_object;

use boa_engine::{property::Attribute, Context, JsError, JsResult, JsValue, Source};
use getrandom::{register_custom_getrandom, Error};
use jstz_serde::{Address, ByteRep, InboxMessage};
use std::num::NonZeroU32;
use tezos_smart_rollup_host::runtime::Runtime;

use host::HostRef;

// custom getrandom
const GETRANDOM_ERROR_CODE: u32 = Error::CUSTOM_START + 42;
fn always_fail(_: &mut [u8]) -> Result<(), Error> {
    let code = NonZeroU32::new(GETRANDOM_ERROR_CODE).unwrap();
    Err(Error::from(code))
}

register_custom_getrandom!(always_fail);

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

fn evaluate_js<Host: Runtime + 'static>(
    host: HostRef<Host>,
    source: impl AsRef<[u8]>,
) -> Result<JsValue, JsError> {
    let address: Address = "JsTz42BADA55".try_into().unwrap();
    make_context(&host, &address)?.eval(Source::from_bytes(&source))
}

// JS eval function
pub fn evaluate_from_bytes(
    host: &mut (impl Runtime + 'static),
    source: impl AsRef<[u8]>,
) -> String {
    let bytes =
        ByteRep::<InboxMessage>::new(source.as_ref().iter().map(Clone::clone).collect());
    #[allow(irrefutable_let_patterns)]
    if let Ok(InboxMessage::RunJs { code }) = bytes.into_t() {
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
