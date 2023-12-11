mod bridge;
pub mod rollup;

use serde::{Deserialize, Serialize};

pub use bridge::*;
pub use rollup::JstzRollup;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapAccount {
    pub address: String,
    pub amount: u64,
}
