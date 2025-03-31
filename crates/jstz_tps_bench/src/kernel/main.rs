#![allow(unused)]

use std::sync::Once;

use jstz_crypto::hash::Hash;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use tezos_smart_rollup::{entrypoint, storage::path::RefPath};
use tezos_smart_rollup_core::smart_rollup_core::SmartRollupCore;
use tezos_smart_rollup_host::runtime::Runtime;

#[entrypoint::main]
#[cfg_attr(
    feature = "static-inbox",
    entrypoint::runtime(static_inbox = "./inbox.json")
)]
pub fn entry(host: &mut impl Runtime) {
    // We need to setup the ticketer (bridge address that funds Jstz) for Jstz to not panic.
    {
        static ONCE: Once = Once::new();

        ONCE.call_once(|| {
            let ticketer =
                SmartFunctionHash::from_base58("KT1HbQepzV1nVGg8QVznG7z4RcHseD5kwqBn")
                    .unwrap();

            const TICKETER: RefPath = RefPath::assert_from(b"/ticketer");
            host.store_write(
                &TICKETER,
                &bincode::encode_to_vec(&ticketer, bincode::config::legacy()).unwrap(),
                0,
            )
            .unwrap();
        });
    }

    // Delegate to Jstz kernel
    jstz_kernel::entry(host);
}

#[cfg(not(feature = "static-inbox"))]
fn main() {}
