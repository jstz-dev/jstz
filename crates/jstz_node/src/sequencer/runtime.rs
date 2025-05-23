use jstz_core::kv::{Storage, Transaction};
use jstz_crypto::{
    hash::Hash, public_key::PublicKey, smart_function_hash::SmartFunctionHash,
};
use jstz_proto::{executor::execute_operation, operation::SignedOperation};
use tezos_smart_rollup::prelude::{debug_msg, Runtime};
use tezos_smart_rollup_host::path::RefPath;

use super::db::Db;

const TICKETER: RefPath = RefPath::assert_from(b"/ticketer");
const INJECTOR: RefPath = RefPath::assert_from(b"/injector");

pub fn init_host(db: Db) -> impl Runtime {
    let mut host = crate::sequencer::host::Host::new(db);
    let ticketer =
        SmartFunctionHash::from_base58("KT1HbQepzV1nVGg8QVznG7z4RcHseD5kwqBn").unwrap();

    host.store_write(
        &TICKETER,
        &bincode::encode_to_vec(&ticketer, bincode::config::legacy()).unwrap(),
        0,
    )
    .unwrap();

    let injector =
        PublicKey::from_base58("edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav")
            .unwrap();

    host.store_write(
        &INJECTOR,
        &bincode::encode_to_vec(&injector, bincode::config::legacy()).unwrap(),
        0,
    )
    .unwrap();

    host
}

fn read_ticketer(rt: &impl Runtime) -> Option<SmartFunctionHash> {
    Storage::get(rt, &TICKETER).ok()?
}

fn read_injector(rt: &impl Runtime) -> Option<PublicKey> {
    Storage::get(rt, &INJECTOR).ok()?
}

pub fn process_message(rt: &mut impl Runtime, op: SignedOperation) -> anyhow::Result<()> {
    let ticketer = read_ticketer(rt).ok_or(anyhow::anyhow!("Ticketer not found"))?;
    let injector = read_injector(rt).ok_or(anyhow::anyhow!("Revealer not found"))?;
    let mut tx = Transaction::default();
    tx.begin();
    let receipt = execute_operation(rt, &mut tx, op, &ticketer, &injector);
    receipt
        .write(rt, &mut tx)
        .map_err(|e| anyhow::anyhow!("failed to write receipt: {e}"))?;

    if let Err(commit_error) = tx.commit(rt) {
        let msg = format!("Failed to commit transaction: {commit_error:?}");
        debug_msg!(rt, "{msg}\n");
        anyhow::bail!(msg)
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, Method, StatusCode, Uri};
    use jstz_core::host::HostRuntime;
    use jstz_core::BinEncodable;
    use jstz_crypto::hash::Hash;
    use jstz_crypto::smart_function_hash::{Kt1Hash, SmartFunctionHash};
    use jstz_crypto::{public_key::PublicKey, signature::Signature};
    use jstz_proto::context::account::{Account, ParsedCode, UserAccount};
    use jstz_proto::operation::DeployFunction;
    use jstz_proto::receipt::{
        DeployFunctionReceipt, Receipt, ReceiptContent, ReceiptResult, RunFunctionReceipt,
    };
    use jstz_proto::{
        context::account::Nonce,
        operation::{Content, Operation, RunFunction, SignedOperation},
    };
    use tempfile::NamedTempFile;
    use tezos_crypto_rs::hash::{Ed25519Signature, PublicKeyEd25519};
    use tezos_smart_rollup_host::path::RefPath;

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
        let mut rt = super::init_host(db);
        assert_eq!(
            super::read_ticketer(&mut rt).unwrap(),
            SmartFunctionHash::from_base58("KT1HbQepzV1nVGg8QVznG7z4RcHseD5kwqBn")
                .unwrap()
        );
        assert_eq!(
            super::read_injector(&mut rt).expect("Revealer not found"),
            PublicKey::from_base58(
                "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav"
            )
            .unwrap()
        );
    }

    #[test]
    fn process_message() {
        // using a slightly complicated scenario here to check if transaction works properly
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let mut h = super::init_host(db);

        let deploy_op = dummy_op("edsigtuqETA6KDxKzW6gYkZpefJd5FfhX4DbVJCryoeDGLdF1LosZKnrfBUUKQzAsQNdLXk9sufDb6PRPP3rKWJvVqDoBAPVuY2", 0, Content::DeployFunction(DeployFunction {function_code: ParsedCode::try_from("const handler = async () => {\n  const myHeaders = new Headers();\n  myHeaders.append(\"X-JSTZ-TRANSFER\", \"1\");\n  return await fetch(\n    new Request(\"jstz://tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx/\", {\n      headers: myHeaders,\n    }),\n  );\n};\nexport default handler;\n".to_string()).unwrap(), account_credit: 1}));

        let call_op = dummy_op("edsigtjxGWgim6Jz5c5PqotHsV27niAScXaJWaVqFJFrdjGUiicZ6asnQQYwN9Uk5beYou6FpKYbFMaL4S3PkfMVxo2EJq6EHwD", 1, Content::RunFunction(RunFunction { uri: Uri::from_static("jstz://KT1UW7TWuCKbVbTDtek2gbvDwRCLfK3M1hVd/"), method: Method::GET, headers: HeaderMap::new(), body: None, gas_limit: 550000 }));

        let dst_account_path =
            RefPath::assert_from(b"/jstz_account/tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx");

        // the destination account should not exist yet
        assert!(h.store_has(&dst_account_path).unwrap().is_none());

        // init account that deploys the function
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

        // deploy smart function
        super::process_message(&mut h, deploy_op).unwrap();
        let v = Receipt::decode(&h.store_read_all(&RefPath::assert_from(b"/jstz_receipt/c5daa7fc294c01e4777a96ab35feb55ea5c947e7495ce60e1441e0e5b1b104b3")).unwrap()).unwrap();
        assert!(matches!(
            v.result,
            ReceiptResult::Success(ReceiptContent::DeployFunction(
                DeployFunctionReceipt { address: SmartFunctionHash(Kt1Hash(addr)) }
            )) if addr.to_base58_check() == "KT1UW7TWuCKbVbTDtek2gbvDwRCLfK3M1hVd"
        ));

        // call smart function
        super::process_message(&mut h, call_op).unwrap();
        let (v, _) = bincode::decode_from_slice::<Receipt, _>(&h.store_read_all(&RefPath::assert_from(b"/jstz_receipt/03bce30cce2bd94e9d346f4c70933fb0f0bf800c02bcbcd734e19bac76a2b374")).unwrap(), bincode::config::legacy()).unwrap();
        assert!(matches!(
            v.result,
            ReceiptResult::Success(ReceiptContent::RunFunction(RunFunctionReceipt {
                body: _,
                status_code: StatusCode::OK,
                headers: _
            }))
        ));

        // check if transfer is performed by the smart function
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
