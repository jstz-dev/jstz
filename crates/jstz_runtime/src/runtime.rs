use crate::error::Result;
use crate::ext::jstz_fetch;
use crate::ext::jstz_fetch::NotSupportedFetch;
use deno_core::v8::new_single_threaded_default_platform;
use deno_core::v8::CreateParams;
use deno_core::v8::OwnedIsolate;
use deno_core::*;
use jstz_core::host::HostRuntime;
use jstz_core::host::JsHostRuntime;
use jstz_core::kv::Transaction;
use jstz_crypto::hash::Hash;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use pin_project::pin_project;
use std::mem::ManuallyDrop;
use std::{
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

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

const FOUR_MIB: usize = 4_194_304;
const SIXTY_FOUR_MIB: usize = FOUR_MIB * 16;

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

        let v8_platform = Some(new_single_threaded_default_platform(false).make_shared());

        // 12Mb initial memory, 64Mb max memor
        let params = CreateParams::default().heap_limits(FOUR_MIB, FOUR_MIB * 2);
        // Construct Runtime options
        let js_runtime_options = RuntimeOptions {
            extensions,
            module_loader: Some(options.module_loader),
            create_params: Some(params),
            v8_platform,
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

    extern "C" fn near_heap_limit_callback2(
        data: *mut std::ffi::c_void,
        current_heap_limit: usize,
        initial_heap_limit: usize,
    ) -> usize {
        println!("Initial Heap Limit: {initial_heap_limit}");
        println!("Current Heap Limit: {current_heap_limit}");
        let context: Box<v8::Local<v8::Context>> =
            unsafe { Box::from_raw(data as *mut v8::Local<v8::Context>) };
        let scope = unsafe { v8::CallbackScope::new(*context) };
        // let message = v8::String::new(&mut scope, "Out of memory").unwrap();
        // let exception = v8::Exception::error(&mut scope, message);
        scope.terminate_execution();
        // std::mem::forget(handle_scope);
        std::mem::forget(scope);
        std::mem::forget(context);
        // std::mem::forget(data);
        if current_heap_limit < SIXTY_FOUR_MIB {
            current_heap_limit + FOUR_MIB
        } else {
            current_heap_limit
        }
    }

    /// Executes traditional, non-ECMAScript-module JavaScript code, ignoring
    /// its result
    pub fn execute(&mut self, code: &str) -> Result<()> {
        unsafe { self.v8_isolate().enter() }
        self.execute_script("jstz://run", code.to_string())?;
        unsafe { self.v8_isolate().exit() }
        Ok(())
    }

    /// Executes traditional, non-ECMAScript-module JavaScript code, parsing
    /// its result ot a Rust type T
    pub fn execute_with_result<'de, T: Deserialize<'de>>(
        &mut self,
        code: &str,
    ) -> Result<T> {
        unsafe { self.v8_isolate().enter() }
        let value = self.execute_script("jstz://run", code.to_string()).unwrap();
        let result = {
            let scope = &mut self.handle_scope();
            let local = v8::Local::new(scope, value);
            serde_v8::from_v8::<T>(scope, local)?
        };
        unsafe { self.v8_isolate().exit() }
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
        let isolate_ptr = self.v8_isolate() as *mut _;
        let fut = IsolatedFuture::new(isolate_ptr, || {
            self.call_default_handler_inner(id, args)
        });
        fut.await
    }

    async fn call_default_handler_inner(
        &mut self,
        id: ModuleId,
        args: &[v8::Global<v8::Value>],
        // mut cancel_token: tokio_util::sync::CancellationToken,
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
        let fut = {
            let scope = &mut self.handle_scope();
            let mut scope = v8::HandleScope::new(scope);
            let context = Box::into_raw(Box::new(scope.get_current_context()));
            scope.add_near_heap_limit_callback(
                Self::near_heap_limit_callback2,
                context as *mut _,
            );
            JsRuntime::scoped_call_with_args(&mut scope, &default_fn, args)
        };
        let result = self.with_event_loop_future(fut, Default::default()).await;
        Ok(result?)
    }

    async fn _call_default_handler_inner_(
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

#[pin_project]
pub struct IsolatedFuture<B, F>
where
    B: FnOnce() -> F,
    F: Future,
{
    #[pin]
    future: Option<F>,
    builder: Option<B>,
    isolate_ptr: *mut OwnedIsolate,
}

impl<B, F> IsolatedFuture<B, F>
where
    B: FnOnce() -> F,
    F: Future,
{
    pub fn new(isolate_ptr: *mut OwnedIsolate, builder: B) -> Self {
        // # Safety: Ok
        IsolatedFuture {
            builder: Some(builder),
            future: None,
            isolate_ptr,
        }
    }
}

impl<B, F> Future for IsolatedFuture<B, F>
where
    B: FnOnce() -> F,
    F: Future,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        unsafe {
            this.isolate_ptr.as_mut().unwrap().enter();
        };
        let res = {
            if this.future.is_none() {
                let builder = this
                    .builder
                    .take()
                    .expect("builder must be present on first poll");
                this.future.set(Some(builder()));
            }

            let fut = this.future.as_mut().as_pin_mut().unwrap();
            fut.poll(cx)
        };
        unsafe {
            this.isolate_ptr.as_mut().unwrap().exit();
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

pub struct ProtocolContext {
    pub host: JsHostRuntime<'static>,
    pub tx: Transaction,
    pub kv: Kv,
    pub address: SmartFunctionHash,
}

impl ProtocolContext {
    pub fn new(
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        address: SmartFunctionHash,
    ) -> Self {
        let host = JsHostRuntime::new(hrt);
        ProtocolContext {
            host,
            tx: tx.clone(),
            kv: Kv::new(address.to_base58()),
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

    use jstz_utils::test_util::TOKIO;

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
    fn test_out_of_memory_handler() {
        TOKIO.block_on(async {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async {
                    // let mut aborted = tokio::signal::unix::signal(
                    //     tokio::signal::unix::SignalKind::from_raw(libc::SIGTRAP),
                    // )
                    // .unwrap();
                    let local = tokio::task::spawn_local(async {
                        let code = r#"
                            const handler = async () => {
                                let s = Array(4294967295).fill("a");
                                // console.log("ok!")
                                return new Response("");
                            };
                            export default handler;
                        "#;
                        init_test_setup! {
                            runtime = rt;
                            specifier = (specifier, code);
                        };
                        let id = rt.execute_main_module(&specifier).await.unwrap();
                        rt.call_default_handler(id, &[]).await
                    });
                    // let abort_handle = local.abort_handle();
                    tokio::select! {
                        // _ = aborted.recv() => {
                        //     abort_handle.abort();
                        //     println!("Abort caught succesfully!");
                        // },
                        _ = local => {
                            println!("Executed successfully!");
                        }
                    };
                })
                .await;
        })
    }
}

/*
let local = tokio::task::LocalSet::new();
            local.run_until(async {
                let code = r#"
                const handler = async () => {
                    let s = Array(4294967295).fill("a".repeat(100));
                    // console.log("ok!")
                    return new Response("");
                };
                export default handler;
            "#;
                init_test_setup! {
                    runtime = rt;
                    specifier = (specifier, code);
                };

                let id = rt.execute_main_module(&specifier).await.unwrap();

                let cancel_token = tokio_util::sync::CancellationToken::new();
                let child = cancel_token.child_token();
                let call_default_handler = tokio::task::spawn_local(
                    rt.call_default_handler_with_cancel(id, &[], cancel_token),
                );
                let abort_handle = call_default_handler.abort_handle();

                let _ = tokio::select! {
                    _ = child.cancelled() => {
                        abort_handle.abort();
                        ();
                    }
                    join_result = call_default_handler => {
                        match join_result {
                            Ok(_join_result) => println!("Succesful"),
                            Err(err) => println!("{:?}", err)
                        }
                    }
                };
                todo!()
            }).await;
*/
