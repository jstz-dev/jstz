pub mod kv;
pub(crate) mod extension {
    use super::kv::KvValue;
    use crate::{ext::NotSupported, runtime::RuntimeContext};
    use deno_core::{extension, op2, OpState};
    use thiserror;
    struct Kv;

    const NOT_SUPPORTED_ERROR: NotSupported = NotSupported { name: "Kv" };
    #[op2]
    impl Kv {
        #[static_method]
        #[serde]
        fn get(
            op_state: &mut OpState,
            #[string] key: &str,
        ) -> Result<Option<serde_json::Value>> {
            let maybe_proto = op_state.try_borrow_mut::<RuntimeContext>();
            match maybe_proto {
                Some(RuntimeContext { host, tx, kv, .. }) => {
                    let maybe_value = kv
                        .get(host, tx, key)
                        .map_err(|e| KvError::JstzCoreError(e.to_string()))?;
                    Ok(maybe_value.map(|v| v.0.clone()))
                }
                None => Err(NOT_SUPPORTED_ERROR)?,
            }
        }

        #[static_method]
        fn set(
            op_state: &mut OpState,
            #[string] key: &str,
            #[serde] value: serde_json::Value,
        ) -> Result<()> {
            let maybe_proto = op_state.try_borrow_mut::<RuntimeContext>();
            match maybe_proto {
                Some(RuntimeContext { tx, kv, .. }) => kv
                    .set(tx, key, KvValue(value))
                    .map_err(|e| KvError::JstzCoreError(e.to_string())),
                None => Err(NOT_SUPPORTED_ERROR)?,
            }
        }

        #[fast]
        #[static_method]
        fn delete(op_state: &mut OpState, #[string] key: &str) -> Result<()> {
            let maybe_proto = op_state.try_borrow_mut::<RuntimeContext>();
            match maybe_proto {
                Some(RuntimeContext { tx, kv, .. }) => kv
                    .delete(tx, key)
                    .map_err(|e| KvError::JstzCoreError(e.to_string())),
                None => Err(NOT_SUPPORTED_ERROR)?,
            }
        }

        #[fast]
        #[static_method]
        fn contains(op_state: &mut OpState, #[string] key: &str) -> Result<bool> {
            let maybe_proto = op_state.try_borrow_mut::<RuntimeContext>();
            match maybe_proto {
                Some(RuntimeContext { tx, kv, host, .. }) => kv
                    .has(host, tx, key)
                    .map_err(|e| KvError::JstzCoreError(e.to_string())),
                None => Err(NOT_SUPPORTED_ERROR)?,
            }
        }
    }

    #[derive(Debug, thiserror::Error, deno_error::JsError)]
    pub enum KvError {
        #[class(generic)]
        #[error("{0}")]
        JstzCoreError(String),

        #[class(inherit)]
        #[error(transparent)]
        UnsupportedError(#[from] NotSupported),
    }

    type Result<T> = std::result::Result<T, KvError>;

    extension!(
        jstz_kv,
        objects = [Kv],
        esm_entry_point = "ext:jstz_kv/kv.js",
        esm = [dir "src/ext/jstz_kv", "kv.js"]
    );

    #[cfg(test)]
    mod test {
        use deno_error::JsErrorClass;

        use crate::{init_test_setup, JstzRuntime, JstzRuntimeOptions};

        #[test]
        fn kv() {
            init_test_setup! {
                runtime = runtime;
            };
            let code = r#"
                Kv.set("hello", "world")
                let value = Kv.get("hello");
                let failed = Kv.get("not/found");
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

        #[test]
        fn kv_not_supported() {
            let mut runtime = JstzRuntime::new(JstzRuntimeOptions::default());
            let code = r#"Kv.set("hello", "world")"#;
            let err = runtime.execute(code).unwrap_err();
            assert_eq!(err.get_class(), "NotSupported");
            assert!(err.get_message().contains("Kv is not supported"));

            let code = r#"Kv.get("hello")"#;
            let err = runtime.execute(code).unwrap_err();
            assert_eq!(err.get_class(), "NotSupported");
            assert!(err.get_message().contains("Kv is not supported"));

            let code = r#"Kv.contains("hello")"#;
            let err = runtime.execute(code).unwrap_err();
            assert_eq!(err.get_class(), "NotSupported");
            assert!(err.get_message().contains("Kv is not supported"));

            let code = r#"Kv.delete("hello")"#;
            let err = runtime.execute(code).unwrap_err();
            assert_eq!(err.get_class(), "NotSupported");
            assert!(err.get_message().contains("Kv is not supported"));
        }
    }
}
pub use extension::*;
