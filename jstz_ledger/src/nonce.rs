use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Nonce(u64);

impl Default for Nonce {
    fn default() -> Self {
        Self(0)
    }
}

impl Nonce {
    pub fn increment(&mut self) {
        self.0 += 1
    }
}

impl ToString for Nonce {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}
