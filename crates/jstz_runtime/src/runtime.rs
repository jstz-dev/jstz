use deno_core::error::CoreError;
use deno_core::*;
use jstz_core::host::HostRuntime;
use jstz_core::host::JsHostRuntime;
use jstz_core::kv::Transaction;
use jstz_crypto::hash::Hash;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use serde::Deserialize;
use std::ops::Deref;
use std::ops::DerefMut;

use crate::jstz_console::jstz_console;
use crate::jstz_kv::jstz_kv;
use crate::jstz_kv::kv::Kv;
use deno_console::deno_console;

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
    pub fn new(
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        address: SmartFunctionHash,
        options: Option<RuntimeOptions>,
    ) -> Self {
        let protocol = Protocol::new(hrt, tx, address);

        let mut runtime = JsRuntime::new(options.unwrap_or(Self::options()));

        let op_state = runtime.op_state();
        op_state.borrow_mut().put(protocol);

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

pub struct Protocol {
    pub host: JsHostRuntime<'static>,
    pub tx: &'static mut Transaction,
    pub kv: Kv,
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
        Protocol {
            host,
            tx,
            kv: Kv::new(address.to_base58()),
        }
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
    init_ops_and_esm_extensions!(deno_console, jstz_console, jstz_kv)
}

#[cfg(test)]
mod test {

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
}
