use std::path::Path;

use deno_core::StaticModuleLoader;
use jstz_core::kv::Transaction;
use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
use jstz_runtime::{JstzRuntime, JstzRuntimeOptions, RuntimeContext};
use tezos_smart_rollup_mock::MockHost;
use url::Url;

deno_core::extension!(
    api_coverage_test,
    esm_entry_point = "ext:api_coverage_test/entrypoint.js",
    // `baseline.js` and `utils.js` are supposed to fetched from remote in advance.
    esm = [dir "tests/api_coverage", "entrypoint.js", "baseline.js", "utils.js"]
);

#[cfg_attr(feature = "skip-wpt", ignore)]
#[tokio::test]
async fn test() {
    let mut tx = Transaction::default();
    tx.begin();
    let mut host = MockHost::default();
    let address =
        SmartFunctionHash::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton").unwrap();
    let mut rt = JstzRuntime::new(JstzRuntimeOptions {
        protocol: Some(RuntimeContext::new(
            &mut host,
            &mut tx,
            address,
            String::new(),
        )),
        extensions: vec![api_coverage_test::init_ops_and_esm()],
        module_loader: std::rc::Rc::new(StaticModuleLoader::with(
            Url::parse("file://main").unwrap(),
            // `runTest` is the actual test injected via the extension `api_coverage_test`.
            "export default async () => {return await runTest();}",
        )),
        ..Default::default()
    });

    let output = execute(&mut rt).await;

    if let Ok(v) = std::env::var("OUTPUT_PATH") {
        let path = Path::new(&v);
        let output_file = std::fs::File::create(path).unwrap();
        serde_json::to_writer_pretty(output_file, &output).unwrap();
    }
}

async fn execute(rt: &mut JstzRuntime) -> serde_json::Value {
    let specifier = Url::parse("file://main").unwrap();

    let id = rt.execute_main_module(&specifier).await.unwrap();
    let result = rt.call_default_handler(id, &[]).await.unwrap();
    // The default function is supposed to return a JSON string that represents the coverage
    // data. See `entrypoint.js`.
    let json_str = result
        .open(rt.v8_isolate())
        .to_rust_string_lossy(&mut rt.handle_scope());
    serde_json::from_str(&json_str).unwrap()
}
