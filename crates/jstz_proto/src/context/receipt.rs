use jstz_core::{host::HostRuntime, kv::Transaction};
use tezos_smart_rollup::storage::path::{self, OwnedPath, RefPath};

use crate::{receipt::Receipt, Result};

const RECEIPTS_PATH: RefPath = RefPath::assert_from(b"/jstz_receipt");

impl Receipt {
    pub fn write(self, _hrt: &impl HostRuntime, tx: &mut Transaction) -> Result<()> {
        let receipt_path = OwnedPath::try_from(format!("/{}", self.hash()))?;
        Ok(tx.insert(path::concat(&RECEIPTS_PATH, &receipt_path)?, self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipt::{DeployFunctionReceipt, Receipt, ReceiptContent, ReceiptResult};
    use jstz_core::kv::Transaction;
    use jstz_crypto::{
        hash::Blake2b,
        smart_function_hash::{Kt1Hash, SmartFunctionHash},
    };
    use tezos_crypto_rs::hash::ContractKt1Hash;
    use tezos_smart_rollup_mock::MockHost;

    #[test]
    fn test_write_receipt_inserts_into_transaction() {
        let host = MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();
        let receipt = Receipt::new(
            Blake2b::default(),
            Ok(ReceiptContent::DeployFunction(DeployFunctionReceipt {
                address: SmartFunctionHash(Kt1Hash(
                    ContractKt1Hash::from_base58_check(
                        "KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ",
                    )
                    .unwrap(),
                )),
            })),
        );
        receipt.clone().write(&host, &mut tx).unwrap();
        let hash = receipt.hash();
        let p = &OwnedPath::try_from(format!("/{}", hash)).unwrap();
        let path = path::concat(&RECEIPTS_PATH, p).unwrap();
        let stored = tx.get::<Receipt>(&host, path).unwrap();
        assert!(matches!(stored.unwrap().result, ReceiptResult::Success(_)));
    }
}
