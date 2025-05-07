use std::rc::Rc;

use deno_core::v8;
use jstz_runtime::{JstzRuntime, JstzRuntimeOptions};

const SCRIPT: &str = r#"
    function handler(value) {
        return 42 + value;
    }

    export default handler;
"#;

pub fn main() {
    let tokio = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
    let specifier = deno_core::resolve_import("file://jstz/accounts/root", "//sf/main.js").unwrap();
    let module_loader = deno_core::StaticModuleLoader::with(specifier.clone(), SCRIPT);
    let options = JstzRuntimeOptions {
        module_loader: Rc::new(module_loader),
        ..Default::default()
    };
    let mut rt = JstzRuntime::new(options);
    tokio.block_on(async {
        let id = rt.execute_main_module(&specifier).await.unwrap();
        let value = {
            let scope = &mut rt.handle_scope();
            let value = v8::Integer::new(scope, 20_i32).cast::<v8::Value>();
            v8::Global::new(scope, value)
        };
        let result = rt.call_default_handler(id, &[value]).await;
        let scope = &mut rt.handle_scope();
        let result_i64 = result.unwrap().open(scope).integer_value(scope).unwrap();
        assert_eq!(result_i64, 62);
    })
}
