use jstz_core::{host::HostRuntime, kv::Transaction};

use crate::{
    context::account::Addressable,
    error::Result,
    operation::{self, OperationHash},
    receipt::RunFunctionReceipt,
};

pub const NOOP_PATH: &str = "/-/noop";
pub const X_JSTZ_TRANSFER: &str = "X-JSTZ-TRANSFER";
pub const X_JSTZ_AMOUNT: &str = "X-JSTZ-AMOUNT";

pub async fn execute(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source: &(impl Addressable + 'static),
    run_operation: operation::RunFunction,
    operation_hash: OperationHash,
) -> Result<RunFunctionReceipt> {
    crate::runtime::run_toplevel_fetch(hrt, tx, source, run_operation, operation_hash)
        .await
}

#[cfg(test)]
mod test {
    use super::*;
    use http::{HeaderMap, Method};
    use jstz_core::kv::Transaction;
    use jstz_crypto::{hash::Blake2b, smart_function_hash::SmartFunctionHash};
    use jstz_mock::host::JstzMockHost;

    use tezos_smart_rollup_mock::MockHost;

    use crate::{
        context::account::{Account, Address},
        executor::smart_function,
        operation::RunFunction,
        HttpBody,
    };

    #[cfg(not(feature = "v2_runtime"))]
    use {
        crate::context::ticket_table::TicketTable,
        crate::executor::smart_function::deploy::deploy_smart_function, serde_json::json,
    };

    #[tokio::test]
    async fn transfer_xtz_to_and_from_smart_function_succeeds() {
        let source = Address::User(jstz_mock::account1());
        // 1. Deploy the smart function
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let transfer_amount = 3;
        let refund_amount = 2;
        tx.begin();
        Account::add_balance(host, &mut tx, &source, transfer_amount)
            .expect("add balance");
        let source_balance = Account::balance(host, &mut tx, &source).unwrap();
        assert_eq!(source_balance, transfer_amount);
        tx.commit(host).unwrap();

        // 1. Deploy the smart function that transfers the balance to the source
        let code = format!(
            r#"
        const handler = async (request) => {{
            const transferred_amount = request.headers.get("X-JSTZ-AMOUNT");
            if (transferred_amount !== "{transfer_amount}") {{
                return Response.error("Invalid transferred amount");
            }}
            const headers = {{"X-JSTZ-TRANSFER": "{refund_amount}"}};
            return new Response(null, {{headers}});
        }};
        export default handler;
        "#
        );
        let parsed_code = code.to_string();
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        let balance_before = Account::balance(host, &mut tx, &smart_function).unwrap();
        assert_eq!(balance_before, 0);

        tx.commit(host).unwrap();

        // 2. Call the smart function
        tx.begin();
        let mut headers = HeaderMap::new();
        headers.insert(
            X_JSTZ_TRANSFER,
            transfer_amount.to_string().try_into().unwrap(),
        );
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
            method: Method::GET,
            headers,
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        let response = execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .await
        .expect("run function expected");

        assert!(response.headers.get(X_JSTZ_TRANSFER).is_none());
        assert!(response.headers.get(X_JSTZ_AMOUNT).is_some_and(|amt| amt
            .to_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            == refund_amount));
        tx.commit(host).unwrap();

        // 3. assert the transfer to the sf and refund to the source
        tx.begin();
        let balance_after = Account::balance(host, &mut tx, &smart_function).unwrap();
        assert_eq!(
            balance_after - balance_before,
            transfer_amount - refund_amount
        );
        assert_eq!(
            Account::balance(host, &mut tx, &source).unwrap(),
            refund_amount
        );

        // 4. transferring to the smart function should fail (source has insufficient funds)
        let result = execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .await
        .unwrap();
        assert!(result.status_code.is_server_error());

        // 5. transferring from the smart function should fail with insufficient funds and the balance is rolled back
        let balance_before = Account::balance(host, &mut tx, &source).unwrap();
        // drain the balance of the smart function
        Account::set_balance(host, &mut tx, &smart_function, 0).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            X_JSTZ_TRANSFER,
            transfer_amount.to_string().try_into().unwrap(),
        );
        let result = execute(
            host,
            &mut tx,
            &source,
            RunFunction {
                headers,
                ..run_function
            },
            fake_op_hash.clone(),
        )
        .await
        .unwrap();
        assert!(result.status_code.is_server_error());

        // tx rolled back as smart function has insufficient funds
        let balance_after = Account::balance(host, &mut tx, &source).unwrap();
        assert_eq!(balance_after, balance_before);
    }

    #[tokio::test]
    async fn transfer_xtz_to_smart_function_succeeds_with_noop_path() {
        let source = Address::User(jstz_mock::account1());
        // 1. Deploy the smart function
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_balance = 1;
        tx.begin();
        Account::add_balance(host, &mut tx, &source, initial_balance)
            .expect("add balance");
        let source_balance = Account::balance(host, &mut tx, &source).unwrap();
        assert_eq!(source_balance, initial_balance);
        tx.commit(host).unwrap();

        // 1. Deploy the smart function that refunds the balance to the source
        let code = format!(
            r#"
            const handler = async () => {{
                await fetch(new Request("jstz://{source}", {{
                    headers: {{"X-JSTZ-TRANSFER": "{initial_balance}"}}
                }}));
                return new Response();
            }};
            export default handler;
            "#
        );
        let parsed_code = code.to_string();
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        let balance_before = Account::balance(host, &mut tx, &smart_function).unwrap();
        assert_eq!(balance_before, 0);

        tx.commit(host).unwrap();

        // transfer should happen with `/-/noop` path
        tx.begin();
        let mut headers = HeaderMap::new();
        headers.insert(
            X_JSTZ_TRANSFER,
            initial_balance.to_string().try_into().unwrap(),
        );
        let run_function = RunFunction {
            uri: format!("jstz://{}/-/noop", &smart_function)
                .try_into()
                .unwrap(),
            method: Method::GET,
            headers,
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        execute(host, &mut tx, &source, run_function.clone(), fake_op_hash)
            .await
            .expect("run function expected");
        tx.commit(host).unwrap();

        tx.begin();
        let balance_after = Account::balance(host, &mut tx, &smart_function).unwrap();
        assert_eq!(balance_after - balance_before, initial_balance);
        assert_eq!(Account::balance(host, &mut tx, &source).unwrap(), 0);
    }

    #[tokio::test]
    async fn transfer_xtz_to_user_succeeds() {
        let source = Address::User(jstz_mock::account1());
        let destination = Address::User(jstz_mock::account2());
        // 1. Deploy the smart function
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_balance = 1;
        tx.begin();
        Account::add_balance(host, &mut tx, &source, initial_balance)
            .expect("add balance");
        let source_balance = Account::balance(host, &mut tx, &source).unwrap();
        assert_eq!(source_balance, initial_balance);
        tx.commit(host).unwrap();

        // 2. sending request to transfer from source to the destination
        tx.begin();
        let mut headers = HeaderMap::new();
        headers.insert(
            X_JSTZ_TRANSFER,
            initial_balance.to_string().try_into().unwrap(),
        );
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &destination).try_into().unwrap(),
            method: Method::GET,
            headers,
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        let result =
            execute(host, &mut tx, &source, run_function.clone(), fake_op_hash).await;
        assert!(result.is_ok());

        tx.commit(host).unwrap();

        tx.begin();
        let balance_after = Account::balance(host, &mut tx, &source).unwrap();
        assert_eq!(balance_after, 0);
        assert_eq!(
            Account::balance(host, &mut tx, &destination).unwrap(),
            initial_balance
        );

        // 3. transferring again should fail
        let fake_op_hash2 = Blake2b::from(b"fake_op_hash2".as_ref());
        let result = execute(host, &mut tx, &source, run_function, fake_op_hash2)
            .await
            .unwrap();
        assert!(result.status_code.is_server_error());
    }

    // TODO: https://linear.app/tezos/issue/JSTZ-656/v2-fetch-should-fail-no-invalid-headers
    // v2 runtime fetch ignores invalid headers instead of failing. It should fail
    #[cfg(not(feature = "v2_runtime"))]
    #[tokio::test]
    async fn invalid_request_should_fail() {
        let source = Address::User(jstz_mock::account1());
        // 1. Deploy the smart function
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_balance = 1;
        tx.begin();
        Account::add_balance(host, &mut tx, &source, initial_balance)
            .expect("add balance");
        tx.commit(host).unwrap();

        let code = r#"
            const handler = () => {{
                return new Response();
            }};
            export default handler;
            "#;

        // 1. Deploy smart function
        let parsed_code = code.to_string();
        tx.begin();
        let smart_function =
            deploy_smart_function(host, &mut tx, &source, parsed_code, 0).unwrap();

        tx.commit(host).unwrap();

        // Calling the smart function should error or return an error response
        tx.begin();

        let sf_balance_before = Account::balance(host, &mut tx, &smart_function).unwrap();
        let source_balance_before = Account::balance(host, &mut tx, &source).unwrap();
        let mut invalid_headers = HeaderMap::new();
        invalid_headers.insert(
            X_JSTZ_AMOUNT,
            initial_balance.to_string().try_into().unwrap(),
        );
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
            method: Method::GET,
            headers: invalid_headers,
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let result = execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            Blake2b::from(b"fake_op_hash".as_ref()),
        )
        .await;
        let sf_balance_after = Account::balance(host, &mut tx, &smart_function).unwrap();
        let source_balance_after = Account::balance(host, &mut tx, &source).unwrap();

        assert_eq!(sf_balance_before, sf_balance_after);
        assert_eq!(source_balance_before, source_balance_after);
        let call_failed = match result {
            Ok(receipt) => receipt.status_code.is_server_error(),
            _ => true,
        };
        assert!(call_failed);
    }

    // TODO: https://linear.app/tezos/issue/JSTZ-656/v2-fetch-should-fail-no-invalid-headers
    #[cfg(not(feature = "v2_runtime"))]
    #[tokio::test]
    async fn invalid_response_should_fail() {
        let source = Address::User(jstz_mock::account1());
        // 1. Deploy the smart function
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_balance = 1;
        tx.begin();
        Account::add_balance(host, &mut tx, &source, initial_balance)
            .expect("add balance");
        tx.commit(host).unwrap();

        let code = format!(
            r#"
            const handler = () => {{
                const headers = new Headers();
                return new Response(null, {{
                    headers: {{ "X-JSTZ-AMOUNT": "{initial_balance}" }},
                }});
            }};
            export default handler;
            "#
        );

        // 1. Deploy smart function
        let parsed_code = code.to_string();
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, initial_balance)
                .unwrap();

        let sf_balance_before = Account::balance(host, &mut tx, &smart_function).unwrap();
        let source_balance_before = Account::balance(host, &mut tx, &source).unwrap();

        tx.commit(host).unwrap();

        // Calling the smart function should error or return an error response
        tx.begin();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
            method: Method::GET,
            headers: Default::default(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let result = execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            Blake2b::from(b"fake_op_hash".as_ref()),
        )
        .await;
        let sf_balance_after = Account::balance(host, &mut tx, &smart_function).unwrap();
        let source_balance_after = Account::balance(host, &mut tx, &source).unwrap();

        assert_eq!(sf_balance_before, sf_balance_after);
        assert_eq!(source_balance_before, source_balance_after);

        let call_failed = match result {
            Ok(receipt) => receipt.status_code.is_server_error(),
            _ => true,
        };
        assert!(call_failed);
    }

    #[tokio::test]
    async fn transfer_xtz_and_smart_function_call_is_atomic1() {
        let invalid_code = r#"
        const handler = () => {{
            invalid();
        }};
        export default handler;
        "#;
        transfer_xtz_and_run_erroneous_sf(invalid_code, 500).await;
    }

    #[tokio::test]
    async fn transfer_xtz_and_smart_function_call_is_atomic2() {
        let invalid_code = r#"
        const handler = () => {{
             return Response.error("error!");
        }};
        export default handler;
        "#;
        transfer_xtz_and_run_erroneous_sf(invalid_code, 400).await;
    }

    #[tokio::test]
    async fn transfer_xtz_and_smart_function_call_is_atomic3() {
        let invalid_code = r#"
        const handler = () => {{
            return 3;
        }};
        export default handler;
        "#;
        transfer_xtz_and_run_erroneous_sf(invalid_code, 500).await;
    }

    async fn transfer_xtz_and_run_erroneous_sf(code: &str, expected_status_code: u16) {
        let source = Address::User(jstz_mock::account1());
        // 1. Deploy the smart function
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_balance = 1;
        tx.begin();
        Account::add_balance(host, &mut tx, &source, initial_balance)
            .expect("add balance");
        tx.commit(host).unwrap();

        // 1. Deploy smart function
        let parsed_code = code.to_string();
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        tx.commit(host).unwrap();

        // Calling the smart function should error or return an error response
        tx.begin();
        let mut headers = HeaderMap::new();
        headers.insert(
            X_JSTZ_TRANSFER,
            initial_balance.to_string().try_into().unwrap(),
        );
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
            method: Method::GET,
            headers,
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let result = execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            Blake2b::from(b"fake_op_hash".as_ref()),
        )
        .await;
        let call_failed = match result {
            Ok(receipt) => receipt.status_code == expected_status_code,
            _ => true,
        };
        assert!(call_failed);
        // The balance should not be affected
        assert_eq!(
            Account::balance(host, &mut tx, &source).unwrap(),
            initial_balance
        );
        let balance_after = Account::balance(host, &mut tx, &smart_function).unwrap();
        assert_eq!(balance_after, 0);
    }

    // TODO: https://linear.app/tezos/issue/JSTZ-655/support-hostscript-fa-withdraw-in-v2
    // v2 runtime does not support HostScript withdrawals yet
    #[cfg(not(feature = "v2_runtime"))]
    #[tokio::test]
    async fn host_script_withdraw_from_smart_function_succeeds() {
        let mut mock_host = JstzMockHost::default();
        let host = mock_host.rt();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let code = r#"
        export default (request) => {
            const withdrawRequest = new Request("jstz://jstz/withdraw", {
                method: "POST",
                headers: {
                    "Content-type": "application/json",
                },
                body: JSON.stringify({
                    receiver: "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx",
                    amount: 5,
                }),
            });
            return SmartFunction.call(withdrawRequest);
        }
        "#;
        let parsed_code = code.to_string();
        tx.begin();
        Account::add_balance(host, &mut tx, &source, 1000).unwrap();
        let smart_function = smart_function::deploy::deploy_smart_function(
            host,
            &mut tx,
            &source,
            parsed_code,
            5,
        )
        .unwrap();
        tx.commit(host).unwrap();

        tx.begin();
        let run_function = RunFunction {
            uri: format!("jstz://{smart_function}/").try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash,
        )
        .await
        .expect("Withdrawal expected to succeed");
        tx.commit(host).unwrap();

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(1, outbox.len());

        // Trying to withdraw again should fail with insufficient funds
        tx.begin();
        let fake_op_hash2 = Blake2b::from(b"fake_op_hash2".as_ref());
        let error = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function,
            fake_op_hash2,
        )
        .await
        .expect_err("Expected error");
        assert_eq!("EvalError: InsufficientFunds", error.to_string());
    }

    #[tokio::test]
    async fn transfer_xtz_from_smart_function_succeeds() {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let transfer_amount: u64 = 1;

        let smart_function1 = deploy_transfer_sf_and_execute(
            source.clone(),
            host,
            &mut tx,
            transfer_amount,
        )
        .await;
        // deploy a new smart function that transfers balance to a smart function address
        let code2 = format!(
            r#"
            const handler = async () => {{
                const myHeaders = new Headers();
                myHeaders.append("X-JSTZ-TRANSFER", "{transfer_amount}");
                await fetch(new Request("jstz://{smart_function1}/", {{
                    headers: myHeaders
                }}));
                return new Response();
            }};
            export default handler;
            "#
        );
        let parsed_code2 = code2.to_string();
        tx.begin();
        let smart_function2 =
            smart_function::deploy(host, &mut tx, &source, parsed_code2, transfer_amount)
                .unwrap();

        // 6. Call the new smart function
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function2).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash2 = Blake2b::from(b"fake_op_hash2".as_ref());
        let source_before = Account::balance(host, &mut tx, &source).unwrap();
        smart_function::run::execute(host, &mut tx, &source, run_function, fake_op_hash2)
            .await
            .unwrap();
        tx.commit(host).unwrap();
        tx.begin();
        let source_after = Account::balance(host, &mut tx, &source).unwrap();
        // 7. Assert sf2 transferred to sf1
        assert_eq!(
            Account::balance(host, &mut tx, &smart_function2).unwrap(),
            0
        );
        assert_eq!(source_after - source_before, transfer_amount);
    }
    #[tokio::test]
    async fn transfer_xtz_from_smart_function_succeeds_with_noop() {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx: Transaction = Transaction::default();
        let transfer_amount: u64 = 1;
        tx.begin();
        tx.commit(host).unwrap();
        // deploy and execute smart function that transfers `transfer_amount` to the `source`
        let smart_function = deploy_transfer_sf_and_execute(
            source.clone(),
            host,
            &mut tx,
            transfer_amount,
        )
        .await;

        // deploy a new smart function that transfers balance to a smart function address
        // without executing the sf using /-/noop path
        let code2 = format!(
            r#"
            const handler = async () => {{
                const myHeaders = new Headers();
                myHeaders.append("X-JSTZ-TRANSFER", "{transfer_amount}");
                await fetch(new Request("jstz://{smart_function}/-/noop", {{
                    headers: myHeaders
                }}));
                return new Response();
            }};
            export default handler;
            "#
        );
        let parsed_code2 = code2.to_string();
        tx.begin();
        let smart_function2 =
            smart_function::deploy(host, &mut tx, &source, parsed_code2, transfer_amount)
                .unwrap();

        // calling the smart function2
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function2).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash2 = Blake2b::from(b"fake_op_hash2".as_ref());
        let source_before = Account::balance(host, &mut tx, &source).unwrap();
        let sf2_before = Account::balance(host, &mut tx, &smart_function2).unwrap();
        smart_function::run::execute(host, &mut tx, &source, run_function, fake_op_hash2)
            .await
            .unwrap();
        tx.commit(host).unwrap();
        // the source shouldn't received balance as sf1 isn't executed
        tx.begin();
        let source_after = Account::balance(host, &mut tx, &source).unwrap();
        let sf2_after = Account::balance(host, &mut tx, &smart_function2).unwrap();
        assert_eq!(source_after, source_before);
        assert_eq!(sf2_before - sf2_after, transfer_amount);
    }

    // deploy a smart function that transfers `transfer_amount` to the `source`
    // and executes it. returns the executed smart function address
    async fn deploy_transfer_sf_and_execute(
        source: Address,
        host: &mut MockHost,
        tx: &mut Transaction,
        transfer_amount: u64,
    ) -> SmartFunctionHash {
        let initial_sf_balance: u64 = 1_028_230_587 * 1_000_000;
        tx.begin();
        Account::add_balance(host, tx, &source, initial_sf_balance).unwrap();
        tx.commit(host).unwrap();

        // 1. Deploy the smart function that transfers the balance to user address
        let code = format!(
            r#"
            const handler = async () => {{
                const myHeaders = new Headers();
                myHeaders.append("X-JSTZ-TRANSFER", "{transfer_amount}");
                await fetch(new Request("jstz://{source}", {{
                    headers: myHeaders
                }}));
                return new Response();
            }};
            export default handler;
            "#
        );
        let parsed_code = code.to_string();
        tx.begin();
        let smart_function = smart_function::deploy(
            host,
            tx,
            &source,
            parsed_code.clone(),
            initial_sf_balance,
        )
        .unwrap();

        let balance_before = Account::balance(host, tx, &smart_function).unwrap();
        assert_eq!(balance_before, initial_sf_balance);

        tx.commit(host).unwrap();

        // 2. Call the smart function
        tx.begin();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        smart_function::run::execute(
            host,
            tx,
            &source,
            run_function.clone(),
            fake_op_hash,
        )
        .await
        .expect("run function expected");
        tx.commit(host).unwrap();

        // 3. Assert the transfer from the smart function to the user address
        tx.begin();
        let balance_after = Account::balance(host, tx, &smart_function).unwrap();
        assert_eq!(balance_before - balance_after, transfer_amount);
        assert_eq!(
            Account::balance(host, tx, &source).unwrap(),
            transfer_amount
        );
        tx.commit(host).unwrap();
        smart_function
    }

    #[tokio::test]
    async fn failure_on_transfer_xtz_from_smart_function_returns_error_response() {
        let source = Address::User(jstz_mock::account2());
        // 1. Deploy the smart function
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();

        // 2. Deploy the smart function that transfers the balance to the source
        let code = format!(
            r#"
            const handler = async () => {{
                const myHeaders = new Headers();
                myHeaders.append("X-JSTZ-TRANSFER", "1");
                return await fetch(new Request("jstz://{source}", {{
                    headers: myHeaders
                }}));
            }};
            export default handler;
            "#
        );
        let parsed_code = code.to_string();
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        tx.commit(host).unwrap();

        // 3. Calling the smart function with insufficient funds should result in an error response
        tx.begin();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        let receipt = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash,
        )
        .await
        .expect("run function expected receipt");

        assert!(receipt.status_code.is_server_error());
    }

    #[tokio::test]
    async fn smart_function_refund_can_propagate() {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_caller_sf_balance: u64 = 0;
        let initial_refund_sf_balance: u64 = 1;
        tx.begin();

        Account::add_balance(host, &mut tx, &source, initial_refund_sf_balance).unwrap();

        // 1. Deploy the smart function that refunds to the caller
        let refund_amount = 1;
        let refund_code = format!(
            r#"
            const handler = () => {{
                return new Response(null, {{
                    headers: {{ "X-JSTZ-TRANSFER": "{refund_amount}" }},
                }});
            }};
            export default handler;
            "#
        );
        let parsed_code = refund_code.to_string();
        let refund_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_refund_sf_balance,
        )
        .unwrap();

        // 2. deploy a smart function that calls the refund smart function and propagates the response
        let code = format!(
            r#"
            const handler = async() => {{
                const response = await fetch(new Request("jstz://{refund_sf}"));
                const refunded = response.headers.get("X-JSTZ-AMOUNT");
                // propagate the refunded amount to the caller
                return new Response(null, {{
                    headers: {{ "X-JSTZ-TRANSFER": refunded }},
                }});
            }};
            export default handler;
            "#
        );
        let parsed_code = code.to_string();
        let caller_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_caller_sf_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the caller smart function
        tx.begin();
        let balance_before_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_before_source = Account::balance(host, &mut tx, &source).unwrap();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &caller_sf).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .await
        .expect("run function expected");
        let balance_after_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_after_source = Account::balance(host, &mut tx, &source).unwrap();
        tx.commit(host).unwrap();

        // 4. Assert the refund is propagated to the source instead of the caller_sf
        assert_eq!(balance_before_caller, balance_after_caller);
        assert_eq!(balance_before_source + refund_amount, balance_after_source);
    }

    // TODO: https://linear.app/tezos/issue/JSTZ-656/v2-fetch-should-fail-no-invalid-headers
    // v2 runtime fetch ignores invalid headers instead of failing. It should fail
    #[cfg(not(feature = "v2_runtime"))]
    #[tokio::test]
    async fn propagating_smart_function_refund_fails() {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_caller_sf_balance: u64 = 0;
        let initial_refund_sf_balance: u64 = 1;
        tx.begin();

        Account::add_balance(host, &mut tx, &source, initial_refund_sf_balance).unwrap();

        // 1. Deploy the smart function that refunds to the caller
        let refund_amount = 1;
        let refund_code = format!(
            r#"
            const handler = () => {{
                return new Response(null, {{
                    headers: {{ "X-JSTZ-TRANSFER": "{refund_amount}" }},
                }});
            }};
            export default handler;
            "#
        );
        let parsed_code = refund_code.to_string();
        let refund_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_refund_sf_balance,
        )
        .unwrap();

        // 2. deploy a smart function that calls the refund smart function
        //    and returns the response
        let code = format!(
            r#"
            const handler = () => {{
                return fetch(new Request("jstz://{refund_sf}"));
            }};
            export default handler;
            "#
        );
        let parsed_code = code.to_string();
        let caller_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_caller_sf_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the caller smart function
        tx.begin();
        let balance_before_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_before_source = Account::balance(host, &mut tx, &source).unwrap();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &caller_sf).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        let result = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .await
        .unwrap();
        assert!(result.status_code.is_server_error());

        tx.commit(host).unwrap();

        tx.begin();
        let balance_after_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_after_source = Account::balance(host, &mut tx, &source).unwrap();

        // 4. Assert the refund is not propagated to the source
        assert_eq!(balance_before_caller, balance_after_caller);
        assert_eq!(balance_before_source, balance_after_source);
    }

    #[tokio::test]
    async fn returning_invalid_refund_amount_in_response_fails() {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_caller_sf_balance: u64 = 0;
        let initial_refund_sf_balance: u64 = 1;
        tx.begin();

        Account::add_balance(host, &mut tx, &source, initial_refund_sf_balance).unwrap();

        // 1. Deploy the smart function that refunds to the caller
        let refund_amount = 1;
        let invalid_refund_code = format!(
            r#"
            const handler = () => {{
                return new Response(null, {{
                    headers: {{ "X-JSTZ-AMOUNT": "{refund_amount}" }},
                }});
            }};
            export default handler;
            "#
        );
        let parsed_code = invalid_refund_code.to_string();
        let fake_refund_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_refund_sf_balance,
        )
        .unwrap();

        // 2. deploy a smart function that calls the refund smart function
        //    and returns the response
        let code = format!(
            r#"
            const handler = async () => {{
                await fetch(new Request("jstz://{fake_refund_sf}"));
                return new Response();
            }};
            export default handler;
            "#
        );
        let parsed_code = code.to_string();
        let caller_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_caller_sf_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the caller smart function
        tx.begin();
        let balance_before_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_before_source = Account::balance(host, &mut tx, &source).unwrap();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &caller_sf).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .await
        .unwrap();
        tx.commit(host).unwrap();

        tx.begin();
        let balance_after_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_after_source = Account::balance(host, &mut tx, &source).unwrap();

        // 4. Assert the refund is not propagated to the source
        assert_eq!(balance_before_caller, balance_after_caller);
        assert_eq!(balance_before_source, balance_after_source);
    }

    // TODO: https://linear.app/tezos/issue/JSTZ-656/v2-fetch-should-fail-no-invalid-headers
    // v2 runtime fetch ignores invalid headers instead of failing. It should fail
    #[cfg(not(feature = "v2_runtime"))]
    #[tokio::test]
    async fn returning_invalid_request_amount_fails() {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_caller_sf_balance: u64 = 1;
        let initial_refund_sf_balance: u64 = 1;
        tx.begin();

        Account::add_balance(
            host,
            &mut tx,
            &source,
            initial_refund_sf_balance + initial_caller_sf_balance,
        )
        .unwrap();

        // 1. Deploy the smart function that refunds to the caller
        let refund_code = r#"
            const handler = () => {
                return new Response(null, {
                    headers: { "X-JSTZ-TRANSFER": "1" },
                });
            };
            export default handler;
            "#;
        let parsed_code = refund_code.to_string();
        let fake_refund_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_refund_sf_balance,
        )
        .unwrap();

        // 2. deploy a smart function that calls the refund smart function
        //    and returns the response
        let invalid_request_amount_code = format!(
            r#"
            const handler = async () => {{
                const myHeaders = new Headers();
                myHeaders.append("X-JSTZ-AMOUNT", "{initial_caller_sf_balance}");
                return fetch(new Request("jstz://{fake_refund_sf}/", {{
                    headers: myHeaders
                }}));
            }};
            export default handler;
            "#
        );
        let caller_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            invalid_request_amount_code.to_string(),
            initial_caller_sf_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the caller smart function
        tx.begin();
        let balance_before_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_before_source = Account::balance(host, &mut tx, &source).unwrap();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &caller_sf).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        let result = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .await
        .unwrap();
        assert!(result.status_code.is_server_error());
        tx.commit(host).unwrap();

        tx.begin();
        let balance_after_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_after_source = Account::balance(host, &mut tx, &source).unwrap();

        // // 4. Assert the refund is not propagated to the source
        assert_eq!(balance_before_caller, balance_after_caller);
        assert_eq!(balance_before_source, balance_after_source);
    }

    #[tokio::test]
    async fn smart_function_refunds_succeeds() {
        let refund_amount = 1;
        let refund_code = format!(
            r#"
            const handler = () => {{
                return new Response(null, {{
                    headers: {{ "X-JSTZ-TRANSFER": "{refund_amount}" }},
                }});
            }};
            export default handler;
            "#
        );
        test_smart_function_refund(refund_code, refund_amount).await;
    }

    #[tokio::test]
    async fn smart_function_refunds_succeeds_async() {
        let refund_amount = 1;
        let refund_code = format!(
            r#"
            const handler = async () => {{
                return new Response(null, {{
                    headers: {{ "X-JSTZ-TRANSFER": "{refund_amount}" }},
                }});
            }};
            export default handler;
            "#
        );
        test_smart_function_refund(refund_code, refund_amount).await;
    }

    async fn test_smart_function_refund(refund_code: String, refund_amount: u64) {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_caller_sf_balance: u64 = 0;
        let initial_refund_sf_balance: u64 = 1;
        tx.begin();

        Account::add_balance(host, &mut tx, &source, initial_refund_sf_balance).unwrap();

        // 1. Deploy the smart function that refunds to the caller
        let parsed_code = refund_code.to_string();
        let refund_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_refund_sf_balance,
        )
        .unwrap();

        // 2. deploy a smart function that calls the refund smart function
        let code = format!(
            r#"
            const handler = async () => {{
                const response = await fetch(new Request("jstz://{refund_sf}"));
                if (response.ok) {{
                    return new Response();
                }} else {{
                    return Response.error();
                }}  
            }};
            export default handler;
            "#
        );
        let parsed_code = code.to_string();
        let caller_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_caller_sf_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the caller smart function
        tx.begin();
        let balance_before = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &caller_sf).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .await
        .expect("run function expected");
        let balance_after = Account::balance(host, &mut tx, &caller_sf).unwrap();
        tx.commit(host).unwrap();

        // 4. Assert the refund from the refund smart function to the caller
        assert_eq!(balance_before + refund_amount, balance_after);

        // 5. Calling the transaction again results in an error and a tx rollback
        tx.begin();
        let transfer_amount = 1;
        Account::add_balance(host, &mut tx, &source, transfer_amount).unwrap();
        let balance_before = Account::balance(host, &mut tx, &source).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            X_JSTZ_TRANSFER,
            transfer_amount.to_string().try_into().unwrap(),
        );
        let result = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            RunFunction {
                headers,
                ..run_function
            },
            fake_op_hash,
        )
        .await
        .unwrap();
        assert_eq!(result.status_code, 400);

        let balance_after = Account::balance(host, &mut tx, &source).unwrap();
        assert_eq!(balance_before, balance_after);
    }

    // TODO: https://linear.app/tezos/issue/JSTZ-655/support-hostscript-fa-withdraw-in-v2
    // v2 runtime does not support HostScript withdrawals yet
    #[cfg(not(feature = "v2_runtime"))]
    #[tokio::test]
    async fn host_script_fa_withdraw_from_smart_function_succeeds() {
        let receiver = Address::User(jstz_mock::account1());
        let source = Address::User(jstz_mock::account2());
        let ticketer = jstz_mock::kt1_account1();
        let ticketer_string = ticketer.clone();
        let l1_proxy_contract = ticketer.clone();

        let ticket_id = 1234;
        let ticket_content = b"random ticket content".to_vec();
        let json_ticket_content = json!(&ticket_content);
        assert_eq!("[114,97,110,100,111,109,32,116,105,99,107,101,116,32,99,111,110,116,101,110,116]", format!("{json_ticket_content}"));
        let ticket =
            jstz_mock::parse_ticket(ticketer, 1, (ticket_id, Some(ticket_content)));
        let ticket_hash = ticket.hash().unwrap();
        let token_smart_function_initial_ticket_balance = 100;
        let withdraw_amount = 90;
        let mut jstz_mock_host = JstzMockHost::default();

        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();

        // 1. Deploy our "token contract"
        tx.begin();
        let token_contract_code = format!(
            r#"
                export default (request) => {{
                    const url = new URL(request.url)
                    if (url.pathname === "/withdraw") {{
                        const withdrawRequest = new Request("jstz://jstz/fa-withdraw", {{
                            method: "POST",
                            headers: {{
                                "Content-type": "application/json",
                            }},
                            body: JSON.stringify({{
                                amount: {withdraw_amount},
                                routingInfo: {{
                                    receiver: "{receiver}",
                                    proxyL1Contract: "{l1_proxy_contract}"
                                }},
                                ticketInfo: {{
                                    id: {ticket_id},
                                    content: {json_ticket_content},
                                    ticketer: "{ticketer_string}"
                                }}
                            }}),
                        }});
                        return SmartFunction.call(withdrawRequest);
                    }}
                    else {{
                        return Response.error();
                    }}

                }}
            "#,
        );
        let parsed_code = token_contract_code.to_string();
        let token_smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        // 2. Add its ticket blance
        TicketTable::add(
            host,
            &mut tx,
            &token_smart_function,
            &ticket_hash,
            token_smart_function_initial_ticket_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the smart function
        tx.begin();
        let run_function = RunFunction {
            uri: format!("jstz://{}/withdraw", &token_smart_function)
                .try_into()
                .unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash,
        )
        .await
        .expect("Fa withdraw expected");

        tx.commit(host).unwrap();

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(1, outbox.len());
        tx.begin();
        let balance = TicketTable::get_balance(
            host,
            &mut tx,
            &Address::SmartFunction(token_smart_function),
            &ticket_hash,
        )
        .unwrap();
        assert_eq!(10, balance);

        // Trying a second fa withdraw should fail with insufficient funds
        tx.begin();
        let fake_op_hash2 = Blake2b::from(b"fake_op_hash2".as_ref());
        let error = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function,
            fake_op_hash2,
        )
        .await
        .expect_err("Expected error");
        assert_eq!(
            "EvalError: TicketTableError: InsufficientFunds",
            error.to_string()
        );
    }

    #[cfg(feature = "v2_runtime")]
    #[tokio::test]
    async fn execute_v2_runtime() {
        let source = Address::User(jstz_mock::account1());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();

        // This smart function uses FormData, which is not supported in v1 runtime but in v2 runtime.
        let code = format!(
            r#"
        const handler = async (request) => {{
            const f = new FormData();
            f.append("a", "b");
            f.append("c", "d");
            let output = "";
            for (const [k, v] of f) {{
                output += `${{k}}-${{v}};`;
            }}
            return new Response(output);
        }};
        export default handler;
        "#
        );
        let parsed_code = code.to_string();
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        tx.commit(host).unwrap();

        // call smart function; should get an ok response.
        tx.begin();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        let response = super::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .await
        .expect("run function expected");
        tx.commit(host).unwrap();

        let text = String::from_utf8(response.body.unwrap()).unwrap();
        assert_eq!(text, "a-b;c-d;");
        assert_eq!(response.status_code, http::StatusCode::OK);
    }

    #[cfg(feature = "v2_runtime")]
    #[tokio::test]
    async fn handles_infinite_recursion() {
        let source = Address::User(jstz_mock::account1());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();

        // This smart function recursively calls itself given a body of its own address
        let code = format!(
            r#"
        async function handler(req) {{
            const k = await req.text();
            return await fetch(`jstz://${{k}}/`, {{body: k, method: "POST"}});
        }}
        export default handler;
        "#
        );
        let parsed_code = code.to_string();
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        tx.commit(host).unwrap();

        tx.begin();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
            method: Method::POST,
            headers: HeaderMap::new(),
            body: HttpBody::from_string(smart_function.to_base58()),
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        let response = super::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .await
        .expect("run function expected");
        tx.commit(host).unwrap();

        let text = String::from_utf8(response.body.unwrap()).unwrap();
        assert!(text.contains("Too many smart function calls (max: 5)"));
        assert_eq!(
            response.status_code,
            http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
