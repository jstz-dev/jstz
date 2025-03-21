mod bin_encodable;
pub mod error;
pub mod future;
pub mod host;
pub mod iterators;
pub mod js_fn;
pub mod kv;
pub mod native;
pub mod realm;
pub mod reveal_data;
pub mod runtime;
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
pub use runtime::Runtime;
