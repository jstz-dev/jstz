mod console;
mod kv;

// TODO: onl for draft pull erquest
mod refactored;

pub mod http;
mod text_encoder;
pub mod url;
use boa_engine::Context;
pub use console::ConsoleApi;
pub use kv::KvApi;
pub use text_encoder::TextEncoderApi;

pub struct Api;

use jstz_core::GlobalApi;
impl GlobalApi for Api {
    fn init(context: &mut Context) {
        refactored::Api::init(context);
        // TODO migrate rest of apis
    }
}
