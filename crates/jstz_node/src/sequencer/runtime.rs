use std::path::PathBuf;

use anyhow::{anyhow, bail, Context};
use jstz_core::kv::{Storage, Transaction};
use jstz_crypto::{
    hash::Hash, public_key::PublicKey, smart_function_hash::SmartFunctionHash,
};
use jstz_proto::{
    executor::{deposit, execute_operation},
    operation::InternalOperation,
};
use tezos_smart_rollup::{
    prelude::{debug_msg, Runtime},
    storage::path::RefPath,
};

use crate::sequencer::inbox::parsing::Message;

use super::db::Db;

const TICKETER_PATH: RefPath = RefPath::assert_from(b"/ticketer");
const INJECTOR_PATH: RefPath = RefPath::assert_from(b"/injector");
const INJECTOR_PK: &str = "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav";

pub const TICKETER: &str = "KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ";
pub const JSTZ_ROLLUP_ADDRESS: &str = "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK";

pub fn init_host(db: Db, preimage_dir: PathBuf) -> anyhow::Result<impl Runtime> {
    let mut host = crate::sequencer::host::Host::new(db, preimage_dir);
    let ticketer = SmartFunctionHash::from_base58(TICKETER)
        .context("failed to parse ticketer address")?;

    host.store_write_all(
        &TICKETER_PATH,
        &bincode::encode_to_vec(&ticketer, bincode::config::legacy())
            .context("failed to encode ticketer")?,
    )
    .context("failed to write ticketer to host store")?;

    let injector = PublicKey::from_base58(INJECTOR_PK)
        .context("failed to parse injector public key")?;

    host.store_write_all(
        &INJECTOR_PATH,
        &bincode::encode_to_vec(&injector, bincode::config::legacy())
            .context("failed to encode injector")?,
    )
    .context("failed to write injector to host store")?;

    Ok(host)
}

fn read_ticketer(rt: &impl Runtime) -> Option<SmartFunctionHash> {
    Storage::get(rt, &TICKETER_PATH).ok()?
}

fn read_injector(rt: &impl Runtime) -> Option<PublicKey> {
    Storage::get(rt, &INJECTOR_PATH).ok()?
}

pub fn process_message(rt: &mut impl Runtime, op: Message) -> anyhow::Result<()> {
    let ticketer = read_ticketer(rt).ok_or(anyhow!("Ticketer not found"))?;
    let injector = read_injector(rt).ok_or(anyhow!("Revealer not found"))?;
    let mut tx = Transaction::default();
    tx.begin();
    let receipt = match op {
        Message::External(op) => jstz_utils::TOKIO
            .block_on(execute_operation(rt, &mut tx, op, &ticketer, &injector)),
        Message::Internal(op) => match op {
            InternalOperation::Deposit(op) => deposit::execute(rt, &mut tx, op),
            _ => {
                // TODO: handle fa deposit
                // https://linear.app/tezos/issue/JSTZ-640/fa-deposit
                bail!("FA deposit not supported");
            }
        },
    };
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
    use super::*;
    use std::{io::Write, path::PathBuf};

    use axum::http::{HeaderMap, Method, StatusCode, Uri};
    use jstz_core::{host::HostRuntime, reveal_data::PreimageHash, BinEncodable};
    use jstz_crypto::{
        hash::{Blake2b, Hash},
        public_key::PublicKey,
        signature::Signature,
        smart_function_hash::{Kt1Hash, SmartFunctionHash},
    };
    use jstz_proto::{
        context::account::{Account, Address, Nonce, UserAccount},
        operation::{
            internal::Deposit, Content, DeployFunction, Operation, RevealLargePayload,
            RunFunction, SignedOperation,
        },
        receipt::{
            DeployFunctionReceipt, DepositReceipt, Receipt, ReceiptContent,
            ReceiptResult, RunFunctionReceipt,
        },
        runtime::ParsedCode,
    };
    use tempfile::{NamedTempFile, TempDir};
    use tezos_crypto_rs::hash::{Ed25519Signature, PublicKeyEd25519};
    use tezos_smart_rollup::storage::path::RefPath;

    use crate::sequencer::db::Db;

    fn dummy_op(
        sig_str: &str,
        public_key_str: &str,
        nonce: u64,
        content: Content,
    ) -> SignedOperation {
        SignedOperation::new(
            Signature::Ed25519(
                Ed25519Signature::from_base58_check(sig_str).unwrap().into(),
            ),
            Operation {
                public_key: PublicKey::Ed25519(
                    PublicKeyEd25519::from_base58_check(public_key_str)
                        .unwrap()
                        .into(),
                ),
                nonce: Nonce(nonce),
                content,
            },
        )
    }

    fn dummy_int_op(amount: u64, receiver: Address) -> Message {
        let inner = InternalOperation::Deposit(Deposit {
            inbox_id: 1,
            amount,
            receiver,
        });
        Message::Internal(inner)
    }

    #[test]
    fn init_host() {
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let rt = super::init_host(db, PathBuf::new()).unwrap();
        assert_eq!(
            super::read_ticketer(&rt).unwrap(),
            SmartFunctionHash::from_base58(TICKETER).unwrap()
        );
        assert_eq!(
            super::read_injector(&rt).expect("Revealer not found"),
            PublicKey::from_base58(super::INJECTOR_PK).unwrap()
        );
    }

    #[test]
    fn process_message() {
        // Using a slightly complicated scenario here to check if transaction works properly.
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let mut h = super::init_host(db, PathBuf::new()).unwrap();

        // This smart function has about 8k characters. The runtime is okay with it and simply
        // stores it in the data store, though this would not work with a rollup.
        let public_key = "edpkuXD2CqRpWoTT8p4exrMPQYR2NqsYH3jTMeJMijHdgQqkMkzvnz";
        let deploy_op = dummy_op("edsigtk8TSJBNphqsn9uD2tgMbNj9YhsgCMrJmhQQd6EKo8X6viVpZqinN7MnEjwh9EcYTV8NKr3LVEoSfjsTat58mxeemYnyTg", public_key, 0, Content::DeployFunction(DeployFunction {function_code: ParsedCode::try_from(format!("const handler = async () => {{ const s = \"{}\"; const myHeaders = new Headers();  myHeaders.append(\"X-JSTZ-TRANSFER\", \"1\"); return await fetch(new Request(\"jstz://tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx/\", {{ headers: myHeaders }})); }}; export default handler;", "a".repeat(8000))).unwrap(), account_credit: 1}));

        let call_op = dummy_op("edsigtkEeUaGxH983imqMYvWZpm24pkoMK7cnrmNWFnCefEWLfVwzjZYnvFauBh8cfww9f2UN67kEve8NDUpQ1D9u9QWsUnXaAh", public_key, 1, Content::RunFunction(RunFunction { uri: Uri::from_static("jstz://KT1CDAkLMEHKNs2VbVZeSdxYx3wWN5auGARR/"), method: Method::GET, headers: HeaderMap::new(), body: None, gas_limit: 550000 }));

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
        super::process_message(&mut h, Message::External(deploy_op)).unwrap();
        let v = Receipt::decode(&h.store_read_all(&RefPath::assert_from(b"/jstz_receipt/843a8438af97d97e134ae10bdcf10b5a6bcbf8c7d4912e65bacf1be26a5a73c3")).unwrap()).unwrap();
        assert!(matches!(
            v.result,
            ReceiptResult::Success(ReceiptContent::DeployFunction(
                DeployFunctionReceipt { address: SmartFunctionHash(Kt1Hash(addr)) }
            )) if addr.to_base58_check() == "KT1CDAkLMEHKNs2VbVZeSdxYx3wWN5auGARR"
        ));

        // Call smart function
        super::process_message(&mut h, Message::External(call_op)).unwrap();
        let v = Receipt::decode(&h.store_read_all(&RefPath::assert_from(b"/jstz_receipt/9b15976cc8162fe39458739de340a1a95c59a9bcff73bd3c83402fad6352396e")).unwrap()).unwrap();
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

    #[test]
    fn process_message_deposit() {
        // Using a slightly complicated scenario here to check if transaction works properly.
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let mut h = super::init_host(db, PathBuf::new()).unwrap();

        let receiver =
            Address::from_base58("tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx").unwrap();

        let deposit_op = dummy_int_op(10, receiver);

        let dst_account_path =
            RefPath::assert_from(b"/jstz_account/tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx");

        // The destination account should not exist yet
        assert!(h.store_has(&dst_account_path).unwrap().is_none());

        // Initialise the receiver account
        h.store_write_all(
            &RefPath::assert_from(b"/jstz_account/tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"),
            &Account::User(UserAccount {
                amount: 0,
                nonce: Nonce(0),
            })
            .encode()
            .unwrap(),
        )
        .unwrap();

        // Execute the deposit
        super::process_message(&mut h, deposit_op).unwrap();
        let v = Receipt::decode(&h.store_read_all(&RefPath::assert_from(b"/jstz_receipt/270c07945707b0a86fdbd6930e7bb3cae8978a3bcfb6659e8062ef39ec58c32a")).unwrap()).unwrap();
        assert!(matches!(
            v.result,
            ReceiptResult::Success(ReceiptContent::Deposit(DepositReceipt {
                updated_balance: 10,
                ..
            }))
        ));

        // Check if transfer is performed by the smart function
        let account =
            Account::decode(&h.store_read_all(&dst_account_path).unwrap()).unwrap();
        assert!(matches!(
            account,
            Account::User(UserAccount {
                amount: 10,
                nonce: Nonce(0),
            })
        ));
    }

    #[test]
    fn process_message_large_payload() {
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let preimage_dir = TempDir::new().unwrap();
        let path = preimage_dir.path().to_path_buf();

        // This is a smart function that has a lot of useless a's in its source and simply returns
        // a text response "this is a big function". Just large enough to fit in one preimage file.
        std::fs::File::create(path.join("003fef1ffda1460c3a5b738c91b60b036c1a8a6741bb5f15c23d7847a809b44475")).unwrap().write_all(&hex::decode(format!("0000000fe00000000040000000000000008ee52d6db7fdd691405e7ac0ad1a8c2b5d071244f3c0e01bd5fdd88302ee6f8d267a883d2c18f5e2aa29c1851463348b8c140adceabc719933ee84f724388e0600000000200000000000000073c4126614137d8c738a14a5602c5d798b24b21c179a0c70c51ee53ec1a82e450000000000000000000000004c0f000000000000636f6e73742068616e646c6572203d206173796e63202829203d3e207b20636f6e73742073203d2022{}223b2072657475726e206e657720526573706f6e73652822746869732069732061206269672066756e6374696f6e22293b207d3b206578706f72742064656661756c742068616e646c65723b0000000000000000", "61".repeat(3799))).unwrap()).unwrap();
        let mut h = super::init_host(db, path).unwrap();

        let deploy_op = dummy_op("edsigtpNrm3AoevvFfdboe5kijt5KpQWgXeaTqDNhAYD5dta8JWXHFW6afyEeCj6QsrxXg8WdRQhxaG9TDzaQx1mnC6vyMDSJ3B", super::INJECTOR_PK,0, Content::RevealLargePayload(RevealLargePayload { root_hash: PreimageHash([0, 63, 239, 31, 253, 161, 70, 12, 58, 91, 115, 140, 145, 182, 11, 3, 108, 26, 138, 103, 65, 187, 95, 21, 194, 61, 120, 71, 168, 9, 180, 68, 117]), reveal_type: jstz_proto::operation::RevealType::DeployFunction, original_op_hash: Blake2b::try_parse("aa8216661480132414f3ddd4bccc61fffd1db9961b259efb4d4d6597d3f7f6aa".to_string()).unwrap() }));

        super::process_message(&mut h, Message::External(deploy_op)).unwrap();
        let v = Receipt::decode(&h.store_read_all(&RefPath::assert_from(b"/jstz_receipt/aa8216661480132414f3ddd4bccc61fffd1db9961b259efb4d4d6597d3f7f6aa")).unwrap()).unwrap();
        assert!(matches!(
            v.result,
            ReceiptResult::Success(ReceiptContent::DeployFunction(
                DeployFunctionReceipt { address: SmartFunctionHash(Kt1Hash(addr)) }
            )) if addr.to_base58_check() == "KT1CkPcKaAKLX1eibkkTLub84nf1uXT7FYjG"
        ));

        let user_public_key = "edpkuXD2CqRpWoTT8p4exrMPQYR2NqsYH3jTMeJMijHdgQqkMkzvnz";
        let call_op = dummy_op("edsigtnvb4e2nPcfadUt7VbdMgFZByP1SUAmEWVfaLAerBGazZuVqCWZ4wjNJRxZhbjnzUfdMihXuH62APQv169xQvQvkEYQKQX", user_public_key, 1, Content::RunFunction(RunFunction { uri: Uri::from_static("jstz://KT1CkPcKaAKLX1eibkkTLub84nf1uXT7FYjG/"), method: Method::GET, headers: HeaderMap::new(), body: None, gas_limit: 550000 }));
        super::process_message(&mut h, Message::External(call_op)).unwrap();
        let v = Receipt::decode(&h.store_read_all(&RefPath::assert_from(b"/jstz_receipt/e6f9a74841205885f6dd1d639afcb39de14a2350d01519edf792631e39403b75")).unwrap()).unwrap();
        assert!(matches!(
            v.result,
            ReceiptResult::Success(ReceiptContent::RunFunction(RunFunctionReceipt {
                body,
                status_code: StatusCode::OK,
                headers: _
            })) if String::from_utf8(body.clone().unwrap()).unwrap() == "this is a big function"));
    }
}
