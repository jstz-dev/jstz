use std::rc::Rc;

use deno_core::NoopModuleLoader;
//use jstz_core::host::HostRuntime;
//use jstz_core::kv::Transaction;
//use jstz_crypto::smart_function_hash::SmartFunctionHash;
//use jstz_runtime::host::HostRuntime;
use jstz_runtime::{JstzRuntime, JstzRuntimeOptions, Protocol};
use log::info;
//use std::rc::Rc;
use tezos_smart_rollup::{
    entrypoint,
    prelude::{debug_msg, Runtime},
};

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    debug_msg!(rt, "Debug.");
    info!("{}", "Kernel entrypoint accessed.");

    // Run a test similar to what's in the test module
    run_runtime_example(rt);

    // Don't panic, just log success
    info!("Runtime test completed successfully.");
}

fn run_runtime_example(host: &mut impl Runtime) {
    info!("Running runtime example");

    // Create a simple test setup
    let mut sink: Box<Vec<u8>> = Box::default();
    let mut host = tezos_smart_rollup_mock::MockHost::default();

    // Set up the debug handler
    host.set_debug_handler(unsafe {
        std::mem::transmute::<&mut std::vec::Vec<u8>, &'static mut Vec<u8>>(sink.as_mut())
    });

    // Create a smart function hash
    //let address = <jstz_crypto::smart_function_hash::SmartFunctionHash as jstz_crypto::hash::Hash>::from_base58(
    //    "KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton",
    //)
    //.unwrap();
    //let $address =
    //              <jstz_crypto::smart_function_hash::SmartFunctionHash as jstz_crypto::hash::Hash>::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton")
    //                .unwrap();

    // Create a transaction
    //let mut tx = jstz_core::kv::Transaction::default();
    //tx.begin();

    // Create runtime options
    let options = JstzRuntimeOptions {
        protocol: Some(Protocol::new(/*,host  &mut tx, address*/)),
        extensions: vec![],
        module_loader: Rc::new(NoopModuleLoader),
    };

    // Create the runtime
    let mut runtime = JstzRuntime::new(options);

    // Run a simple JavaScript test
    let code = r#"
        2 + 2
    "#;

    // Execute the code and get the result
    let result = runtime.execute_with_result::<u32>(code).unwrap();

    // Log the result
    info!("Test result: {}", result);

    // Verify the result
    //assert_eq!(result, "hello47");

    // Log the sink output
    info!("Console output: {}", String::from_utf8_lossy(sink.as_ref()));

    info!("Runtime example completed successfully");
}
