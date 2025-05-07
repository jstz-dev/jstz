use jstz_core::host::HostRuntime;
use jstz_core::host::JsHostRuntime;
use jstz_core::kv::Transaction;
use jstz_crypto::hash::Hash;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use parking_lot::FairMutex as Mutex;
use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::{
    ops::{Deref, DerefMut},
    rc::Rc,
};

use crate::error::Result;
use crate::ext::jstz_fetch;
use crate::ext::jstz_fetch::NotSupportedFetch;
use deno_core::*;

use serde::Deserialize;
use tokio;

use crate::ext::{jstz_console, jstz_kv, jstz_kv::kv::Kv, jstz_main};
use deno_console;
use deno_url;
use deno_web::TimersPermission;
use deno_webidl;

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
    runtime: std::mem::ManuallyDrop<JsRuntime>,
}

impl Drop for JstzRuntime {
    fn drop(&mut self) {
        // Safety
        //
        // Deno automatically enters the Isolate upon creation of a new runtime.
        // Additionally, it implements RAII-like behaviour on OwnedIsolates to ensure
        // that the isolate is always up cleaned up and exited (unlocked) upon drop.
        // Its drop behaviour relies on the fact that the isolate being dropped is the currently
        // entered one which effectively makes it impossible to progress runtimes concurrently because
        // other runtimes could have been created and not dropped between the start and end of lifetime
        // of the original isolate.
        //
        // Locking isolates is redundant in a single threaded environment since only one isolate can ever
        // be scheduled at a time. Crucially, the main thread is always allowed to progress any of the isolates. To
        // suport this behaviour, we do two things
        // 1. Since V8 automatically enters the isolate upon creation, we explicitly exit
        //    it. See [`JstzRuntime::new`]
        // 2. Before dropping the runtime, we re-enter it to ensure we satisfy the JsRuntime's
        //    Drop precondition
        unsafe {
            self.runtime.v8_isolate().enter();
            ManuallyDrop::drop(&mut self.runtime);
        };
    }
}
pub struct JstzRuntimeOptions {
    /// Protocol context accessible by protocol defined APIs
    pub protocol: Option<ProtocolContext>,
    /// Additional extensions to be registered on initialization.
    pub extensions: Vec<Extension>,
    /// Implementation of the `ModuleLoader` which will be
    /// called when we request a ES module in the main realm.
    ///
    /// Not to be confused with ES modules registered by extensions
    /// (these are static, and treated differently)
    pub module_loader: Rc<dyn ModuleLoader>,
    /// Fetch extension
    pub fetch: Extension,
}

impl Default for JstzRuntimeOptions {
    fn default() -> Self {
        Self {
            protocol: Default::default(),
            extensions: Default::default(),
            module_loader: Rc::new(NoopModuleLoader),
            fetch: jstz_fetch::jstz_fetch::init_ops_and_esm::<NotSupportedFetch>(()),
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
        RuntimeOptions {
            extensions,
            ..Default::default()
        }
    }

    /// Creates a new [`JstzRuntime`] with [`JstzRuntimeOptions`]
    pub fn new(options: JstzRuntimeOptions) -> Self {
        // Register extensions
        let mut extensions = init_extenions();
        extensions.push(options.fetch);
        extensions.extend(options.extensions);
        let v8_platform = v8::new_single_threaded_default_platform(false).make_shared();
        
        // Construct Runtime options
        let js_runtime_options = RuntimeOptions {
            extensions,
            module_loader: Some(options.module_loader),
            v8_platform: Some(v8_platform),
            ..Default::default()
        };

        // SAFETY: See `impl Drop for JstzRuntime`
        let mut runtime = ManuallyDrop::new(JsRuntime::new(js_runtime_options));
        unsafe { runtime.v8_isolate().exit() };

        // Give protocol access to the running script
        let op_state = runtime.op_state();
        if let Some(protocol) = options.protocol {
            op_state.borrow_mut().put(protocol);
        };
        op_state.borrow_mut().put(JstzPermissions);

        Self { runtime }
    }

    /// Executes traditional, non-ECMAScript-module JavaScript code, ignoring
    /// its result
    pub fn execute(&mut self, code: &str) -> Result<()> {
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
        args: &[v8::Global<v8::Value>],
    ) -> Result<v8::Global<v8::Value>> {
        let ns = self.runtime.get_module_namespace(id)?;
        let default_fn = {
            let scope = &mut self.handle_scope();
            let default_value = get_default_export(ns, scope);
            let default_fn = v8::Local::<v8::Function>::try_from(default_value)?;
            v8::Global::new(scope, default_fn)
        };
        // Note: [`call_with_args`] wraps the scope with TryCatch for us and converts
        // any exception into an error
        let fut = self.call_with_args(&default_fn, args);
        let result = self.with_event_loop_future(fut, Default::default()).await;
        Ok(result?)
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

pub struct ProtocolContext {
    pub host: JsHostRuntime<'static>,
    pub tx: Arc<Mutex<Transaction>>,
    pub kv: Kv,
    pub address: SmartFunctionHash,
}

impl ProtocolContext {
    pub fn new(
        hrt: &mut impl HostRuntime,
        tx: Arc<Mutex<Transaction>>,
        address: SmartFunctionHash,
    ) -> Self {
        let host = JsHostRuntime::new(hrt);
        ProtocolContext {
            host,
            tx,
            kv: Kv::new(address.clone().to_base58()),
            address,
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
        jstz_console,
        deno_url,
        jstz_kv,
        deno_web::<JstzPermissions>(Default::default(), None),
        jstz_main
    )
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::{error::RuntimeError, init_test_setup};

    #[test]
    fn test_init_jstz_runtime() {
        init_test_setup! {
            runtime = runtime;
            sink = sink;
        };
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
        init_test_setup! {
            runtime = rt;
            specifier = (specifier, code);
        };
        let id = rt.execute_main_module(&specifier).await.unwrap();
        let result = rt.call_default_handler(id, &[]).await;
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

    #[tokio::test]
    async fn call_default_handler_returns_error() {
        let (_rt, result) = init_and_call_default_handler(
            r#"
function handler() {
    throw new Error("boom")
}
export default handler;
        "#,
        )
        .await;

        let result = result.unwrap_err();
        assert!(matches!(result, RuntimeError::DenoCore(_)));
    }

    #[tokio::test]
    async fn test_call_default_handler_with_arguments() {
        let code = r#"
    function handler(value) {
        return 42 + value;
    }

    export default handler;
            "#;
        init_test_setup! {
            runtime = rt;
            specifier = (specifier, code);
        };
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
    }
}
