use crate::error::Result;
use crate::ext::jstz_fetch::FetchAPI;
use crate::ext::jstz_fetch::NotSupportedFetch;
use deno_core::v8::new_single_threaded_default_platform;
use deno_core::*;
use derive_more::{Deref, DerefMut};
use jstz_core::host::HostRuntime;
use jstz_core::host::JsHostRuntime;
use jstz_core::kv::Transaction;
use jstz_crypto::hash::Hash;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use pin_project::pin_project;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::{
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use serde::Deserialize;

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
        // be scheduled at a time. However, an isolate can only progress correctly if it is the currently
        // entered one. Crucially, the main thread is always allowed to progress any of the isolates. To
        // support this behaviour, we do:
        // 1. Since V8 automatically enters the isolate upon creation, we explicitly exit
        //    it. See [`JstzRuntime::new`]
        // 2. Before dropping the runtime, we re-enter it to ensure we satisfy the JsRuntime's
        //    Drop precondition
        // 3. Ensure that the curent isolate is entered/exited on execution of code through explicit
        //    re-entrance for sync code or wrapping in `IsolatedFuture` for async code
        unsafe {
            self.runtime.v8_isolate().enter();
            ManuallyDrop::drop(&mut self.runtime);
        };
    }
}
pub struct JstzRuntimeOptions<F: FetchAPI> {
    /// Protocol context accessible by protocol defined APIs
    pub protocol: Option<RuntimeContext>,
    /// Additional extensions to be registered on initialization.
    pub extensions: Vec<Extension>,
    /// Implementation of the `ModuleLoader` which will be
    /// called when we request a ES module in the main realm.
    ///
    /// Not to be confused with ES modules registered by extensions
    /// (these are static, and treated differently)
    pub module_loader: Rc<dyn ModuleLoader>,
    /// Fetch extension
    pub fetch: F,
}

impl Default for JstzRuntimeOptions<NotSupportedFetch> {
    fn default() -> Self {
        Self {
            protocol: Default::default(),
            extensions: Default::default(),
            module_loader: Rc::new(NoopModuleLoader),
            fetch: NotSupportedFetch,
        }
    }
}
impl JstzRuntime {
    /// Creates a new [`JstzRuntime`] with [`JstzRuntimeOptions`]
    pub fn new<F: FetchAPI>(options: JstzRuntimeOptions<F>) -> Self {
        // Register extensions
        let mut extensions = vec![];
        extensions.extend(init_base_extensions_ops_and_esm::<F>());
        extensions.extend(options.extensions);
        Self::new_inner(extensions, options.module_loader, options.protocol, None)
    }

    /// Creates a new [`JstzRuntime`] with [`JstzRuntimeOptions`] from a previously
    /// snapshotted [`JsRuntime`]. Using a snapshot will reduce startup latency
    pub fn new_from_snapshot<F: FetchAPI>(
        options: JstzRuntimeOptions<F>,
        snapshot: &'static [u8],
    ) -> Self {
        let mut extensions = vec![];
        extensions.extend(init_base_extensions_ops::<F>());
        extensions.extend(options.extensions);
        Self::new_inner(
            extensions,
            options.module_loader,
            options.protocol,
            Some(snapshot),
        )
    }

    /// Unlike `new`, this function will not add default extensions
    pub(crate) fn new_inner(
        // The full extensions to initialize
        extensions: Vec<Extension>,
        module_loader: Rc<dyn ModuleLoader>,
        protocol: Option<RuntimeContext>,
        snapshot: Option<&'static [u8]>,
    ) -> Self {
        let v8_platform = Some(new_single_threaded_default_platform(false).make_shared());
        // Construct Runtime options
        let js_runtime_options = RuntimeOptions {
            extensions,
            module_loader: Some(module_loader),
            v8_platform,
            startup_snapshot: snapshot,
            skip_op_registration: false,
            ..Default::default()
        };

        // SAFETY: See `impl Drop for JstzRuntime`
        let mut runtime = ManuallyDrop::new(JsRuntime::new(js_runtime_options));
        unsafe { runtime.v8_isolate().exit() };
        // Give protocol access to the running script
        let op_state = runtime.op_state();
        if let Some(protocol) = protocol {
            op_state.borrow_mut().put(protocol);
        };
        op_state.borrow_mut().put(JstzPermissions);

        Self { runtime }
    }

    pub fn set_state<S: 'static>(&mut self, state: S) {
        let op_state = self.op_state();
        op_state.borrow_mut().put(state);
    }

    /// Executes traditional, non-ECMAScript-module JavaScript code, ignoring
    /// its result
    pub fn execute(&mut self, code: &str) -> Result<()> {
        let mut this = Entered::new(self);
        this.execute_script("jstz://run", code.to_string())?;
        Ok(())
    }

    /// Executes traditional, non-ECMAScript-module JavaScript code, parsing
    /// its result ot a Rust type T
    pub fn execute_with_result<'de, T: Deserialize<'de>>(
        &mut self,
        code: &str,
    ) -> Result<T> {
        let mut this = Entered::new(self);
        let value = this.execute_script("jstz://run", code.to_string())?;
        let result = {
            let scope = &mut this.handle_scope();
            let local = v8::Local::new(scope, value);
            serde_v8::from_v8::<T>(scope, local)?
        };
        Ok(result)
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
    // TODO: Should we operate on Pin<&mut Self> instead?
    pub async fn call_default_handler(
        &mut self,
        id: ModuleId,
        args: &[v8::Global<v8::Value>],
    ) -> Result<v8::Global<v8::Value>> {
        let fut = AsyncEntered::new(self, |runtime| {
            runtime.call_default_handler_inner(id, args)
        });
        fut.await
    }

    async fn call_default_handler_inner(
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
        // FIXME(ryan): If user code throws an uncaught exception, the original
        // exception is lost and replaced with Uncaught undefined
        let fut = self.call_with_args(&default_fn, args);
        let result = self.with_event_loop_future(fut, Default::default()).await;
        Ok(result?)
    }
}

/// RAII guard for entering and existing an Isolate.
#[derive(Deref, DerefMut)]
struct Entered<'a> {
    runtime: &'a mut JstzRuntime,
}

impl<'a> Entered<'a> {
    pub fn new(runtime: &'a mut JstzRuntime) -> Self {
        unsafe { runtime.v8_isolate().enter() };
        Entered { runtime }
    }
}

impl Drop for Entered<'_> {
    fn drop(&mut self) {
        unsafe { self.runtime.v8_isolate().exit() };
    }
}

#[pin_project]
pub struct AsyncEntered<'a, B, F>
where
    B: FnOnce(&'a mut JstzRuntime) -> F,
    F: Future,
{
    #[pin]
    future: Option<F>,
    builder: Option<B>,
    runtime_ptr: *mut JstzRuntime,
    marker: PhantomData<&'a ()>,
}

impl<'a, B, F> AsyncEntered<'a, B, F>
where
    B: FnOnce(&'a mut JstzRuntime) -> F,
    F: Future,
{
    pub fn new(runtime: &'a mut JstzRuntime, builder: B) -> Self {
        // # Safety: Ok
        AsyncEntered {
            builder: Some(builder),
            future: None,
            runtime_ptr: runtime as *mut _,
            marker: PhantomData,
        }
    }
}

impl<'a, B, F> Future for AsyncEntered<'a, B, F>
where
    B: FnOnce(&'a mut JstzRuntime) -> F,
    F: Future,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        unsafe {
            (**this.runtime_ptr).v8_isolate().enter();
        };
        let res = {
            if this.future.is_none() {
                let builder = this
                    .builder
                    .take()
                    .expect("builder must be present on first poll");
                this.future
                    .set(Some(builder(unsafe { &mut **this.runtime_ptr })));
            }

            let fut = this.future.as_mut().as_pin_mut().unwrap();
            fut.poll(cx)
        };
        unsafe {
            (**this.runtime_ptr).v8_isolate().exit();
        };
        res
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

pub struct RuntimeContext {
    pub host: JsHostRuntime<'static>,
    pub tx: Transaction,
    pub kv: Kv,
    pub address: SmartFunctionHash,
    pub request_id: String,
}

impl RuntimeContext {
    pub fn new(
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        address: SmartFunctionHash,
        request_id: String,
    ) -> Self {
        let host = JsHostRuntime::new(hrt);
        RuntimeContext {
            host,
            tx: tx.clone(),
            kv: Kv::new(address.to_base58()),
            address,
            request_id,
        }
    }
}

pub struct JstzPermissions;

impl TimersPermission for JstzPermissions {
    fn allow_hrtime(&mut self) -> bool {
        // Disables high resolution time
        false
    }
}

/// Initializes extension ops and esm sources
fn init_base_extensions_ops_and_esm<F: FetchAPI>() -> Vec<Extension> {
    vec![
        deno_webidl::deno_webidl::init_ops_and_esm(),
        deno_console::deno_console::init_ops_and_esm(),
        jstz_console::jstz_console::init_ops_and_esm(),
        deno_url::deno_url::init_ops_and_esm(),
        jstz_kv::jstz_kv::init_ops_and_esm(),
        deno_web::deno_web::init_ops_and_esm::<JstzPermissions>(Default::default(), None),
        deno_fetch_base::deno_fetch::init_ops_and_esm::<F>(F::options()),
        jstz_main::jstz_main::init_ops_and_esm(),
    ]
}

/// Initializes extension ops only. Used when initializing runtime
/// from snapshot
fn init_base_extensions_ops<F: FetchAPI>() -> Vec<Extension> {
    vec![
        deno_webidl::deno_webidl::init_ops(),
        deno_console::deno_console::init_ops(),
        jstz_console::jstz_console::init_ops(),
        deno_url::deno_url::init_ops(),
        jstz_kv::jstz_kv::init_ops(),
        deno_web::deno_web::init_ops::<JstzPermissions>(Default::default(), None),
        deno_fetch_base::deno_fetch::init_ops::<F>(F::options()),
        jstz_main::jstz_main::init_ops(),
    ]
}

#[cfg(test)]
mod test {

    use super::*;

    use crate::{error::RuntimeError, init_test_setup};

    use jstz_utils::test_util::TOKIO;
    use tezos_smart_rollup_mock::MockHost;

    #[test]
    #[cfg_attr(
        feature = "kernel",
        ignore = "logging format is different when kernel feature is enabled"
    )]
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
        assert_eq!(sink.to_string(), "[INFO] world\n[INFO] 42\n".to_string())
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

    #[test]
    fn test_call_default_handler_with_exn() {
        TOKIO.block_on(async {
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
        })
    }

    #[test]
    fn test_call_default_handler_with_missing_export() {
        TOKIO.block_on(async {
            let (_rt, result) = init_and_call_default_handler(
                r#"
export function handler() {
    return 42;
}
        "#,
            )
            .await;

            assert!(result.is_err())
        })
    }

    #[test]
    fn test_call_default_handler() {
        TOKIO.block_on(async {
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
        })
    }

    #[test]
    fn call_default_handler_returns_error() {
        TOKIO.block_on(async {
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
        })
    }

    #[test]
    fn test_call_default_handler_with_arguments() {
        TOKIO.block_on(async {
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
        })
    }

    #[test]
    #[ignore = "Will run forever"]
    fn test_infinite_loop() {
        TOKIO.block_on(async {
            let code = r#"
    function handler() {
    let i = 0;
        while (true) {
            console.log(i);
            i++;
        }
    }
    export default handler;
            "#;
            init_test_setup! {
                runtime = rt;
                specifier = (specifier, code);
            };
            let id = rt.execute_main_module(&specifier).await.unwrap();
            let _ = rt.call_default_handler(id, &[]).await;
        })
    }

    #[test]
    fn test_snapshot() {
        use deno_core::snapshot::create_snapshot;
        use deno_core::snapshot::CreateSnapshotOptions;
        let extensions = init_base_extensions_ops_and_esm::<NotSupportedFetch>();
        let options = CreateSnapshotOptions {
            cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
            startup_snapshot: None,
            skip_op_registration: false,
            extensions,
            extension_transpiler: None,
            with_runtime_cb: None,
        };
        let snapshot = create_snapshot(options, None).unwrap();
        let static_snapshot = Box::leak(snapshot.output);

        TOKIO.block_on(async {
            let code = r#"
function handler() {
    Kv.set("abc", 22)
    let result = Kv.get("abc")
    return 42 + result;
}

export default handler;
        "#;
            let mut host = MockHost::default();
            let mut tx = Transaction::default();
            tx.begin();
            let init_addr = SmartFunctionHash::digest(&[0u8; 32]).unwrap();
            let specifier =
                resolve_import("file://jstz/accounts/root", "//sf/main.js").unwrap();
            let module_loader = StaticModuleLoader::with(specifier.clone(), code);
            let protocol =
                RuntimeContext::new(&mut host, &mut tx, init_addr.clone(), String::new());
            let mut runtime = JstzRuntime::new_from_snapshot(
                JstzRuntimeOptions {
                    protocol: Some(protocol),
                    extensions: vec![],
                    module_loader: Rc::new(module_loader),
                    fetch: NotSupportedFetch,
                },
                static_snapshot,
            );

            let id = runtime.execute_main_module(&specifier).await.unwrap();
            let result = runtime.call_default_handler(id, &[]).await;
            let scope = &mut runtime.handle_scope();
            let result_i64 = result.unwrap().open(scope).integer_value(scope).unwrap();
            assert_eq!(result_i64, 64);
        })
    }
}
