pub mod error;
pub mod ext;
pub use ext::jstz_kv::kv::*;

pub mod runtime;
pub mod sys;

#[cfg(feature = "wpt")]
pub mod wpt;

pub use ext::*;
pub use runtime::{JstzRuntime, JstzRuntimeOptions, RuntimeContext};

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

    // Initializes components required for testing. We need to do this because we use
    // hacks to create 'static lifetime references which means the locally created
    // references must not be dropped. We do so by instantiating the instances in the
    // local scope.
    //
    // # Example
    //
    // ```
    // init_test_setup! {
    //      runtime = runtime;
    //      host = host;
    //      tx = tx;
    //      sink = sink;
    //      address = address;
    //      specifier = (specifier, code);
    // }
    // ```
    // - (required) runtime   - JstzRuntime
    // - (optional) host      - MockHost
    // - (optional) tx        - Transaction
    // - (optional) specifier - Loads `code` as module whose module specifier is bounded to `specifier`
    // - (optional) sink      - Captures `debug_msg!` messages for use in assertions. Defaults to stdout
    // - (optional) address   - Provides a hash for the smart function

    #[macro_export]
    macro_rules! init_test_setup {
        (
            runtime = $runtime:ident;
            $(host = $host:ident;)?
            $(tx = $tx:ident;)?
            $(specifier = ($specifier:ident,$code:ident);)?
            $(sink = $sink:ident;)?
            $(address = $addr:ident;)?
            $(fetch = $fetch_ext:expr;)?
            $(request_id = $request_id:tt;)?
            $(extensions = $extensions:expr;)?
        ) => {
            #[allow(unused)]
            let mut init_host = tezos_smart_rollup_mock::MockHost::default();
            let mut init_tx = jstz_core::kv::Transaction::default();
            init_tx.begin();
            #[allow(unused)]
            let module_loader = deno_core::NoopModuleLoader;
            let init_addr =
                <jstz_crypto::smart_function_hash::SmartFunctionHash as jstz_crypto::hash::Hash>::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton")
            .unwrap();
            $(
                let $specifier = deno_core::resolve_import("file://jstz/accounts/root", "//sf/main.js").unwrap();
                let module_loader = deno_core::StaticModuleLoader::with($specifier.clone(), $code);
            )?
            #[allow(unused)]
            let request_id = String::new();
            $(let request_id = $request_id.to_string();)?
            #[allow(unused)]
            let protocol  = Some($crate::RuntimeContext::new(&mut init_host, &mut init_tx, init_addr.clone(), request_id, $crate::runtime::Limiter::<5>::default().try_acquire().unwrap()));
            #[allow(unused)]
            let mut $runtime = $crate::JstzRuntime::new($crate::JstzRuntimeOptions {
                protocol,
                module_loader: std::rc::Rc::new(module_loader),
                $(
                    fetch: $fetch_ext,
                )?
                $(
                    extensions: $extensions,
                )?
                ..Default::default()

            });

            $(let mut $host = init_host;)?
            $(let mut $tx = init_tx;)?
            $(let $addr = init_addr;)?
            $(
                let mut $sink: Box<Vec<u8>> = Box::default();
                init_host.set_debug_handler(unsafe {
                std::mem::transmute::<&mut std::vec::Vec<u8>, &'static mut Vec<u8>>(
                    $sink.as_mut(),
                    )
                });
                #[allow(unused)]
                let $sink = $crate::test_utils::Sink($sink);
            )?
        };
    }
}
