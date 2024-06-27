use jstz_core::{host::HostRuntime, kv::Transaction};
use tezos_smart_rollup::storage::path::{self, OwnedPath, RefPath};

use crate::{receipt::Receipt, Result};

const RECEIPTS_PATH: RefPath = RefPath::assert_from(b"/jstz_receipt");

impl Receipt {
    pub fn write(self, _hrt: &impl HostRuntime, tx: &mut Transaction) -> Result<()> {
        let receipt_path = OwnedPath::try_from(format!("/{}", self.hash().to_string()))?;

        Ok(tx.insert(path::concat(&RECEIPTS_PATH, &receipt_path)?, self)?)
    }
}
