use console::jstz_console;
use deno_console::deno_console;
use jstz_core::{
    host::{HostRuntime, JsHostRuntime},
    kv::Transaction,
};
use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
use jstz_runtime::{init_ops_and_esm_extensions, JstzRuntime};
use kv::jstz_kv;

use crate::api::Kv;

mod console;
mod kv;

/// Initializes a new [`JstzRuntime`] with the given protocol
pub fn init_jstz_runtime(protocol: Protocol) -> JstzRuntime {
    let extensions = init_ops_and_esm_extensions!(deno_console, jstz_console, jstz_kv);
    JstzRuntime::init(extensions, Some(protocol))
}

/// [`Protocol`] exposes proto-specific state to [`JstzRuntime`]
pub struct Protocol {
    pub host: JsHostRuntime<'static>,
    pub tx: &'static mut Transaction,
    pub kv: Kv,
}

impl Protocol {
    pub fn new(
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        address: SmartFunctionHash,
    ) -> Self {
        let host = JsHostRuntime::new(hrt);
        // Safety: Since we synchronisely execute Operatios, the tx will not be dropped before
        // the runtime, so this is safe
        let tx = unsafe { std::mem::transmute(tx) };
        let kv = Kv::new(address.to_base58());
        Protocol { host, tx, kv }
    }
}

#[cfg(test)]
mod test_utils {

    use tezos_smart_rollup_mock::MockHost;

    #[allow(unused)]
    pub fn init_mock_host() -> (Box<Vec<u8>>, MockHost) {
        let mut sink: Box<Vec<u8>> = Box::default();
        let mut host = MockHost::default();
        host.set_debug_handler(unsafe {
            std::mem::transmute::<&mut std::vec::Vec<u8>, &'static mut Vec<u8>>(
                sink.as_mut(),
            )
        });

        (sink, host)
    }
}

#[cfg(test)]
mod test {
    use jstz_core::kv::Transaction;
    use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};

    use crate::runtime::Protocol;

    use super::{init_jstz_runtime, test_utils::init_mock_host};

    #[test]
    fn test_init_jstz_runtime() {
        let address =
            SmartFunctionHash::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton")
                .unwrap();
        let (sink, mut host) = init_mock_host();
        let tx = &mut Transaction::default();
        tx.begin();
        let mut jstz_runtime = init_jstz_runtime(Protocol::new(&mut host, tx, address));

        let code = r#"
        KV.set("hello", "world");
        KV.set("abc", 42);
        let hello = KV.get("hello");
        console.log(hello);
        let abc = KV.get("abc");
        console.log(42);
        42 + 8
    "#;

        let result = jstz_runtime.execute_with_result::<u32>(code).unwrap();
        assert_eq!(result, 50);
        assert_eq!(
            String::from_utf8_lossy(&sink),
            "[INFO] world\n[INFO] \u{1b}[33m42\u{1b}[39m\n"
        )
    }
}
