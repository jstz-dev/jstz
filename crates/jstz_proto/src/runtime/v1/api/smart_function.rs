use boa_engine::{
    js_string,
    object::{builtins::JsPromise, ErasedObject, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsData, JsNativeError, JsResult, JsValue, NativeFunction,
};

use jstz_api::http::request::Request;
use jstz_core::{
    host::HostRuntime,
    host_defined,
    kv::Transaction,
    native::JsNativeObject,
    runtime::{self},
    value::IntoJs,
};
use jstz_crypto::smart_function_hash::SmartFunctionHash;

use crate::{
    context::account::{Account, Amount},
    executor::smart_function,
    operation::{DeployFunction, OperationHash},
    runtime::v1::{fetch_handler, ProtocolData},
    runtime::ParsedCode,
    Result,
};

use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};

#[derive(JsData)]
struct SmartFunction {
    address: SmartFunctionHash,
}
impl Finalize for SmartFunction {}

unsafe impl Trace for SmartFunction {
    empty_trace!();
}

impl SmartFunction {
    fn from_js_value(value: &JsValue) -> JsResult<GcRefMut<'_, ErasedObject, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message(
                        "Failed to convert js value into rust type `SmartFunction`",
                    )
                    .into()
            })
    }

    fn create(
        &self,
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        function_code: ParsedCode,
        initial_balance: Amount,
    ) -> Result<String> {
        // 1. Deploy the smart function
        let deploy_receipt = smart_function::deploy::execute(
            hrt,
            tx,
            &self.address,
            DeployFunction {
                function_code,
                account_credit: initial_balance,
            },
        )?;

        // 2. Increment nonce of current account
        Account::nonce(hrt, tx, &self.address)?.increment();

        Ok(deploy_receipt.address.to_string())
    }

    // Invariant: The function should always be called within a js_host_context
    fn call(
        self_address: &SmartFunctionHash,
        request: &JsNativeObject<Request>,
        operation_hash: OperationHash,
        context: &mut Context,
    ) -> JsResult<JsValue> {
        fetch_handler::fetch(self_address, operation_hash, request, context)
    }
}

pub struct SmartFunctionApi {
    pub address: SmartFunctionHash,
}

impl SmartFunctionApi {
    const NAME: &'static str = "SmartFunction";

    fn fetch(
        address: &SmartFunctionHash,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        host_defined!(context, host_defined);
        let proto_data = host_defined
            .get::<ProtocolData>()
            .expect("trace data undefined");

        let request: JsNativeObject<Request> =
            args.get_or_undefined(0).clone().try_into()?;

        SmartFunction::call(
            address,
            &request,
            proto_data.operation_hash.clone(),
            context,
        )
    }

    fn call(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let smart_function = SmartFunction::from_js_value(this)?;
        Self::fetch(&smart_function.address, args, context)
    }

    fn create(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let smart_function = SmartFunction::from_js_value(this)?;

        let function_code: String = args
            .first()
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Expected at least 1 argument but 0 provided")
            })?
            .try_js_into(context)?;
        let parsed_code: ParsedCode = function_code.try_into()?;

        let initial_balance = match args.get(1) {
            None => 0,
            Some(balance) => balance.to_big_uint64(context)?,
        };

        let promise = JsPromise::new(
            move |resolvers, context| {
                let address = runtime::with_js_hrt_and_tx(|hrt, tx| {
                    smart_function.create(hrt, tx, parsed_code, initial_balance as Amount)
                })?;

                resolvers.resolve.call(
                    &JsValue::undefined(),
                    &[address.into_js(context)],
                    context,
                )?;
                Ok(JsValue::undefined())
            },
            context,
        );

        Ok(promise.into())
    }
}

impl jstz_core::Api for SmartFunctionApi {
    fn init(self, context: &mut Context) {
        let smart_function = ObjectInitializer::with_native_data(
            SmartFunction {
                address: self.address.clone(),
            },
            context,
        )
        .function(
            NativeFunction::from_fn_ptr(Self::call),
            js_string!("call"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(Self::create),
            js_string!("create"),
            2,
        )
        .build();

        context
            .register_global_property(
                js_string!(Self::NAME),
                smart_function,
                Attribute::all(),
            )
            .expect("The smart function object shouldn't exist yet");

        context
            .register_global_builtin_callable(
                js_string!("fetch"),
                1,
                NativeFunction::from_copy_closure_with_captures(
                    |_, args, this, ctx| Self::fetch(&this.address, args, ctx),
                    SmartFunction {
                        address: self.address,
                    },
                ),
            )
            .expect("The fetch function shouldn't exist yet");
    }
}

#[cfg(test)]
mod test {

    use http::Method;
    use jstz_api::http::request::{Request, RequestClass};
    use jstz_core::{
        kv::Transaction,
        native::JsNativeObject,
        runtime::{self, with_js_hrt_and_tx},
        Runtime,
    };
    use jstz_crypto::{
        hash::{Blake2b, Hash},
        public_key_hash::PublicKeyHash,
        smart_function_hash::SmartFunctionHash,
    };
    use jstz_mock::host::JstzMockHost;
    use serde_json::json;

    use crate::{
        context::account::{Account, Address},
        runtime::v1::api::WebApi,
    };

    use super::SmartFunction;

    #[test]
    fn call_system_script_succeeds() {
        let mut mock_host = JstzMockHost::default();
        let rt = mock_host.rt();

        let mut jstz_rt = Runtime::new(10000).unwrap();
        let realm = jstz_rt.realm().clone();
        let context = jstz_rt.context();

        realm.register_api(WebApi, context);

        let self_address = SmartFunctionHash::digest(b"random bytes").unwrap();

        let amount = 100;

        let operation_hash = Blake2b::from(b"operation_hash".as_ref());
        let receiver = Address::User(PublicKeyHash::digest(b"receiver address").unwrap());
        let http_request = http::Request::builder()
            .method(Method::POST)
            .uri("jstz://jstz/withdraw")
            .header("Content-type", "application/json")
            .body(Some(
                json!({
                    "receiver": receiver,
                    "amount": 100
                })
                .to_string()
                .as_bytes()
                .to_vec(),
            ))
            .unwrap();

        let request = Request::from_http_request(http_request, context).unwrap();

        let mut tx = Transaction::default();
        runtime::enter_js_host_context(rt, &mut tx, || {
            with_js_hrt_and_tx(|hrt, tx| {
                tx.begin();
                Account::add_balance(hrt, tx, &self_address, amount).unwrap();
                tx.commit(hrt).unwrap();
            });

            SmartFunction::call(
                &self_address,
                &JsNativeObject::new::<RequestClass>(request, context).unwrap(),
                operation_hash,
                context,
            )
            .unwrap();
        });
    }
}
