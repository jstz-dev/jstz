use deno_core::*;

use crate::api::KvValue;

use super::Protocol;

struct KV;

#[op2]
impl KV {
    #[static_method]
    #[serde]
    fn get(op_state: &mut OpState, #[string] key: &str) -> Option<serde_json::Value> {
        let Protocol { kv, host, tx } = &mut op_state.borrow_mut::<Protocol>();
        let value = kv.get(host, tx, key).ok()??;
        Some(value.0.clone())
    }

    #[static_method]
    fn set(
        op_state: &mut OpState,
        #[string] key: &str,
        #[serde] value: serde_json::Value,
    ) -> bool {
        let Protocol { kv, tx, .. } = &mut op_state.borrow_mut::<Protocol>();
        kv.set(tx, key, KvValue(value)).is_ok()
    }
}

extension!(
    jstz_kv,
    objects = [KV],
    esm_entry_point = "ext:jstz_kv/kv.js",
    esm = [dir "src/runtime/kv", "kv.js"]
);
