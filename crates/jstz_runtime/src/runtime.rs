use deno_core::*;
use jstz_core::host::HostRuntime;
use jstz_core::host::JsHostRuntime;
use jstz_core::kv::Transaction;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use serde::Deserialize;
use std::ops::Deref;
use std::ops::DerefMut;

fn init_extenions() -> Vec<Extension> {
    vec![]
}

/// [`JstzRuntime`] manages the [`JsRuntime`] state. It is also
/// provides [`JsRuntime`] with the instiatiated [`HostRuntime`]
/// and protocol capabilities
pub struct JstzRuntime {
    runtime: JsRuntime,
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
    pub fn init(
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        address: SmartFunctionHash,
    ) -> Self {
        let protocol = Protocol::new(hrt, tx, address);

        let mut runtime = JsRuntime::new(RuntimeOptions {
            extensions: init_extenions(),
            ..Default::default()
        });

        let op_state = runtime.op_state();
        op_state.borrow_mut().put(protocol);

        Self { runtime }
    }

    /// Executes traditional, non-ECMAScript-module JavaScript code, ignoring
    /// its result
    pub fn execute(mut self, code: &str) -> Option<()> {
        self.execute_script("jstz://run", code.to_string()).ok()?;
        Some(())
    }

    /// Executes traditional, non-ECMAScript-module JavaScript code, parsing
    /// its result ot a Rust type T
    pub fn execute_with_result<'de, T: Deserialize<'de>>(
        &mut self,
        code: &str,
    ) -> Option<T> {
        let value = self.execute_script("jstz://run", code.to_string()).unwrap();
        let scope = &mut self.handle_scope();
        let local = v8::Local::new(scope, value);
        serde_v8::from_v8::<T>(scope, local).ok()
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
    pub host: JsHostRuntime<'static>,
    pub tx: &'static mut Transaction,
    pub address: SmartFunctionHash,
}

impl Protocol {
    pub fn new(
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        address: SmartFunctionHash,
    ) -> Self {
        let host = JsHostRuntime::new(hrt);

        // Safety: Since we synchronisely execute Operatios, the tx will not be dropped before
        // the runtime, so this is safe
        // TODO: Replace with Arc<Mutex<Transaction>>
        // https://linear.app/tezos/issue/JSTZ-375/replace-andmut-transaction-with-arcmutextransaction
        let tx = unsafe {
            std::mem::transmute::<&mut Transaction, &'static mut Transaction>(tx)
        };
        Protocol { host, tx, address }
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
