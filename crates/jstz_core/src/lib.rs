mod error;

use boa_engine::Context;

pub use error::{Error, Result};
pub mod future;
pub mod host;
pub mod iterators;
pub mod js_fn;
pub mod kv;
pub mod native;
pub mod realm;
pub mod runtime;
pub mod value;

/// A generic runtime API
pub trait Api {
    /// Initialize a runtime API
    fn init(self, context: &mut Context<'_>);
}

pub use realm::{Module, Realm};
pub use runtime::Runtime;
