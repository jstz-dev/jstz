use std::io::Read;

use boa_engine::{
    object::{builtins::JsPromise, FunctionObjectBuilder},
    Context, JsError, JsNativeError, JsResult, JsValue, NativeFunction, Source,
};

use boa_gc::{Finalize, Trace};
use derive_more::{Deref, DerefMut};
use jstz_core::{
    host::HostRuntime,
    host_defined,
    kv::{Kv, Transaction},
    runtime, Module, Realm,
};
use jstz_crypto::public_key_hash::PublicKeyHash;
use tezos_smart_rollup::prelude::debug_msg;

use crate::{api, Result};

fn finally(
    value: JsValue,
    on_finally: fn(&mut Context<'_>),
    context: &mut Context<'_>,
) -> JsValue {
    match value.as_promise() {
        Some(promise) => {
            let promise = JsPromise::from_object(promise.clone()).unwrap();
            promise
                .finally(
                    FunctionObjectBuilder::new(context, unsafe {
                        NativeFunction::from_closure(move |_, _, context| {
                            on_finally(context);
                            Ok(JsValue::undefined())
                        })
                    })
                    .build(),
                    context,
                )
                .unwrap()
                .into()
        }
        None => {
            on_finally(context);
            value
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Deref, DerefMut, Trace, Finalize)]
pub struct Script(Module);

impl Script {
    fn get_default_export(&self, context: &mut Context<'_>) -> JsResult<JsValue> {
        self.namespace(context).get("default", context)
    }

    fn invoke_handler(
        &self,
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let default_export = self.get_default_export(context)?;

        let handler = default_export.as_object().ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ()
                    .with_message("Failed to convert `default` export to js object"),
            )
        })?;

        handler.call(this, args, context)
    }

    pub fn parse<R: Read>(
        src: Source<'_, R>,
        context: &mut Context<'_>,
    ) -> JsResult<Self> {
        let module = Module::parse(src, Some(Realm::new(context)), context)?;

        Ok(Self(module))
    }

    fn register_apis(&self, contract_address: PublicKeyHash, context: &mut Context<'_>) {
        self.realm().register_api(jstz_api::ConsoleApi, context);
        self.realm().register_api(
            jstz_api::KvApi {
                contract_address: contract_address.clone(),
            },
            context,
        );
        self.realm()
            .register_api(api::LedgerApi { contract_address }, context);
        self.realm().register_api(api::ContractApi, context);
        self.realm().register_api(jstz_api::url::UrlApi, context);
    }

    /// Initialize the script, registering all associated runtime APIs
    /// and evaluating the module of the script
    pub fn init(
        &self,
        contract_address: PublicKeyHash,
        context: &mut Context<'_>,
    ) -> JsResult<JsPromise> {
        self.register_apis(contract_address, context);

        self.realm().eval_module(&self, context)
    }

    /// Runs the script
    pub fn run(&self, context: &mut Context<'_>) -> JsResult<JsValue> {
        let context = &mut self.realm().context_handle(context);

        // 1. Register `Kv` and `Transaction` objects in `HostDefined`
        // FIXME: `Kv` and `Transaction` should be externally provided
        {
            host_defined!(context, mut host_defined);

            let kv = Kv::new();
            let tx = kv.begin_transaction();

            host_defined.insert(kv);
            host_defined.insert(tx);
        }

        // 2. Invoke the script's handler
        let result = self.invoke_handler(&JsValue::undefined(), &[], context)?;

        // 3. Ensure that the transaction is commit
        let result = finally(
            result,
            |context| {
                host_defined!(context, mut host_defined);

                runtime::with_global_host(|rt| {
                    let mut kv = host_defined
                        .remove::<Kv>()
                        .expect("Rust type `Kv` should be defined in `HostDefined`");

                    let tx = host_defined.remove::<Transaction>().expect(
                        "Rust type `Transaction` should be defined in `HostDefined`",
                    );

                    kv.commit_transaction(rt, *tx)
                        .expect("Failed to commit transaction");
                })
            },
            context,
        );

        Ok(result)
    }
}

pub mod run {

    use super::*;
    use crate::operation::RunContract;

    pub fn execute(
        hrt: &mut (impl HostRuntime + 'static),
        run: RunContract,
    ) -> Result<String> {
        let RunContract {
            contract_address,
            contract_code,
        } = run;

        debug_msg!(hrt, "Evaluating: {contract_code:?}\n");

        // 1. Initialize runtime
        let rt = &mut jstz_core::Runtime::new();

        let result: JsValue = runtime::with_host_runtime(hrt, || {
            // 2. Initialize script
            let script = Script::parse(Source::from_bytes(&contract_code), rt)
                .expect("Failed to parse script");

            let script_promise = script.init(contract_address, rt)?;

            // 3. Execute
            jstz_core::future::block_on(async move {
                rt.resolve_value(&script_promise.into())
                    .await
                    .expect("Failed to resolve script promise");

                let result = script.run(rt)?;

                rt.resolve_value(&result).await
            })
        })?;

        Ok(result.display().to_string())
    }
}
