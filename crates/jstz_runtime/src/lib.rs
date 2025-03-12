pub mod error;
mod ext;
pub mod runtime;

pub use runtime::{JstzRuntime, JstzRuntimeOptions, Protocol};

#[cfg(test)]
mod test_utils {

    use std::fmt::Display;

    use tezos_smart_rollup_mock::MockHost;

    #[allow(clippy::box_collection)]
    pub struct Sink(pub Box<Vec<u8>>);

    impl Sink {
        #[allow(unused)]
        pub fn lines(&self) -> Vec<String> {
            self.to_string()
                .split("\n")
                .map(ToString::to_string)
                .collect()
        }
    }

    impl Display for Sink {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", String::from_utf8_lossy(self.0.as_ref()))
        }
    }

    #[allow(unused)]
    #[allow(clippy::box_collection)]
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

    // Initializes components required for testing.
    //
    // Because we use hacks to create 'static lifetime references, the component instances
    // must not be dropped. We do so by instantiating the instances in the local scope
    #[macro_export]
    macro_rules! init_test_setup {
        ($runtime:ident, $host:ident, $tx:ident, $sink:ident, $address:ident) => {
            let mut $sink: Box<Vec<u8>> = Box::default();
            let mut $host = tezos_smart_rollup_mock::MockHost::default();
            $host.set_debug_handler(unsafe {
                std::mem::transmute::<&mut std::vec::Vec<u8>, &'static mut Vec<u8>>(
                    $sink.as_mut(),
                )
            });
            let $address =
                <jstz_crypto::smart_function_hash::SmartFunctionHash as jstz_crypto::hash::Hash>::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton")
                    .unwrap();
            let mut $tx = jstz_core::kv::Transaction::default();
            $tx.begin();
            #[allow(unused)]
            let protocol  = Some($crate::Protocol::new(&mut $host, &mut $tx, $address.clone()));
            #[allow(unused)]
            let mut $runtime = $crate::JstzRuntime::new($crate::JstzRuntimeOptions { protocol, ..Default::default() } );
            #[allow(unused)]
            let $sink = $crate::test_utils::Sink($sink);
        };
    }
}
