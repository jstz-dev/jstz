use deno_core::*;
extension!(
    jstz_fetch,

    deps = [deno_fetch],

    esm_entry_point = "ext:jstz_fetch/formdata.js",

    esm = [dir "src/jstz_fetch", "formdata.js"],
);

#[cfg(test)]
mod tests {
    use crate::init_test_setup;

    #[test]
    fn formdata_basic_append_values() {
        init_test_setup!(runtime, host, tx, sink, address);

        let code = r#"
            const formData = new FormData();
            formData.append("key1", "value1");
            formData.append("key2", "value2");

            // Display the values
            for (const value of formData.values()) {
              console.log(value);
            }
        "#;

        runtime.execute(code).unwrap();
        assert_eq!(sink.to_string(), "[INFO] value1\n[INFO] value2\n");
    }

    #[test]
    fn formdata_keys() {
        init_test_setup!(runtime, host, tx, sink, address);

        let code = r#"
            const formData = new FormData();
            formData.append("key1", "value1");
            formData.append("key2", "value2");

            // Display the keys
            for (const key of formData.keys()) {
              console.log(key);
            }
        "#;

        runtime.execute(code).unwrap();
        assert_eq!(sink.to_string(), "[INFO] key1\n[INFO] key2\n");
    }

    #[test]
    fn formdata_entries_multiple_values_for_same_key() {
        init_test_setup!(runtime, host, tx, sink, address);

        let code = r#"
            const formData = new FormData();
            formData.append("key1", "value1");
            formData.append("key1", "value3"); // same key, second value
            formData.append("key2", "value2");

            // Display entries
            for (const [k, v] of formData.entries()) {
              console.log(`${k} -> ${v}`);
            }
        "#;

        runtime.execute(code).unwrap();

        let expected =
            "[INFO] key1 -> value1\n[INFO] key1 -> value3\n[INFO] key2 -> value2\n";
        assert_eq!(sink.to_string(), expected);
    }
}
