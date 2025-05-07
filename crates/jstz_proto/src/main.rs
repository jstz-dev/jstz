use std::sync::Arc;

use jstz_core::{host::{HostRuntime, JsHostRuntime}, kv::Transaction};
use jstz_crypto::{
    public_key_hash::PublicKeyHash, smart_function_hash::SmartFunctionHash,
};
use jstz_proto::{context::account::{Account, Addressable, ParsedCode}, runtime::v2::fetch::process_and_dispatch_request};
use parking_lot::FairMutex as Mutex;

#[cfg(feature = "riscv")]
pub fn main() {
    use url::Url;

    let tokio = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();

    let run = r#"export default async (_req) => new Response(JSON.stringify(2+2))"#;

    let mut host = tezos_smart_rollup_mock::MockHost::default();
    let (host, tx, source_address, hashes) = setup(&mut host, [run]);
    let run_address = hashes[0].clone();
    tokio.block_on(async {
        let response = process_and_dispatch_request(
            host,
            tx,
            source_address.into(),
            "GET".into(),
            Url::parse(format!("jstz://{}", run_address).as_str())
                .unwrap(),
            vec![],
            None,
        )
        .await;

        assert_eq!(
            "4",
            String::from_utf8(response.body.into()).unwrap()
        )
    });
}

fn setup<'a, const N: usize>(
    host: &mut tezos_smart_rollup_mock::MockHost,
    scripts: [&'a str; N],
) -> (
    JsHostRuntime<'static>,
    Arc<Mutex<Transaction>>,
    PublicKeyHash,
    [SmartFunctionHash; N],
) {
    let mut host = JsHostRuntime::new(host);
    let tx = Arc::new(Mutex::new(jstz_core::kv::Transaction::default()));
    tx.lock().begin();
    let source_address = jstz_mock::account1();
    let hashes =
        deploy_smart_functions(scripts, &mut host, &mut tx.lock(), &source_address);
    (host, tx, source_address, hashes)
}

fn deploy_smart_functions<const N: usize>(
    scripts: [&str; N],
    hrt: &impl HostRuntime,
    tx: &mut Transaction,
    creator: &impl Addressable,
) -> [SmartFunctionHash; N] {
    let mut hashes = vec![];
    for i in 0..N {
        let hash = Account::create_smart_function(
            hrt,
            tx,
            creator,
            0,
            ParsedCode(scripts[i].to_string()),
        )
        .unwrap();
        hashes.push(hash);
    }

    hashes.try_into().unwrap()
}
