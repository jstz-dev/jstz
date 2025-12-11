mod bin_encodable;
pub mod error;
pub mod event;
pub mod future;
pub mod host;
pub mod iterators;
pub mod js_fn;
pub mod kv;
pub mod log_record;
pub mod native;
pub mod realm;
pub mod reveal_data;
mod revealer;
pub mod runtime;
#[cfg(feature = "simulation")]
pub mod simulation;
pub mod value;
pub use bin_encodable::*;
use boa_engine::Context;
pub use error::{Error, Result};

/// A generic runtime API
pub trait Api {
    /// Initialize a runtime API
    fn init(self, context: &mut Context);
}

pub use realm::{Module, Realm};
pub use revealer::Revealer;
pub use runtime::Runtime;
