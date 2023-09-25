use std::ops::DerefMut;

use boa_engine::{
    object::{FunctionObjectBuilder, Object, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction, Source,
};
use either::Either;
use jstz_api::http::request::Request;
use jstz_core::{host_defined, kv::Transaction, runtime};
use jstz_crypto::public_key_hash::PublicKeyHash;

use crate::{
    api::ledger::js_value_to_pkh, context::account::Account, executor::contract::Script,
};

use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};
struct Contract {
    contract_address: PublicKeyHash,
}
impl Finalize for Contract {}

unsafe impl Trace for Contract {
    empty_trace!();
}

impl Contract {
    fn from_js_value<'a>(value: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `Ledger`")
                    .into()
            })
    }
    fn _call(
        contract_address: PublicKeyHash,
        contract_code: &String,
        request: &JsValue,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let script = Script::parse(Source::from_bytes(contract_code), context)?;

        // 4. Evaluate the contract's module
        let script_promise = script.init(contract_address, context)?;

        // 5. Once evaluated, call the module's handler
        let result = script_promise.then(
            Some(
                FunctionObjectBuilder::new(context, unsafe {
                    NativeFunction::from_closure_with_captures(
                        |_, _, (script, request), context| script.run(request, context),
                        (script, request.clone()),
                    )
                })
                .build(),
            ),
            None,
            context,
        )?;

        Ok(result.into())
    }
    fn call(&self, tx: &mut Transaction, request: &Request) -> JsResult<JsValue> {
        let contract_address = request
            .url()
            .domain()
            .and_then(|domain| PublicKeyHash::from_base58(domain).ok())
            .ok_or_else(|| {
                JsError::from_native(JsNativeError::error().with_message("Invalid host"))
            })?;
        crate::executor::contract::run::execute_js(
            tx,
            contract_address,
            Either::Right(request.clone()),
            &self.contract_address,
        )
    }
}

pub struct ContractApi {
    pub contract_address: PublicKeyHash,
}

impl ContractApi {
    const NAME: &'static str = "Contract";

    fn _call(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        host_defined!(context, host_defined);
        let mut tx = host_defined
            .get_mut::<Transaction>()
            .expect("Curent transaction undefined");
        let contract_address = js_value_to_pkh(args.get_or_undefined(0))?;

        let contract_code = runtime::with_global_host(|rt| {
            Account::contract_code(rt, tx.deref_mut(), &contract_address)
        })?
        .ok_or_else(|| {
            JsNativeError::eval().with_message(format!(
                "No code associated with address: {contract_address}"
            ))
        })?;
        let request = args.get_or_undefined(1);

        Contract::_call(contract_address, contract_code, request, context)
    }
    fn call(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        host_defined!(context, host_defined);
        let mut tx = host_defined
            .get_mut::<Transaction>()
            .expect("Curent transaction undefined");

        let request =
            args.get_or_undefined(0)
                .as_object()
                .and_then(|obj| obj.downcast_mut::<Request>())
                .ok_or_else(|| {
                    JsError::from_native(JsNativeError::typ().with_message(
                        "Failed to convert js value into rust type `Request`",
                    ))
                })?;
        let contract = Contract::from_js_value(this)?;
        contract.call(&mut tx, &request)
    }
}

impl jstz_core::Api for ContractApi {
    fn init(self, context: &mut Context<'_>) {
        let contract = ObjectInitializer::with_native(
            Contract {
                contract_address: self.contract_address,
            },
            context,
        )
        .function(NativeFunction::from_fn_ptr(Self::call), "call", 2)
        .build();

        context
            .register_global_property(Self::NAME, contract, Attribute::all())
            .expect("The contract object shouldn't exist yet")
    }
}
