use boa_engine::{
    object::{builtins::JsPromise, FunctionObjectBuilder},
    Context, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
};

use crate::{
    host_defined,
    kv::{Kv, Transaction},
    realm::Module,
    runtime,
};

fn get_default_export(module: &Module, context: &mut Context<'_>) -> JsResult<JsValue> {
    module.namespace(context).get("default", context)
}

fn invoke_handler(
    module: &Module,
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context<'_>,
) -> JsResult<JsValue> {
    let default_export = get_default_export(module, context)?;

    let handler = default_export.as_object().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message("Failed to convert `default` export to js object"),
        )
    })?;

    handler.call(this, args, context)
}

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

pub struct Executor;

impl Executor {
    /// Handles a request
    pub fn handle_request(
        module: &Module,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let context = &mut module.realm().context_handle(context);

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
        let result = invoke_handler(module, &JsValue::undefined(), &[], context)?;

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
