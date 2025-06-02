use anyhow::{anyhow, bail, Context};
use jstz_core::kv::{Storage, Transaction};
use jstz_crypto::{
    hash::Hash, public_key::PublicKey, smart_function_hash::SmartFunctionHash,
};
use jstz_proto::{executor::execute_operation, operation::SignedOperation};
use tezos_smart_rollup::{
    prelude::{debug_msg, Runtime},
    storage::path::RefPath,
};

use super::db::Db;

const TICKETER: RefPath = RefPath::assert_from(b"/ticketer");
const INJECTOR: RefPath = RefPath::assert_from(b"/injector");

pub fn init_host(db: Db) -> anyhow::Result<impl Runtime> {
    let mut host = crate::sequencer::host::Host::new(db);
    let ticketer = SmartFunctionHash::from_base58("KT1HbQepzV1nVGg8QVznG7z4RcHseD5kwqBn")
        .context("failed to parse ticketer address")?;

    host.store_write_all(
        &TICKETER,
        &bincode::encode_to_vec(&ticketer, bincode::config::legacy())
            .context("failed to encode ticketer")?,
    )
    .context("failed to write ticketer to host store")?;

    let injector =
        PublicKey::from_base58("edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav")
            .context("failed to parse injector public key")?;

    host.store_write_all(
        &INJECTOR,
        &bincode::encode_to_vec(&injector, bincode::config::legacy())
            .context("failed to encode injector")?,
    )
    .context("failed to write injector to host store")?;

    Ok(host)
}

fn read_ticketer(rt: &impl Runtime) -> Option<SmartFunctionHash> {
    Storage::get(rt, &TICKETER).ok()?
}

fn read_injector(rt: &impl Runtime) -> Option<PublicKey> {
    Storage::get(rt, &INJECTOR).ok()?
}

pub fn process_message(rt: &mut impl Runtime, op: SignedOperation) -> anyhow::Result<()> {
    let ticketer = read_ticketer(rt).ok_or(anyhow!("Ticketer not found"))?;
    let injector = read_injector(rt).ok_or(anyhow!("Revealer not found"))?;
    let mut tx = Transaction::default();
    tx.begin();
    let receipt = execute_operation(rt, &mut tx, op, &ticketer, &injector);
    receipt
        .write(rt, &mut tx)
        .map_err(|e| anyhow!("failed to write receipt: {e}"))?;

    if let Err(commit_error) = tx.commit(rt) {
        let msg = format!("Failed to commit transaction: {commit_error:?}");
        debug_msg!(rt, "{msg}\n");
        bail!(msg)
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, Method, StatusCode, Uri};
    use jstz_core::{host::HostRuntime, BinEncodable};
    use jstz_crypto::{
        hash::Hash,
        public_key::PublicKey,
        signature::Signature,
        smart_function_hash::{Kt1Hash, SmartFunctionHash},
    };
    use jstz_proto::{
        context::account::{Account, Nonce, UserAccount},
        operation::{Content, DeployFunction, Operation, RunFunction, SignedOperation},
        receipt::{
            DeployFunctionReceipt, Receipt, ReceiptContent, ReceiptResult,
            RunFunctionReceipt,
        },
        runtime::ParsedCode,
    };
    use tempfile::NamedTempFile;
    use tezos_crypto_rs::hash::{Ed25519Signature, PublicKeyEd25519};
    use tezos_smart_rollup::storage::path::RefPath;

    use crate::sequencer::db::Db;

    fn dummy_op(sig_str: &str, nonce: u64, content: Content) -> SignedOperation {
        SignedOperation::new(
            Signature::Ed25519(
                Ed25519Signature::from_base58_check(sig_str).unwrap().into(),
            ),
            Operation {
                public_key: PublicKey::Ed25519(
                    PublicKeyEd25519::from_base58_check(
                        "edpkuXD2CqRpWoTT8p4exrMPQYR2NqsYH3jTMeJMijHdgQqkMkzvnz",
                    )
                    .unwrap()
                    .into(),
                ),
                nonce: Nonce(nonce),
                content,
            },
        )
    }

    #[test]
    fn init_host() {
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let rt = super::init_host(db).unwrap();
        assert_eq!(
            super::read_ticketer(&rt).unwrap(),
            SmartFunctionHash::from_base58("KT1HbQepzV1nVGg8QVznG7z4RcHseD5kwqBn")
                .unwrap()
        );
        assert_eq!(
            super::read_injector(&rt).expect("Revealer not found"),
            PublicKey::from_base58(
                "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav"
            )
            .unwrap()
        );
    }

    #[test]
    fn process_message() {
        // Using a slightly complicated scenario here to check if transaction works properly.
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let mut h = super::init_host(db).unwrap();

        // This smart function has about 8k characters. The runtime is okay with it and simply
        // stores it in the data store, though this would not work with a rollup.
        let deploy_op = dummy_op("edsigtk8TSJBNphqsn9uD2tgMbNj9YhsgCMrJmhQQd6EKo8X6viVpZqinN7MnEjwh9EcYTV8NKr3LVEoSfjsTat58mxeemYnyTg", 0, Content::DeployFunction(DeployFunction {function_code: ParsedCode::try_from(format!("const handler = async () => {{ const s = \"{}\"; const myHeaders = new Headers();  myHeaders.append(\"X-JSTZ-TRANSFER\", \"1\"); return await fetch(new Request(\"jstz://tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx/\", {{ headers: myHeaders }})); }}; export default handler;", "a".repeat(8000))).unwrap(), account_credit: 1}));

        let call_op = dummy_op("edsigtkEeUaGxH983imqMYvWZpm24pkoMK7cnrmNWFnCefEWLfVwzjZYnvFauBh8cfww9f2UN67kEve8NDUpQ1D9u9QWsUnXaAh", 1, Content::RunFunction(RunFunction { uri: Uri::from_static("jstz://KT1CDAkLMEHKNs2VbVZeSdxYx3wWN5auGARR/"), method: Method::GET, headers: HeaderMap::new(), body: None, gas_limit: 550000 }));

        let dst_account_path =
            RefPath::assert_from(b"/jstz_account/tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx");

        // The destination account should not exist yet
        assert!(h.store_has(&dst_account_path).unwrap().is_none());

        // Initialise account that deploys the function
        h.store_write_all(
            &RefPath::assert_from(b"/jstz_account/tz1S9rEt3fkYReDdqMPrcGHarAFnaeGBqDeK"),
            &Account::User(UserAccount {
                amount: 1000000,
                nonce: Nonce(0),
            })
            .encode()
            .unwrap(),
        )
        .unwrap();

        // Deploy smart function
        super::process_message(&mut h, deploy_op).unwrap();
        let v = Receipt::decode(&h.store_read_all(&RefPath::assert_from(b"/jstz_receipt/843a8438af97d97e134ae10bdcf10b5a6bcbf8c7d4912e65bacf1be26a5a73c3")).unwrap()).unwrap();
        assert!(matches!(
            v.result,
            ReceiptResult::Success(ReceiptContent::DeployFunction(
                DeployFunctionReceipt { address: SmartFunctionHash(Kt1Hash(addr)) }
            )) if addr.to_base58_check() == "KT1CDAkLMEHKNs2VbVZeSdxYx3wWN5auGARR"
        ));

        // Call smart function
        super::process_message(&mut h, call_op).unwrap();
        let (v, _) = bincode::decode_from_slice::<Receipt, _>(&h.store_read_all(&RefPath::assert_from(b"/jstz_receipt/9b15976cc8162fe39458739de340a1a95c59a9bcff73bd3c83402fad6352396e")).unwrap(), bincode::config::legacy()).unwrap();
        assert!(matches!(
            v.result,
            ReceiptResult::Success(ReceiptContent::RunFunction(RunFunctionReceipt {
                body: _,
                status_code: StatusCode::OK,
                headers: _
            }))
        ));

        // Check if transfer is performed by the smart function
        let account =
            Account::decode(&h.store_read_all(&dst_account_path).unwrap()).unwrap();
        assert!(matches!(
            account,
            Account::User(UserAccount {
                amount: 1,
                nonce: Nonce(0),
            })
        ));
    }
}
