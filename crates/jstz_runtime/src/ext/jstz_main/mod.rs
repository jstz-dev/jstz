use deno_core::extension;

extension!(
  jstz_main,
  deps = [deno_webidl, deno_console, jstz_console, deno_url, deno_web],
  esm_entry_point = "ext:jstz_main/99_main.js",
  esm = [dir "src/ext/jstz_main", "01_errors.js", "98_global_scope.js", "99_main.js"],
);

#[cfg(test)]
mod test {
    use deno_core::{serde_v8, v8};
    use jstz_utils::test_util::TOKIO_MULTI_THREAD;

    use crate::init_test_setup;

    #[test]
    pub fn random_returns_constant() {
        TOKIO_MULTI_THREAD.block_on(async {
            let code = r#"
        const handler = () => {
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
          return collected
        };

        export default handler;
        "#;
            init_test_setup! {
                runtime = runtime;
                specifier = (s, code);
            };
            let id = runtime.execute_main_module(&s).await.unwrap();
            let result = runtime.call_default_handler(id, &[]).await.unwrap();
            let result = {
                let scope = &mut runtime.handle_scope();
                let local = v8::Local::new(scope, result);
                serde_v8::from_v8::<Vec<f64>>(scope, local).unwrap()
            };
            let expected = [vec![0.42; 10], vec![2304.0, 0.123, -1.0]].concat();
            assert_eq!(expected, result);
        })
    }

    #[test]
    pub fn date_returns_constant() {
        TOKIO_MULTI_THREAD.block_on(async {
            let code = r#"
          export default () => {
            function assert(expected, value) {
                if (value !== expected) throw new Error(`${value}  !== ${expected}`)
            }
            assert(1530380397121, Date.now())
            assert(1530380397121, new Date().getTime())
            assert("Sat Jun 30 2018 18:39:57 GMT+0100 (British Summer Time)", Date())
            assert(1530380397121, new Date(1530380397121).getTime())
            assert("Thu Nov 12 2020 00:00:00 GMT+0000 (Greenwich Mean Time)", new Date(2020, 10, 12).toString())
          }
        "#;

            init_test_setup! {
                runtime = runtime;
                specifier = (s, code);
            };

            let id = runtime.execute_main_module(&s).await.unwrap();
            runtime.call_default_handler(id, &[]).await.expect("Unexpected error!");
        });
    }

    #[test]
    #[allow(non_snake_case)]
    pub fn setTimeout_not_supported() {
        TOKIO_MULTI_THREAD.block_on(async {
            let code = r#"let handler = () => setTimeout(() => console.log('hello'), 100);
                export default handler"#;
            init_test_setup! {
                  runtime = runtime;
                  specifier = (s, code);
            }
            let id = runtime.execute_main_module(&s).await.unwrap();
            let error = runtime.call_default_handler(id, &[]).await.unwrap_err();
            assert_eq!(error.to_string(), "NotSupported: 'setTimeout()' is not supported\n    at ext:jstz_main/98_global_scope.js:159:11\n    at handler (file://jstz/accounts/root:1:21)");
        });
    }

    #[test]
    #[allow(non_snake_case)]
    pub fn setInterval_not_supported() {
        TOKIO_MULTI_THREAD.block_on(async {
        let code = r#"let handler = () => setInterval(() => console.log('hello'), 100);
                      export default handler"#;
        init_test_setup! {
              runtime = runtime;
              specifier = (s, code);
        }
        let id = runtime.execute_main_module(&s).await.unwrap();
        let error = runtime.call_default_handler(id, &[]).await.unwrap_err();
        // FIXME: Do not show line number stacktrace to users
        // https://linear.app/tezos/issue/JSTZ-665
        assert_eq!(error.to_string(), "NotSupported: 'setInterval()' is not supported\n    at ext:jstz_main/98_global_scope.js:155:11\n    at handler (file://jstz/accounts/root:1:21)");
      });
    }

    #[test]
    #[allow(non_snake_case)]
    pub fn clearTimeout_not_supported() {
        TOKIO_MULTI_THREAD.block_on(async {
            let code = r#"let handler = () => clearTimeout(null);
                      export default handler"#;
            init_test_setup! {
              runtime = runtime;
              specifier = (s, code);
            }
            let id = runtime.execute_main_module(&s).await.unwrap();
            let error = runtime.call_default_handler(id, &[]).await.unwrap_err();
            assert_eq!(error.to_string(), "NotSupported: 'clearTimeout()' is not supported\n    at ext:jstz_main/98_global_scope.js:146:11\n    at handler (file://jstz/accounts/root:1:21)");
        });
    }

    #[test]
    #[allow(non_snake_case)]
    pub fn clearInterval_not_supported() {
        TOKIO_MULTI_THREAD.block_on(async {
            let code = r#"let handler = () => clearInterval(null);
                    export default handler;"#;
            init_test_setup! {
                  runtime = runtime;
                  specifier = (s, code);
            }
            let id = runtime.execute_main_module(&s).await.unwrap();
            let error = runtime.call_default_handler(id, &[]).await.unwrap_err();
            assert_eq!(error.to_string(), "NotSupported: 'clearInterval()' is not supported\n    at ext:jstz_main/98_global_scope.js:142:11\n    at handler (file://jstz/accounts/root:1:21)");
        });
    }
}
