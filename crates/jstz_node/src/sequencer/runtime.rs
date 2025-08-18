use std::path::PathBuf;

use anyhow::{anyhow, bail, Context};
use jstz_core::kv::{Storage, Transaction};
use jstz_crypto::{
    hash::Hash, public_key::PublicKey, smart_function_hash::SmartFunctionHash,
};
use jstz_kernel::inbox::Message;
use jstz_proto::executor::{execute_internal_operation, execute_operation};
use jstz_utils::KeyPair;
use tezos_smart_rollup::{
    prelude::{debug_msg, Runtime},
    storage::path::RefPath,
};

use super::{db::Db, host::Host};

const TICKETER_PATH: RefPath = RefPath::assert_from(b"/ticketer");
const INJECTOR_PATH: RefPath = RefPath::assert_from(b"/injector");

pub const TICKETER: &str = "KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ";
pub const JSTZ_ROLLUP_ADDRESS: &str = "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK";

pub fn init_host(
    db: Db,
    preimage_dir: PathBuf,
    injector: &KeyPair,
) -> anyhow::Result<Host> {
    let mut host = Host::new(db, preimage_dir);
    let ticketer = SmartFunctionHash::from_base58(TICKETER)
        .context("failed to parse ticketer address")?;

    host.store_write_all(
        &TICKETER_PATH,
        &bincode::encode_to_vec(&ticketer, bincode::config::legacy())
            .context("failed to encode ticketer")?,
    )
    .context("failed to write ticketer to host store")?;

    host.store_write_all(
        &INJECTOR_PATH,
        &bincode::encode_to_vec(&injector.0, bincode::config::legacy())
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

pub async fn process_message(rt: &mut impl Runtime, op: Message) -> anyhow::Result<()> {
    let ticketer = read_ticketer(rt).ok_or(anyhow!("Ticketer not found"))?;
    let injector = read_injector(rt).ok_or(anyhow!("Revealer not found"))?;
    let mut tx = Transaction::default();
    tx.begin();
    let receipt = match op {
        Message::External(op) => {
            execute_operation(rt, &mut tx, op, &ticketer, &injector).await
        }
        Message::Internal(op) => execute_internal_operation(rt, &mut tx, op).await,
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
    use std::{
        io::{Read, Write},
        path::PathBuf,
    };

    use axum::http::{HeaderMap, Method, StatusCode, Uri};
    use jstz_core::{host::HostRuntime, reveal_data::RevealData, BinEncodable};
    use jstz_crypto::{
        hash::Hash,
        public_key::PublicKey,
        secret_key::SecretKey,
        smart_function_hash::{Kt1Hash, SmartFunctionHash},
    };
    use jstz_proto::{
        context::account::{Account, Address, Nonce, UserAccount},
        executor::fa_deposit::FaDepositReceipt,
        operation::{
            internal::{Deposit, FaDeposit, InboxId},
            Content, DeployFunction, InternalOperation, Operation, RevealLargePayload,
            RunFunction, SignedOperation,
        },
        receipt::{
            DeployFunctionReceipt, DepositReceipt, Receipt, ReceiptContent,
            ReceiptResult, RunFunctionReceipt,
        },
        runtime::ParsedCode,
        HttpBody,
    };
    use tempfile::{NamedTempFile, TempDir};
    use tezos_smart_rollup::{
        michelson::ticket::TicketHash,
        storage::path::{OwnedPath, RefPath},
    };

    use crate::{sequencer::db::Db, test::default_injector};

    fn dummy_op(nonce: u64, content: Content) -> SignedOperation {
        let operation = Operation {
            public_key: jstz_mock::pk1(),
            nonce: Nonce(nonce),
            content,
        };
        SignedOperation::new(jstz_mock::sk1().sign(operation.hash()).unwrap(), operation)
    }

    #[test]
    fn init_host() {
        let keys = KeyPair(
            PublicKey::from_base58(
                "edpkv8EUUH68jmo3f7Um5PezmfGrRF24gnfLpH3sVNwJnV5bVCxL2n",
            )
            .unwrap(),
            SecretKey::from_base58(
                "edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm",
            )
            .unwrap(),
        );
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let rt = super::init_host(db, PathBuf::new(), &keys).unwrap();
        assert_eq!(
            super::read_ticketer(&rt).unwrap(),
            SmartFunctionHash::from_base58(TICKETER).unwrap()
        );
        assert_eq!(
            super::read_injector(&rt).expect("Revealer not found"),
            keys.0
        );
    }

    #[tokio::test]
    async fn process_message() {
        // Using a slightly complicated scenario here to check if transaction works properly.
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let debug_log_file = NamedTempFile::new().unwrap();
        let mut h = super::init_host(db, PathBuf::new(), &default_injector())
            .unwrap()
            .with_debug_log_file(debug_log_file.path())
            .unwrap();

        // This smart function has about 8k characters. The runtime is okay with it and simply
        // stores it in the data store, though this would not work with a rollup.
        let deploy_op = dummy_op( 0, Content::DeployFunction(DeployFunction {function_code: ParsedCode::try_from(format!("const handler = async () => {{ const s = \"{}\"; const myHeaders = new Headers();  myHeaders.append(\"X-JSTZ-TRANSFER\", \"1\"); return await fetch(new Request(\"jstz://tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx/\", {{ headers: myHeaders }})); }}; export default handler;", "a".repeat(8000))).unwrap(), account_credit: 1}));

        let call_op = dummy_op(
            1,
            Content::RunFunction(RunFunction {
                uri: Uri::from_static("jstz://KT1WjrJgoaEDHF2RmhhnpjjiwBkt4nA2MiMo/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: HttpBody::empty(),
                gas_limit: 550000,
            }),
        );

        let src_account_address = jstz_mock::pkh1();
        let dst_account_address = "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx";

        let dst_account_path =
            OwnedPath::try_from(format!("/jstz_account/{dst_account_address}")).unwrap();

        // The destination account should not exist yet
        assert!(h.store_has(&dst_account_path).unwrap().is_none());

        // Initialise account that deploys the function
        h.store_write_all(
            &RefPath::assert_from(
                format!("/jstz_account/{src_account_address}").as_bytes(),
            ),
            &Account::User(UserAccount {
                amount: 1000000,
                nonce: Nonce(0),
            })
            .encode()
            .unwrap(),
        )
        .unwrap();

        // Deploy smart function
        let deploy_op_hash = deploy_op.hash();
        super::process_message(&mut h, Message::External(deploy_op))
            .await
            .unwrap();
        let v = Receipt::decode(
            &h.store_read_all(&RefPath::assert_from(
                format!("/jstz_receipt/{deploy_op_hash}").as_bytes(),
            ))
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            v.result,
            ReceiptResult::Success(ReceiptContent::DeployFunction(
                DeployFunctionReceipt { address: SmartFunctionHash(Kt1Hash(addr)) }
            )) if addr.to_base58_check() == "KT1WjrJgoaEDHF2RmhhnpjjiwBkt4nA2MiMo"
        ));

        // Call smart function
        let call_op_hash = call_op.hash();
        super::process_message(&mut h, Message::External(call_op))
            .await
            .unwrap();
        let v = Receipt::decode(
            &h.store_read_all(&RefPath::assert_from(
                format!("/jstz_receipt/{call_op_hash}").as_bytes(),
            ))
            .unwrap(),
        )
        .unwrap();
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

        // Check debug log file
        let mut buf = String::new();
        std::fs::File::open(debug_log_file.path())
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();
        assert!(
            buf.contains("Smart function deployed: KT1WjrJgoaEDHF2RmhhnpjjiwBkt4nA2MiMo")
        );
    }

    #[tokio::test]
    async fn process_message_deposit() {
        // Using a slightly complicated scenario here to check if transaction works properly.
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let mut h = super::init_host(db, PathBuf::new(), &default_injector()).unwrap();

        let receiver =
            Address::from_base58("tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx").unwrap();

        let deposit = Deposit {
            inbox_id: InboxId {
                l1_level: 1,
                l1_message_id: 1,
            },
            amount: 10,
            receiver,
            source: jstz_mock::account1(),
        };
        let op_hash = deposit.hash();
        let deposit_op = Message::Internal(InternalOperation::Deposit(deposit));

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
        super::process_message(&mut h, deposit_op).await.unwrap();
        let v = Receipt::decode(
            &h.store_read_all(&RefPath::assert_from(
                format!("/jstz_receipt/{op_hash}").as_bytes(),
            ))
            .unwrap(),
        )
        .unwrap();
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

    #[tokio::test]
    async fn process_message_fa_deposit() {
        // Using a slightly complicated scenario here to check if transaction works properly.
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let mut h = super::init_host(db, PathBuf::new(), &default_injector()).unwrap();

        let receiver =
            Address::from_base58("tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx").unwrap();

        let fa_deposit = FaDeposit {
            inbox_id: InboxId {
                l1_level: 1,
                l1_message_id: 1,
            },
            amount: 10,
            source: jstz_mock::account1(),
            receiver,
            proxy_smart_function: None,
            ticket_hash: TicketHash::try_from(
                "0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            )
            .unwrap(),
        };
        let op_hash = fa_deposit.hash();
        let fa_deposit_op = Message::Internal(InternalOperation::FaDeposit(fa_deposit));

        // Execute the deposit
        super::process_message(&mut h, fa_deposit_op).await.unwrap();
        let v = Receipt::decode(
            &h.store_read_all(&RefPath::assert_from(
                format!("/jstz_receipt/{op_hash}").as_bytes(),
            ))
            .unwrap(),
        )
        .unwrap();

        assert!(matches!(
            v.result,
            ReceiptResult::Success(ReceiptContent::FaDeposit(FaDepositReceipt {
                receiver : Address::User(addr),
                ticket_balance,
                ..
            })) if ticket_balance == 10 && addr.to_base58() == "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
        ));
    }

    #[tokio::test]
    async fn process_message_large_payload() {
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let preimage_dir = TempDir::new().unwrap();
        let path = preimage_dir.path().to_path_buf();

        let injector_sk = SecretKey::from_base58(
            "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
        )
        .unwrap();

        // Deploy large smart function
        let deploy_fn = Operation {
            public_key: PublicKey::from_base58("edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav").unwrap(),
            nonce: 1.into(),
            content: DeployFunction {
                // # Safety: Ok in test
                function_code:
                    ParsedCode::try_from(format!(
                        "const handler = (request) => {{ let x = '{}'; return new Response('this is a big function'); }}; export default handler;",
                        "a".repeat(5000))).unwrap()
                ,
                account_credit: 0,
            }
            .into(),
        };
        let deploy_op_hash = hex::encode(deploy_fn.hash());
        let signature = injector_sk.sign(deploy_fn.hash()).unwrap();
        let signed_deploy_fn = SignedOperation::new(signature, deploy_fn);

        let preimage_hash =
            // 5345 bytes, 3 pages
            RevealData::encode_and_prepare_preimages(&signed_deploy_fn, |hash, data| {
                std::fs::File::create(path.join(hash.to_string()))
                    .unwrap()
                    .write_all(&data)
                    .unwrap();
            })
            .unwrap();

        let large_payload = Operation {
            public_key: PublicKey::from_base58(
                "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
            )
            .unwrap(),
            nonce: 0.into(),
            content: RevealLargePayload {
                root_hash: preimage_hash,
                reveal_type: jstz_proto::operation::RevealType::DeployFunction,
                original_op_hash: signed_deploy_fn.hash(),
            }
            .into(),
        };

        let signature = injector_sk.sign(large_payload.hash()).unwrap();
        let signed_large_payload = SignedOperation::new(signature, large_payload);

        let mut h = super::init_host(db, path, &default_injector()).unwrap();

        super::process_message(&mut h, Message::External(signed_large_payload))
            .await
            .unwrap();
        let v = Receipt::decode(
            &h.store_read_all(
                &OwnedPath::try_from(format!("/jstz_receipt/{deploy_op_hash}")).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            v.result,
            ReceiptResult::Success(ReceiptContent::DeployFunction(
                DeployFunctionReceipt { address: SmartFunctionHash(Kt1Hash(addr)) }
            )) if addr.to_base58_check() == "KT1FTckranMJ2on3TDufWqJumzSyRUd1tQf2"
        ));

        let run_op = Operation {
            public_key: PublicKey::from_base58(
                "edpkuERbaNDzoXLskejBgBtySZxFN84t4iBKoSHYKRfzbK74HoP1zX",
            )
            .unwrap(),
            nonce: 0.into(),
            content: Content::RunFunction(RunFunction {
                uri: Uri::from_static("jstz://KT1FTckranMJ2on3TDufWqJumzSyRUd1tQf2/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: HttpBody::empty(),
                gas_limit: 550000,
            }),
        };
        let sk = SecretKey::from_base58(
            "edsk4aBPdyDUC4V7RJ5dFTKDTpzMP2sGbAfXSRMPYGdFmXorj9RAYp",
        )
        .unwrap();
        let signature = sk.sign(run_op.hash()).unwrap();
        let signed = SignedOperation::new(signature, run_op);

        let op_hash = signed.hash();

        super::process_message(&mut h, Message::External(signed))
            .await
            .unwrap();
        let v = Receipt::decode(
            &h.store_read_all(
                &OwnedPath::try_from(format!("/jstz_receipt/{op_hash}")).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            v.result,
            ReceiptResult::Success(ReceiptContent::RunFunction(RunFunctionReceipt {
                body,
                status_code: StatusCode::OK,
                headers: _
            })) if String::from_utf8(body.clone().unwrap()).unwrap() == "this is a big function"));
    }
}
