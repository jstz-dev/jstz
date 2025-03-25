use crate::runtime::Protocol;
use deno_core::*;

#[op2]
#[string]
pub fn op_self_address(op_state: &mut OpState) -> String {
    let proto = op_state.borrow::<Protocol>();
    proto.kv.prefix.to_string()
}

extension!(
    jstz_meta,
    deps = [],
    ops = [op_self_address],
    esm_entry_point = "ext:jstz_meta/meta.js",
    esm = [dir "src/jstz_meta", "meta.js"],
);

#[cfg(test)]
mod test {
    use crate::init_test_setup;
    use jstz_crypto::hash::Hash;

    #[test]
    fn test_self_address() {
        init_test_setup!(runtime, host, tx, sink, address);
        let code = r#"global.selfAddress"#;
        let result = runtime.execute_with_result::<String>(code).unwrap();
        assert_eq!(result, address.to_base58());
    }
}
