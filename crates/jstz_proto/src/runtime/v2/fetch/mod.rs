mod error;
mod fetch_handler;
mod host_script;
mod http;
mod resources;

/// Provides the backend for Deno's [fetch](https://docs.deno.com/api/web/~/fetch) which structures
/// its implementation into two steps to allow an [abort handler](https://github.com/jstz-dev/deno/blob/v2.1.10-jstz/ext/fetch_base/26_fetch.js#L182)
/// to be registered in JS in between the two routines
///
/// 1. [`fetch`] creates a future that will dispatch the request to the appropriate handler depending
///    on the scheme then store the future in the Resource table
/// 2. [`fetch_send`] awaits for the future to complete then returns the response with its body hidden
///    behind a Resource which allows the body to be consumed as an [async JS ReadableStream](https://developer.mozilla.org/en-US/docs/Web/API/Streams_API/Using_readable_streams#consuming_a_fetch_using_asynchronous_iteration)
///
/// Unlike smart function calls in the V1 runtime, [`JstzFetchHandler`] decouples succesful Transaction from
/// unsuccessful Responses. This allows callees to send transfers within the header of an error response. However,
/// there are a few things to be mindful off in its current state
///
/// 1. State updates, including transfers, can only be rolledback when the smart function throws an error. A
///    dedicate `abort` API is necessary to perform this more gracefully
/// 2. Although the  `fetch` API is asynchronous, Transactions are not. Trying to `fetch` two smart functions
///    concurrently is Undefined Behaviour
///
/// Current behaviour
///
/// * Calling conveniton
///     - `fetch` should target a `jstz` schemed URL with host referencing a valid Smart Function address (callee)
///     -  The callee smart function should export a default hander that accepts a Request and returns a Response
/// *  Header hygiene in Request/Response
///     - The "referrer" header key will be set to/replaced with the caller's address
///     - "x-jstz-*" header keys will be removed if present except valid header "x-jstz-transfer"
/// *. Header transfer
///     - If the "x-jstz-transfer: <amount>" header key is present, the protocol will attempt to transfer <amount> from caller to callee.
///       If successful, the "x-jstz-transfer" key will be replaced by "x-jstz-amount". If not, the callee will returns an error Response.
///       Header transfers also apply to Responses but from callee to caller.
/// * Transaction
///     - A new transaction snapshot is created before running the callee's handler and committed/rolledback after it completes
/// * Errors
///      - If the callee's script throws an uncaught eror, `fetch` will automatically wrap it into a 500 InternalServerError and
///        the transaction is rolled back
///      - If the callee's script returns 200 < code <= 300 Response, the headers will be cleansed of unexpected headers and the transaction
///        rolled back. That is, anything that isn't a success isn't expected to update state.

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use jstz_core::{
        host::{HostRuntime, JsHostRuntime},
        kv::Transaction,
    };
    use jstz_crypto::{
        public_key_hash::PublicKeyHash, smart_function_hash::SmartFunctionHash,
    };
    use jstz_utils::TOKIO;

    use serde_json::{json, Value as JsonValue};
    use url::Url;

    use super::fetch_handler::process_and_dispatch_request;
    use crate::context::account::{Account, Addressable, Amount};
    use crate::runtime::ParsedCode;

    // Deploy a vec of smart functions from the same creator, each
    // with `amount` XTZ. Returns a vec of hashes corresponding to
    // each sf deployed
    fn deploy_smart_functions<const N: usize>(
        scripts: [&str; N],
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        creator: &impl Addressable,
        amount: Amount,
    ) -> [SmartFunctionHash; N] {
        let mut hashes = vec![];
        for i in 0..N {
            // Safety
            // Script is valid
            let hash = Account::create_smart_function(hrt, tx, creator, amount, unsafe {
                ParsedCode::new_unchecked(scripts[i].to_string())
            })
            .unwrap();
            hashes.push(hash);
        }

        hashes.try_into().unwrap()
    }

    fn setup<'a, const N: usize>(
        host: &mut tezos_smart_rollup_mock::MockHost,
        scripts: [&'a str; N],
    ) -> (
        JsHostRuntime<'static>,
        Transaction,
        PublicKeyHash,
        [SmartFunctionHash; N],
    ) {
        let mut host = JsHostRuntime::new(host);
        let mut tx = jstz_core::kv::Transaction::default();
        tx.begin();
        let source_address = jstz_mock::account1();
        let hashes =
            deploy_smart_functions(scripts, &mut host, &mut tx, &source_address, 0);
        (host, tx, source_address, hashes)
    }

    // Script simply fetches the smart function given in the path param
    // eg. jstz://<host address>/<remote address> will call fetch("jstz://<remote address>")
    const SIMPLE_REMOTE_CALLER: &str = "export default async (req) => await fetch(`jstz://${new URL(req.url).pathname.substring(1)}`)";

    // Run behaviour

    // Fetch with `jstz` scheme runs a smart function.
    #[test]
    fn fetch_runs_smart_function() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (_req) => new Response("hello world")"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (host, tx, source_address, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                host,
                tx,
                source_address.into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            assert_eq!(
                "hello world",
                String::from_utf8(response.body.into()).unwrap()
            );
        });
    }

    // Fetch rejects unsupported schemes runs a smart function.
    #[test]
    fn fetch_rejects_unsupported_scheme() {
        TOKIO.block_on(async {

        // Code
        let run = "export default async (req) => await fetch(`tezos://${new URL(req.url).pathname.substring(1)}`)";

        // Setup
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        let (host, tx, source_address, hashes) = setup(&mut host, [run]);
        let run_address = hashes[0].clone();

        // Run
        let response = process_and_dispatch_request(
            host,
            tx,
            source_address.into(),
            "GET".into(),
            Url::parse(format!("jstz://{}/{}", run_address, run_address).as_str())
                .unwrap(),
            vec![],
            None,
        )
        .await;

        // Assert
        assert_eq!(response.status, 500);
        assert_eq!(response.status_text, "InternalServerError");
        assert_eq!(
            json!({"class":"TypeError","message":"Unsupport scheme 'tezos'"}),
            serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                .unwrap()
        );
    });
    }

    // Fetch rejects unsupported schemes runs a smart function.
    #[test]
    fn fetch_rejects_unsupported_address_scheme() {
        TOKIO.block_on(async {
            // Code
            let run = "export default async (req) => await fetch(`jstz://abc123`)";

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (host, tx, source_address, hashes) = setup(&mut host, [run]);
            let run_address = hashes[0].clone();

            // Run
            let response = process_and_dispatch_request(
                host,
                tx,
                source_address.into(),
                "GET".into(),
                Url::parse(format!("jstz://{}", run_address).as_str()).unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            assert_eq!(response.status, 500);
            assert_eq!(response.status_text, "InternalServerError");
            assert_eq!(
                json!({"class":"RuntimeError","message":"InvalidAddress"}),
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap()
            );
        });
    }

    // Smart functions must return a Response if successfully ran
    #[test]
    fn smart_function_must_return_response() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (_req) => {}"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (host, tx, source_address, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                host,
                tx,
                source_address.into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!("InternalServerError", response.status_text);
            assert_eq!(500, response.status);
            assert_eq!(
                json!({"class": "TypeError","message":"Invalid Response type"}),
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap()
            );
        });
    }

    #[test]
    fn fetch_supports_empty_response_body() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (_req) => new Response()"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (host, tx, source_address, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();
            // Run
            let response = process_and_dispatch_request(
                host,
                tx,
                source_address.into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            let body: Vec<u8> = response.body.into();
            assert!(body.is_empty());
        })
    }

    // Global changes are isolated between smart function calls
    #[test]
    fn fetch_provides_isolation() {
        TOKIO.block_on(async {
            // Code
            let run = r#"export default async (req) => {
            let address = new URL(req.url).pathname.substring(1);
            globalThis.leakyState = "abc"
            return await fetch(`jstz://${address}`)
        }"#;
            let remote = r#"export default async (_req) => {
            if (globalThis.leakyState ===  "abc") {  throw new Error("leak detected!"); }
            return new Response("hello world")
        }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (host, tx, source_address, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                host,
                tx,
                source_address.into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            assert_eq!(
                "hello world",
                String::from_utf8(response.body.into()).unwrap()
            )
        });
    }

    // Fetch can be called recursively (re-entrant)
    // FIXME: Smart functions should not be re-entrant by default
    #[test]
    fn fetch_recursive() {
        TOKIO.block_on(async {
            // Code
            let run = include_str!("tests/resources/recursive/run.js");

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run]);
            let run_address = hashes[0].clone();

            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx,
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}", run_address).as_str()).unwrap(),
                vec![],
                None,
            )
            .await;

            let json =
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap();
            assert_eq!(
                json!({
                    "count": 3
                }),
                json
            )
        })
    }

    // Racing multiple fetch calls is awaitable at different points of the program
    #[test]
    fn fetch_raceable() {
        TOKIO.block_on(async {
            // Code
            let run = include_str!("tests/resources/raceable/run.js");
            let remote = include_str!("tests/resources/raceable/remote.js");

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx,
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            let json =
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap();
            assert_eq!(
                json!({
                    "data": 3
                }),
                json
            )
        });
    }

    // The default behaviour of deno async is to run eagerly, even when not awaited on, for
    // latency reasons. This means that side effects like KV updates and transfers are performed
    // when the execution completes successfully even when not awaited on
    #[test]
    fn fetch_eagerly_executes() {
        TOKIO.block_on(async {
            // Code
            let run = r#"export default async (req) => {
            let address = new URL(req.url).pathname.substring(1);
            fetch(`jstz://${address}/5`)
            fetch(`jstz://${address}/-3`)
            return new Response()
        }"#;
            let remote = r#"export default async (req) => {
            let incr = Number.parseInt(new URL(req.url).pathname.substring(1));
            let value = Kv.get("value") ?? 0;
            Kv.set("value", value + incr);
            return new Response()
        }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, mut tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let _ = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            // check transaction was commited with unawaited on values
            let kv = jstz_runtime::ext::jstz_kv::kv::Kv::new(remote_address.to_string());
            let result = kv.get(&mut host, &mut tx, "value").unwrap().0.clone();
            assert_eq!(2, serde_json::from_value::<usize>(result).unwrap());
        });
    }

    // Headers processing behaviour

    #[test]
    fn fetch_default_headers() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (req) => {
            let body = Object.fromEntries(req.headers.entries());
            return new Response(JSON.stringify(body))
        }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            let request_headers =
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap();
            assert_eq!(
                json!({
                    "accept":"*/*",
                    "accept-language":"*",
                    "referrer":"KT1WEAA8whopt6FqPodVErxnQysYSkTan4wS"
                }),
                request_headers
            );

            let response_headers: HashMap<String, String> = response
                .headers
                .into_iter()
                .map(|(k, v)| {
                    (
                        String::from_utf8(k.as_slice().to_vec()).unwrap(),
                        String::from_utf8(v.as_slice().to_vec()).unwrap(),
                    )
                })
                .collect();
            assert_eq!(
                json!({
                    "content-type":"text/plain;charset=UTF-8",
                }),
                serde_json::to_value(response_headers).unwrap()
            );
        })
    }

    #[test]
    fn request_header_has_referrer() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (req) => {
            let body = Object.fromEntries(req.headers.entries());
            return new Response(JSON.stringify(body))
        }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            let request_headers =
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap();
            assert!(request_headers["referrer"] == run_address.to_string());
        })
    }

    #[test]
    fn fetch_replaces_referrer_in_request_header() {
        TOKIO.block_on(async {

        // Code
        let run = r#"export default async (req) => {
            let address = new URL(req.url).pathname.substring(1);
            let request = new Request(`jstz://${address}`, {
                headers: {
                    Referrer: req.headers.get("referrer") // Tries to forward referrer
                }
            });
            return await fetch(request)
        }"#;
        let remote =
            r#"export default async (req) => new Response(req.headers.get("referrer"))"#;

        // Setup
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
        let run_address = hashes[0].clone();
        let remote_address = hashes[1].clone();

        // Run
        let response = process_and_dispatch_request(
            JsHostRuntime::new(&mut host),
            tx.clone(),
            jstz_mock::account1().into(),
            "GET".into(),
            Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                .unwrap(),
            vec![],
            None,
        )
        .await;

        assert_eq!(
            run_address.to_string(),
            String::from_utf8(response.body.to_vec()).unwrap()
        );
    })
    }

    #[test]
    fn transfer_succeeds() {
        TOKIO.block_on(async {
            // Code
            let run = include_str!("tests/resources/transfer_succeeds/run.js");
            let remote = include_str!("tests/resources/transfer_succeeds/remote.js");

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, mut tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Adds 10 XTZ
            let _ = Account::add_balance(&mut host, &mut tx, &run_address, 10_000_000);

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert!(response.status == 200);
            assert_eq!(
                8_000_000,
                Account::balance(&mut host, &mut tx, &run_address).unwrap()
            );
            assert_eq!(
                2_000_000,
                Account::balance(&mut host, &mut tx, &remote_address).unwrap()
            );
        })
    }

    #[test]
    fn transfer_fails_when_error_thrown() {
        TOKIO.block_on(async {
            let run =
                include_str!("tests/resources/transfer_fails_when_error_thrown/run.js");
            let remote = include_str!(
                "tests/resources/transfer_fails_when_error_thrown/remote.js"
            );

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, mut tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Adds 10 XTZ
            let _ = Account::add_balance(&mut host, &mut tx, &run_address, 10_000_000);

            // Run
            let _ = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!(
                10_000_000,
                Account::balance(&mut host, &mut tx, &run_address).unwrap()
            );
            assert_eq!(
                0,
                Account::balance(&mut host, &mut tx, &remote_address).unwrap()
            );
        })
    }

    #[test]
    fn fetch_cleans_headers() {
        TOKIO.block_on(async {
            let run = include_str!("tests/resources/fetch_cleans_headers/run.js");
            let remote = include_str!("tests/resources/fetch_cleans_headers/remote.js");

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, mut tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Adds 10 XTZ
            let _ = Account::add_balance(&mut host, &mut tx, &run_address, 10_000_000);

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert!(response.status == 200);
            assert_eq!(
                9_000_000,
                Account::balance(&mut host, &mut tx, &run_address).unwrap()
            );
            assert_eq!(
                1_000_000,
                Account::balance(&mut host, &mut tx, &remote_address).unwrap()
            );
        })
    }

    #[test]
    fn transfer_rejects_when_invalid() {
        TOKIO.block_on(async {
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"
            export default async (req) => {
                return new Response(null, {
                    headers: {
                        "X-JSTZ-TRANSFER": 100
                    }
                })
            }
        "#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!(500, response.status);
            assert_eq!(
                json!({"class":"RuntimeError","message":"InsufficientFunds"}),
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap()
            )
        })
    }

    #[test]
    fn transfer_fails_when_status_not_2xx() {
        TOKIO.block_on(async {
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"
            export default async (req) => {
                return new Response(null, {
                    status: 400,
                    headers: {
                        "X-JSTZ-TRANSFER": 4000000
                    }
                })
            }
        "#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, mut tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Adds 10 XTZ
            let _ = Account::add_balance(&mut host, &mut tx, &remote_address, 10_000_000);

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!(400, response.status);
            assert_eq!(
                0,
                Account::balance(&mut host, &mut tx, &run_address).unwrap()
            );
            assert_eq!(
                10_000_000,
                Account::balance(&mut host, &mut tx, &remote_address).unwrap()
            );
        });
    }

    // Transaction behaviour

    #[test]
    fn transaction_rolled_back_when_error_thrown() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (_req) => {
            Kv.set("test", 123)
            throw new Error("boom")
        }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let _ = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // check transaction was commited with unawaited on values
            let kv = jstz_runtime::ext::jstz_kv::kv::Kv::new(remote_address.to_string());
            let mut tx = tx;
            let result = kv.get(&mut host, &mut tx, "test");
            assert!(result.is_none())
        });
    }

    #[test]
    fn transaction_rolled_back_when_status_not_2xx() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (_req) => {
            Kv.set("test", 123)
            return new Response(null, { status: 500 })
        }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let _ = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // check transaction was commited with unawaited on values
            let kv = jstz_runtime::Kv::new(remote_address.to_string());
            let mut tx = tx;
            let result = kv.get(&mut host, &mut tx, "test");
            assert!(result.is_none())
        });
    }

    // Error behaviour

    // Errors that are a result of evaluating the request (server side issues) are converted
    // into an error response
    #[test]
    fn error_during_sf_execution_converts_to_error_response() {
        TOKIO.block_on(async {

        // Code
        let run = SIMPLE_REMOTE_CALLER;
        let remote = r#"export default async (_req) => {
            throw new Error("boom");
        }"#;

        // Setup
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
        let run_address = hashes[0].clone();
        let remote_address = hashes[1].clone();

        // Run
        let response = process_and_dispatch_request(
            JsHostRuntime::new(&mut host),
            tx.clone(),
            jstz_mock::account1().into(),
            "GET".into(),
            Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                .unwrap(),
            vec![],
            None,
        )
        .await;

        assert_eq!("InternalServerError", response.status_text);
        assert_eq!(500, response.status);
        assert_eq!(
            json!({"class":"RuntimeError","message":"Error: boom\n    at default (jstz://KT1WSFFotGccKa4WZ5PNQGT3EgsRutzLMD4z:2:19)"}),
            serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                .unwrap()
        );
    });
    }

    // Fetch API compliance
    // TODO: https://github.com/jstz-dev/jstz/pull/982
    #[allow(dead_code)]
    fn request_get_reader_supported() {}
}
