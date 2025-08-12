#![cfg(feature = "riscv_wpt_test_kernel")]
use tezos_smart_rollup::{entrypoint, host::Runtime, prelude::debug_msg};

use crate::inbox::*;
use deno_core::{
    ascii_str, extension, Extension, ExtensionFileSource, ExtensionFileSourceCode,
};
use deno_core::{
    convert::Smi,
    op2,
    v8::{self},
    FromV8, OpState,
};
use deno_error::JsErrorBox;
use derive_more::{From, Into};
use jstz_core::{host::HostRuntime, kv::Transaction, BinEncodable};
use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
use jstz_runtime::runtime::JstzPermissions;
use jstz_runtime::wpt::{TestHarnessReport, TestResult, TestsResult};
use jstz_runtime::{JstzRuntime, JstzRuntimeOptions, RuntimeContext};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_crypto_rs::hash::SmartRollupHash;
use tezos_smart_rollup_mock::MockHost;

// Add imports for inbox and operation types
use jstz_proto::operation::{Content, Operation};

#[op2]
pub fn test_result_callback(op_state: &mut OpState, #[from_v8] result: TestResult) {
    let report: &mut TestHarnessReport = op_state.borrow_mut::<TestHarnessReport>();
    report.add_test_result(result);
}

#[op2]
pub fn test_completion_callback(
    op_state: &mut OpState,
    _tests: &v8::Value,
    #[from_v8] result: TestsResult,
    _records: &v8::Value,
) -> Result<(), JsErrorBox> {
    let report: &mut TestHarnessReport = op_state.borrow_mut::<TestHarnessReport>();
    report
        .set_harness_result(result)
        .map_err(|e| JsErrorBox::generic(e.to_string()))
}

use deno_core::url::Url;
use deno_web::BlobStore;
use std::sync::Arc;

deno_core::extension!(
    test_harness_api,
    ops = [test_completion_callback, test_result_callback],
    esm_entry_point = "ext:test_harness_api/test_harness_api.js",
    esm = [dir "../jstz_runtime/tests", "test_harness_api.js"],
);

// -------------- TimersPermission --------------
// For tests you can grant everything unconditionally

fn init_runtime(host: &mut impl HostRuntime, tx: &mut Transaction) -> JstzRuntime {
    // -------------- BlobStore --------------
    // Required by deno_web for createObjectURL().
    let blob_store = Arc::new(BlobStore::default());

    // Optional "location" of the global scope.
    // Many runtimes pass something like "file:///main.js".
    let maybe_location = Url::parse("file:///").ok();

    let address =
        SmartFunctionHash::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton").unwrap();

    let mut runtime = JstzRuntime::new(JstzRuntimeOptions {
        protocol: Some(RuntimeContext::new(host, tx, address, String::new())),
        extensions: vec![
            //deno_broadcast_channel::deno_broadcast_channel::init_ops_and_esm(),
            //test_harness_op_ext::init_ops_and_esm(),
            //test_harness_js_ext(),
            test_harness_api::init_ops_and_esm(),
        ],
        ..Default::default()
    });

    let op_state = runtime.op_state();
    // Insert a blank report to be filled in by test cases
    op_state.borrow_mut().put(TestHarnessReport::default());

    runtime
}

fn read_message(
    rt: &mut impl Runtime,
    ticketer: &ContractKt1Hash,
) -> Option<ParsedInboxMessage> {
    //debug_msg!(rt, "reading message from inbox\n");
    //let input = rt.read_input().ok()??;
    let input = match rt.read_input() {
        Ok(input) => match input {
            Some(input) => {
                //debug_msg!(rt, "input: {:?}\n", input);
                input
            }
            None => {
                //debug_msg!(rt, "no input found\n");
                return None;
            }
        },
        Err(e) => {
            //debug_msg!(rt, "error reading input: {:?}\n", e);
            return None;
        }
    };
    //let jstz_rollup_address = rt.reveal_metadata().address();
    let jstz_rollup_address =
        SmartRollupHash::from_base58_check("sr1BxufbqiHt3dn6ahV6eZk9xBD6XV1fYowr")
            .unwrap();
    //debug_msg!(rt, "parsed inbox message\n");
    parse_inbox_message(rt, input.id, input.as_ref(), ticketer, &jstz_rollup_address)
}

// kernel entry
pub fn entry(rt: &mut impl Runtime) {
    //debug_msg!(rt, "Starting Jstz WPT test kernel\n");

    let mut tx = Transaction::default();
    tx.begin();
    let mut host = MockHost::default();
    host.set_debug_handler(std::io::empty());

    // Try to read messages from the inbox until we find a JstzMessage with External operation
    let mut source = String::new();
    let ticketer =
        SmartFunctionHash::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton").unwrap();

    //debug_msg!(rt, "starting to read messages from inbox\n");

    // Keep reading messages until we find the right one
    loop {
        match read_message(rt, &ticketer) {
            Some(message) => {
                match message {
                    ParsedInboxMessage::JstzMessage(Message::External(
                        signed_operation,
                    )) => {
                        // Check if this is a DeployFunction operation
                        let operation: Operation = signed_operation.into();
                        match operation.content {
                            Content::DeployFunction(deploy_function) => {
                                if deploy_function.function_code.to_string() == "STOP" {
                                    //debug_msg!(rt, "STOP message found, exiting loop\n");
                                    break;
                                }
                                // Extract the source code from the DeployFunction
                                source += &deploy_function.function_code.to_string();
                                /*debug_msg!(
                                    rt,
                                    "Found DeployFunction message with source code\n"
                                );*/
                                //break; // Found what we're looking for, exit the loop
                            }
                            _ => {
                                /*debug_msg!(
                                    rt,
                                    "Message is not a DeployFunction, continuing to next message\n"
                                );*/
                                // Continue reading next message
                            }
                        }
                    }
                    ParsedInboxMessage::LevelInfo(level_info_type) => {
                        /*debug_msg!(
                            rt,
                            "LevelInfo message found, continuing to next message\n"
                        );*/
                        // Continue reading next message
                    }
                    _ => {
                        /*debug_msg!(
                            rt,
                            "Some other message found, continuing to next message\n"
                        );*/
                        // Continue reading next message
                    }
                }
            }
            None => {
                //debug_msg!(rt, "no message found\n");
                break;
            }
        }
    }

    //debug_msg!(rt, "{}", format!("source: {}", source));

    // If no suitable message was found, use default source
    if source.is_empty() {
        debug_msg!(
            rt,
            "No suitable message found in inbox, using default source\n"
        );
        source = "console.log('hello');".to_string();
    }

    //println!("source: {}", source);
    //eprintln!("source: {}", source);

    let mut js_rt = init_runtime(&mut host, &mut tx);

    // Somehow each `execute_script` call has some strange side effect such that the global
    // test suite object is completed prematurely before all test cases are registered.
    // Therefore, instead of executing each piece of test scripts separately, we need to
    // collect them and run them all in one `execute_script` call.
    //debug_msg!(rt, "executing script");

    // Use catch_unwind to handle panics (including segmentation faults) gracefully
    let result = js_rt.execute_script("native code", source);

    match result {
        Ok(_) => {
            debug_msg!(rt, "script executed successfully ");
        }
        Err(e) => {
            debug_msg!(
                rt,
                "{}",
                format!("script execution failed with panic: {:?}", e)
            );
            // Return a default report indicating the test failed due to execution error
            /*return TestHarnessReport {
                status: Some(WptTestStatus::Err),
                subtests: vec![WptSubtest {
                    name: "Script execution failed".to_string(),
                    status: WptSubtestStatus::Fail,
                    message: Some(
                        "Test failed due to script execution error (panic/segfault)"
                            .to_string(),
                    ),
                }],
            };*/
        }
    };

    //debug_msg!(rt, "script executed");
    // Take the test harness report out of the runtime and return it
    // Need to store data temporarily so that the borrow can be dropped
    let data = js_rt
        .op_state()
        .borrow()
        .borrow::<TestHarnessReport>()
        .clone();
    debug_msg!(rt, "{}", format!("data: {:?}", data));

    //run_wpt_tests().await?;
    //jstz_kernel::riscv_kernel::run(rt);
}
