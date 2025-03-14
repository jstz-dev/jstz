use jstz_core::{
    host::{HostRuntime, JsHostRuntime},
    kv::Transaction,
};
use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
use jstz_runtime::{runtime::JstzRuntimeOptions, JstzRuntime};

use crate::api::Kv;

/// Initializes a new [`JstzRuntime`] with the given protocol
pub fn init_jstz_runtime(protocol: Protocol) -> JstzRuntime {
    JstzRuntime::init(JstzRuntimeOptions {
        protocol: Some(protocol),
        ..Default::default()
    })
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
