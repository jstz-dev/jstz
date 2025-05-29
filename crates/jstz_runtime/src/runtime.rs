//use jstz_core::host::HostRuntime;
//use jstz_core::host::JsHostRuntime;
//use jstz_core::kv::Transaction;
//use jstz_crypto::hash::Hash;
//use jstz_crypto::smart_function_hash::SmartFunctionHash;
use std::{
    ops::{Deref, DerefMut},
    rc::Rc,
};

use crate::error::Result;
use deno_core::{error::JsError, *};
use serde::Deserialize;
use tokio;

use crate::ext::{/*jstz_console, jstz_kv, jstz_kv::kv::Kv,*/ jstz_main};
use deno_console;
use deno_url;
use deno_web::TimersPermission;
use deno_webidl;
use tezos_smart_rollup::prelude::Runtime;

use std::ffi::CString;
use std::sync::Once;

/// Call this **before** you create any `JsRuntime`.
pub fn init_v8_code_range() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // 1. Build a NUL-terminated C-string
        const FLAG: &str = "--v8-code-range-size=256";
        // 2. Tell V8 about it
        unsafe {
            v8::V8::set_flags_from_string(FLAG);
        }
    });
}

/// Returns the default object of the specified JavaScript namespace (Object).
///
/// Returns `null` if default export is not defined
fn get_default_export<'s>(
    ns: v8::Global<v8::Object>,
    scope: &mut v8::HandleScope<'s>,
) -> v8::Local<'s, v8::Value> {
    let ns_object = ns.open(scope);

    let default_str = v8::String::new_external_onebyte_static(scope, b"default").unwrap();
    ns_object.get(scope, default_str.into()).unwrap()
}

/// [`JstzRuntime`] manages the [`JsRuntime`] state. It is also
/// provides [`JsRuntime`] with the instiatiated [`HostRuntime`]
/// and protocol capabilities
pub struct JstzRuntime {
    runtime: JsRuntime,
}

pub struct JstzRuntimeOptions {
    pub protocol: Option<Protocol>,
    /// Additional extensions to be registered on initialization.
    pub extensions: Vec<Extension>,
    /// Implementation of the `ModuleLoader` which will be
    /// called when we request a ES module in the main realm.
    ///
    /// Not to be confused with ES modules registered by extensions
    /// (these are static, and treated differently)
    pub module_loader: Rc<dyn ModuleLoader>,
}

impl Default for JstzRuntimeOptions {
    fn default() -> Self {
        Self {
            protocol: Default::default(),
            extensions: Default::default(),
            module_loader: Rc::new(NoopModuleLoader),
        }
    }
}

pub struct JstzRuntimeSnapshot(Box<[u8]>);
impl JstzRuntimeSnapshot {
    pub fn snapshot(self) -> &'static [u8] {
        // Safety: `JstzRuntimeSnapshot` is only dropped when the kernel
        // is shutdown
        Box::leak(self.0)
    }

    pub fn new(options: RuntimeOptions) -> Self {
        let snapshot = JsRuntimeForSnapshot::new(options);
        Self(snapshot.snapshot())
    }
}

impl JstzRuntime {
    /// Returns the default [`RuntimeOptions`] configured
    /// with custom extensions
    pub fn options() -> RuntimeOptions {
        let extensions = init_extenions();
        let v8_single_threaded = v8::Platform::new_single_threaded(true).make_shared();
        RuntimeOptions {
            extensions,
            v8_platform: Some(v8_single_threaded),
            ..Default::default()
        }
    }

    /// Creates a new [`JstzRuntime`] with [`JstzRuntimeOptions`]
    pub fn new(options: JstzRuntimeOptions) -> Self {
        //init_v8_code_range();
        let mut extensions = init_extenions();
        extensions.extend(options.extensions);

        let v8_single_threaded = v8::Platform::new_single_threaded(true).make_shared();
        let create_params = v8::CreateParams::default();
        //.heap_limits(300_000_000, 300_000_000)
        //.heap_limits_from_system_memory(300_000_000, 300_000_000);
        let mut runtime = JsRuntime::new(RuntimeOptions {
            extensions,
            module_loader: Some(options.module_loader),
            v8_platform: Some(v8_single_threaded),
            create_params: Some(create_params),
            ..Default::default()
        });

        let op_state = runtime.op_state();

        if let Some(protocol) = options.protocol {
            op_state.borrow_mut().put(protocol);
        };

        op_state.borrow_mut().put(JstzPermissions);

        Self { runtime }
    }

    /// Executes traditional, non-ECMAScript-module JavaScript code, ignoring
    /// its result
    pub fn execute(mut self, code: &str) -> Result<()> {
        self.execute_script("jstz://run", code.to_string())?;
        Ok(())
    }

    /// Executes traditional, non-ECMAScript-module JavaScript code, parsing
    /// its result ot a Rust type T
    pub fn execute_with_result<'de, T: Deserialize<'de>>(
        &mut self,
        code: &str,
    ) -> Result<T> {
        let value = self.execute_script("jstz://run", code.to_string()).unwrap();
        let scope = &mut self.handle_scope();
        let local = v8::Local::new(scope, value);
        Ok(serde_v8::from_v8::<T>(scope, local)?)
    }

    /// Loads and instantiated specified JavaScript module as the "main" module.
    /// The module is "main" in the sense that [`import.meta.main`] is set to [`true`].
    pub async fn preload_main_module(
        &mut self,
        module_specifier: &ModuleSpecifier,
    ) -> Result<ModuleId> {
        Ok(self.runtime.load_main_es_module(module_specifier).await?)
    }

    /// Evaluates specified JavaScript module.
    pub async fn evaluate_module(&mut self, id: ModuleId) -> Result<()> {
        let mut receiver = self.runtime.mod_evaluate(id);
        Ok(tokio::select! {
          result = &mut receiver => {
            result
          }

          run_event_loop_result = self.run_event_loop(Default::default()) => {
            run_event_loop_result?;
            receiver.await
          }
        }?)
    }

    pub async fn run_event_loop(
        &mut self,
        poll_options: PollEventLoopOptions,
    ) -> Result<()> {
        Ok(self.runtime.run_event_loop(poll_options).await?)
    }

    /// Loads, instantiates and executes the specified JavaScript module.
    ///
    /// This module is treated as the "main" module. See [`preload_main_module`]
    /// for details.
    pub async fn execute_main_module(
        &mut self,
        module_specifier: &ModuleSpecifier,
    ) -> Result<ModuleId> {
        let id = self.preload_main_module(module_specifier).await?;
        self.evaluate_module(id).await?;
        Ok(id)
    }

    /// Returns the result of calling the default handler in the specified JavaScript module.
    ///
    /// This function panics if the module has not been loaded.
    pub async fn call_default_handler(
        &mut self,
        id: ModuleId,
    ) -> Result<v8::Global<v8::Value>> {
        let ns = self.runtime.get_module_namespace(id)?;
        let scope = &mut self.handle_scope();

        let default_value = get_default_export(ns, scope);
        let default_fn = v8::Local::<v8::Function>::try_from(default_value)?;

        let result = {
            let tc_scope = &mut v8::TryCatch::new(scope);
            let undefined = v8::undefined(tc_scope);

            // TODO():
            // Support passing values to the handler
            let result = default_fn.call(tc_scope, undefined.into(), &[]);

            if let Some(exn) = tc_scope.exception() {
                let error = JsError::from_v8_exception(tc_scope, exn);
                return Err(error.into());
            }

            result
        };

        Ok(v8::Global::new(scope, result.unwrap()))
    }
}

impl Deref for JstzRuntime {
    type Target = JsRuntime;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

impl DerefMut for JstzRuntime {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.runtime
    }
}

pub struct Protocol {
    //pub host: Box<dyn Runtime>, // Ref&JsHostRuntime<'static>,
    //pub tx: &'static mut Transaction,
    //pub kv: Kv,
}

impl Protocol {
    pub fn new(//hrt: &mut impl Runtime,
        //tx: &mut Transaction,
        //address: SmartFunctionHash,
    ) -> Self {
        //let host = JsHostRuntime::new(hrt);

        // Safety: Since we synchronisely execute Operations, the tx will not be dropped before
        // the runtime, so this is safe
        // TODO: Replace with Arc<Mutex<Transaction>>
        // https://linear.app/tezos/issue/JSTZ-375/replace-andmut-transaction-with-arcmutextransaction
        //let tx = unsafe {
        //    std::mem::transmute::<&mut Transaction, &'static mut Transaction>(tx)
        //};
        Protocol {
            //host,
            //tx,
            //kv: Kv::new(address.to_base58()),
        }
    }
}

#[macro_export]
macro_rules! init_ops_and_esm_extensions  {
    ($($ext:ident $(::<$($generics:ty),*> )? $(($($args:expr),*))?),*) => {
        vec![
            $($ext::$ext::init_ops_and_esm$(::<$($generics),*> )?($($($args),*)?)),*
        ]
    };
}

struct JstzPermissions;

impl TimersPermission for JstzPermissions {
    fn allow_hrtime(&mut self) -> bool {
        // Disables high resolution time
        false
    }
}

fn init_extenions() -> Vec<Extension> {
    init_ops_and_esm_extensions!(
        deno_webidl,
        deno_console,
        //jstz_console,
        deno_url,
        //jstz_kv,
        deno_web::<JstzPermissions>(Default::default(), None),
        jstz_main
    )
}
/*
#[cfg(test)]
mod test {
    use super::*;

    use crate::init_test_setup;

    #[test]
    fn test_init_jstz_runtime() {
        init_test_setup!(runtime, host, tx, sink, address);

        let code = r#"
            Kv.set("hello", "world");
            Kv.set("abc", 42);
            let hello = Kv.get("hello");
            console.log(hello);
            let abc = Kv.get("abc");
            console.log(42);
            42 + 8
        "#;

        let result = runtime.execute_with_result::<u32>(code).unwrap();
        assert_eq!(result, 50);
        assert_eq!(
            sink.to_string(),
            "[INFO] world\n[INFO] \u{1b}[33m42\u{1b}[39m\n".to_string()
        )
    }

    async fn init_and_call_default_handler(
        code: &'static str,
    ) -> (JstzRuntime, Result<v8::Global<v8::Value>>) {
        let specifier =
            resolve_import("file://jstz/accounts/root", "//sf/main.js").unwrap();
        let module_loader = StaticModuleLoader::with(specifier.clone(), code);

        let mut rt = JstzRuntime::new(JstzRuntimeOptions {
            module_loader: Rc::new(module_loader),
            ..Default::default()
        });

        let id = rt.execute_main_module(&specifier).await.unwrap();
        let result = rt.call_default_handler(id).await;
        (rt, result)
    }

    #[tokio::test]
    async fn test_call_default_handler_with_exn() {
        let (_rt, result) = init_and_call_default_handler(
            r#"
function handler() {
    throw "error";
}

export default handler;
        "#,
        )
        .await;

        assert!(result.is_err())
    }

    #[tokio::test]
    async fn test_call_default_handler_with_missing_export() {
        let (_rt, result) = init_and_call_default_handler(
            r#"
export function handler() {
    return 42;
}
        "#,
        )
        .await;

        assert!(result.is_err())
    }

    #[tokio::test]
    async fn test_call_default_handler() {
        let (mut rt, result) = init_and_call_default_handler(
            r#"
function handler() {
    return 42;
}

export default handler;
        "#,
        )
        .await;

        let scope = &mut rt.handle_scope();
        let result_i64 = result.unwrap().open(scope).integer_value(scope).unwrap();
        assert_eq!(result_i64, 42);
    }
}*/
