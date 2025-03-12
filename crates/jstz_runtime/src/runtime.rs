use deno_core::error::CoreError;
use deno_core::*;
use serde::Deserialize;
use std::ops::Deref;
use std::ops::DerefMut;

/// [`JstzRuntime`] manages the [`JsRuntime`] state. It is also
/// provides [`JsRuntime`] with the instiatiated [`HostRuntime`]
/// and protocol capabilities
pub struct JstzRuntime {
    runtime: JsRuntime,
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
    pub fn init<Protocol: 'static>(
        protocol_ext: Vec<Extension>,
        protocol: Option<Protocol>,
    ) -> Self {
        let mut extensions = init_extenions();
        extensions.extend(protocol_ext);

        let mut runtime = JsRuntime::new(RuntimeOptions {
            extensions,
            ..Default::default()
        });

        if let Some(protocol) = protocol {
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
    ) -> Option<T> {
        let value = self.execute_script("jstz://run", code.to_string()).ok()?;
        let scope = &mut self.handle_scope();
        let local = v8::Local::new(scope, value);
        let t = serde_v8::from_v8::<T>(scope, local).ok()?;
        Some(t)
    }
}

type Result<T> = std::result::Result<T, CoreError>;

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

fn init_extenions() -> Vec<Extension> {
    vec![]
}
