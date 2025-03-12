use std::{
    ops::{Deref, DerefMut},
    rc::Rc,
};

use deno_core::{error::JsError, *};
use serde::Deserialize;

use crate::error::Result;

fn init_extenions() -> Vec<Extension> {
    vec![]
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

pub struct JstzRuntimeOptions<Protocol: 'static> {
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

impl<Protocol: 'static> Default for JstzRuntimeOptions<Protocol> {
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
        RuntimeOptions {
            extensions,
            ..Default::default()
        }
    }

    /// Creates a new [`JstzRuntime`] with protocol extensions registered
    /// and an instance of Protocol exposed in the [`OpState`]
    pub fn init<Protocol: 'static>(options: JstzRuntimeOptions<Protocol>) -> Self {
        let mut extensions = init_extenions();
        extensions.extend(options.extensions);

        let mut runtime = JsRuntime::new(RuntimeOptions {
            extensions,
            module_loader: Some(options.module_loader),
            ..Default::default()
        });

        if let Some(protocol) = options.protocol {
            let op_state = runtime.op_state();
            op_state.borrow_mut().put(protocol);
        };

        Self { runtime }
    }

    /// Creates a [`JstzRuntime`] from a snapshot blob
    // FIXME: Doesn't work as expected
    pub fn from_snapshot<Protocol: 'static>(
        snapshot: &'static [u8],
        protocol: Option<Protocol>,
    ) -> Self {
        let mut runtime = JsRuntime::new(RuntimeOptions {
            startup_snapshot: Some(snapshot),
            ..Default::default()
        });

        if let Some(protocol) = protocol {
            let op_state = runtime.op_state();
            op_state.borrow_mut().put(protocol);
        };

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

          run_event_loop_result = self.run_event_loop() => {
            run_event_loop_result?;
            receiver.await
          }
        }?)
    }

    pub async fn run_event_loop(&mut self) -> Result<()> {
        Ok(self.runtime.run_event_loop(Default::default()).await?)
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

#[macro_export]
macro_rules! init_ops_and_esm_extensions  {
    ($($ext:ident),*) => {
        vec![
            $($ext::init_ops_and_esm()),*
        ]
    };
}

#[cfg(test)]
mod test {

    use super::*;

    async fn init_and_call_default_handler(
        code: &'static str,
    ) -> (JstzRuntime, Result<v8::Global<v8::Value>>) {
        let specifier =
            resolve_import("file://jstz/accounts/root", "//sf/main.js").unwrap();
        let module_loader = StaticModuleLoader::with(specifier.clone(), code);

        let mut rt = JstzRuntime::init::<()>(JstzRuntimeOptions {
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
}
