use deno_core::error::CoreError;
use deno_core::*;
use jstz_core::host::HostRuntime;
use jstz_core::host::JsHostRuntime;
use jstz_core::kv::Transaction;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use serde::Deserialize;
use std::ops::Deref;
use std::ops::DerefMut;

use crate::jstz_console::jstz_console;
use crate::jstz_fetch::jstz_fetch;
use deno_console::deno_console;
use deno_core::anyhow::Result as AnyhowResult;
use deno_core::error::JsError;
use deno_core::url::Url;
use deno_fetch::FetchPermissions;
use std::path::Path;

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

pub struct JstzDenoFetchPermissions;

impl FetchPermissions for JstzDenoFetchPermissions {
    fn check_net_url(&mut self, _url: &Url, _api_name: &str) -> AnyhowResult<()> {
        Ok(())
    }
    fn check_read(&mut self, _p: &Path, _api_name: &str) -> AnyhowResult<()> {
        Ok(())
    }
}

fn init_extenions() -> Vec<Extension> {
    /*deno_fetch::deno_fetch::init_ops_and_esm::<JstzDenoFetchPermissions>(
        Default::default(),
    );
    init_ops_and_esm_extensions!(deno_console, jstz_console, jstz_fetch)*/

    vec![
        deno_fetch::deno_fetch::init_ops_and_esm::<JstzDenoFetchPermissions>(
            Default::default(),
        ),
        deno_console::init_ops_and_esm(),
        jstz_console::init_ops_and_esm(),
        jstz_fetch::init_ops_and_esm(),
    ]
}

#[cfg(test)]
mod test {
    use jstz_core::kv::Transaction;
    use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};

    use crate::{test_utils::init_mock_host, JstzRuntime};

    #[test]
    fn test_init_jstz_runtime() {
        let address =
            SmartFunctionHash::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton")
                .unwrap();
        let (sink, mut host) = init_mock_host();
        let tx = &mut Transaction::default();
        tx.begin();
        let jstz_runtime = JstzRuntime::new(&mut host, tx, address);

        let code = r#"
            console.log("hello");
            console.log(42);
        "#;

        jstz_runtime.execute(code).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&sink),
            "[INFO] hello\n[INFO] \u{1b}[33m42\u{1b}[39m\n".to_string()
        )
    }
}
