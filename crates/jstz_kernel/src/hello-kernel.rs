use tezos_smart_rollup::{entrypoint, host::Runtime, prelude::debug_msg};

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    loop {
        let message = rt.read_input();
        debug_msg!(rt, "{:?}\n", message);
    }
}
