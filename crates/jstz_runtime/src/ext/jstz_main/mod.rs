use deno_core::extension;

extension!(
  jstz_main,
  deps = [deno_webidl, deno_console, jstz_console, deno_url, deno_web],
  esm_entry_point = "ext:jstz_main/99_main.js",
  esm = [dir "src/ext/jstz_main", "98_global_scope.js", "99_main.js"],
);

#[cfg(test)]
mod test {
    use crate::init_test_setup;

    #[test]
    pub fn random_returns_constant() {
        init_test_setup! {
            runtime = runtime;
        };
        let code = r#"
          let collected = []
          function assert(expected, value) {
              if (value !== expected) throw new Error(`${value}  !== ${expected}`)
              collected.push(value)
          }
          for (let i = 0; i <10; i++) {
              assert(0.42, Math.random())
          }
          assert(2304, Math.max(0.123, 2304))
          assert(0.123, Math.min(0.123, 2304))
          assert(-1, Math.sign(-3));
          collected
        "#;
        let result: Vec<f64> = runtime.execute_with_result(code).unwrap();
        let expected = [vec![0.42; 10], vec![2304.0, 0.123, -1.0]].concat();
        assert_eq!(expected, result);
    }

    #[test]
    pub fn time_is_unsupported() {}
}
