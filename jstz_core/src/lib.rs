use boa_engine::{context::ContextBuilder, Context, Source};
use getrandom::{register_custom_getrandom, Error};
use std::num::NonZeroU32;
use tezos_smart_rollup_host::runtime::Runtime;

mod error;

pub use error::{Error, Result};
pub mod host;
pub mod kv;

// custom getrandom
const GETRANDOM_ERROR_CODE: u32 = Error::CUSTOM_START + 42;
fn always_fail(_: &mut [u8]) -> Result<(), Error> {
    let code = NonZeroU32::new(GETRANDOM_ERROR_CODE).unwrap();
    Err(Error::from(code))
}

register_custom_getrandom!(always_fail);

pub struct JstzRuntime<'host, H: Runtime + 'static> {
    context: Context<'host>,
    host: host::Host<H>,
}

impl<'a, H: Runtime> JstzRuntime<'a, H> {
    pub fn new(host: host::Host<H>) -> Self {
        let context = ContextBuilder::new()
            .host_hooks(host::HOOKS)
            .build()
            .unwrap();

        Self { context, host }
    }

    pub fn register_global_api<T>(&mut self)
    where
        T: host::Api,
    {
        T::init(&mut self.context, self.host.clone())
    }

    pub fn eval(&mut self, src: impl AsRef<[u8]>) -> String {
        self.context
            .eval(Source::from_bytes(&src))
            .map(|v| v.display().to_string())
            .unwrap_or("Uncaught error".to_string())
    }
}

// JS eval function
pub fn evaluate_from_bytes<H: Runtime>(
    host: host::Host<H>,
    src: impl AsRef<[u8]>,
) -> String {
    JstzRuntime::new(host).eval(src)
}
