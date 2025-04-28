use deno_core::*;
use kv::KvValue;

use crate::runtime::ProtocolContext;

pub mod kv;

struct Kv;

#[op2]
impl Kv {
    #[static_method]
    #[serde]
    fn get(op_state: &mut OpState, #[string] key: &str) -> Option<serde_json::Value> {
        let ProtocolContext { host, tx, kv } = op_state.borrow_mut::<ProtocolContext>();
        kv.get(host, tx, key).map(|v| v.0.clone())
    }

    #[static_method]
    fn set(
        op_state: &mut OpState,
        #[string] key: &str,
        #[serde] value: serde_json::Value,
    ) -> bool {
        let ProtocolContext { tx, kv, .. } =
            &mut op_state.borrow_mut::<ProtocolContext>();
        kv.set(tx, key, KvValue(value)).is_some()
    }

    #[fast]
    #[static_method]
    fn delete(op_state: &mut OpState, #[string] key: &str) -> bool {
        let ProtocolContext { tx, kv, .. } =
            &mut op_state.borrow_mut::<ProtocolContext>();
        kv.delete(tx, key).is_some()
    }

    #[fast]
    #[static_method]
    fn contains(op_state: &mut OpState, #[string] key: &str) -> bool {
        let ProtocolContext { tx, kv, host } =
            &mut op_state.borrow_mut::<ProtocolContext>();
        kv.has(host, tx, key).is_some_and(|t| t)
    }
}

extension!(
    jstz_kv,
    objects = [Kv],
    esm_entry_point = "ext:jstz_kv/kv.js",
    esm = [dir "src/ext/jstz_kv", "kv.js"]
);

#[cfg(test)]
mod test {
    use crate::init_test_setup;

    #[test]
    fn kv() {
        init_test_setup! {
            runtime = runtime;
        };
        let code = r#"
            Kv.set("hello", "world")
            let value = Kv.get("hello");
            let failed = Kv.get("not found");
            let containsValue = Kv.contains("hello");
            Kv.delete("hello");
            let containsAfterDelete = Kv.contains("hello");
            [value, failed, containsValue, containsAfterDelete]
        "#;
        let (value, failed, has_value, has_value_after_delete) = runtime
            .execute_with_result::<(String, Option<String>, bool, bool)>(code)
            .unwrap();
        assert_eq!(value, "world");
        assert_eq!(failed, None);
        assert!(has_value);
        assert!(!has_value_after_delete);
    }
}
