pub mod error;

use bincode::config::{Configuration, Fixint, LittleEndian};
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
    fn init(self, context: &mut Context);
}

pub use realm::{Module, Realm};
pub use runtime::Runtime;

pub use crate::kv::value::{deserialize, serialize};

pub static BINCODE_CONFIGURATION: Configuration<LittleEndian, Fixint> =
    bincode::config::legacy();
