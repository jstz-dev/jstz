use boa_engine::{Context, Source};
use getrandom::{register_custom_getrandom, Error};
use std::num::NonZeroU32;
pub mod console;
mod host;

// custom getrandom
const GETRANDOM_ERROR_CODE: u32 = Error::CUSTOM_START + 42;
fn always_fail(_: &mut [u8]) -> Result<(), Error> {
    let code = NonZeroU32::new(GETRANDOM_ERROR_CODE).unwrap();
    Err(Error::from(code))
}

register_custom_getrandom!(always_fail);

// JS eval function
pub fn evaluate_from_bytes(src: impl AsRef<[u8]>) -> String {
    // Setup the executor
    Context::default()
        .eval(Source::from_bytes(&src))
        .map(|v| v.display().to_string())
        .unwrap_or("Uncaught error".to_string())
}
