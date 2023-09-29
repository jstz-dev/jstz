use std::ops::DerefMut;

use boa_engine::{
    object::{Object, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
};
use jstz_api::http::request::Request;
use jstz_core::{host_defined, kv::Transaction, native::JsNativeObject};

use crate::{
    context::account::Address,
    executor::contract::{headers, Script},
};

use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};
struct Contract {
    contract_address: Address,
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

    fn call(
        &self,
        tx: &mut Transaction,
        request: &JsNativeObject<Request>,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        // 1. Get address from request
        let address = request
            .deref()
            .url()
            .domain()
            .and_then(|domain| Address::from_base58(domain).ok())
            .ok_or_else(|| {
                JsError::from_native(JsNativeError::error().with_message("Invalid host"))
            })?;

        // 2. Set the referer of the request to the current contract address
        headers::test_and_set_referrer(&request.deref(), &self.contract_address)?;

        // 3. Load, init and run!
        Script::load_init_run(tx, &address, request.inner(), context)
    }
}

pub struct ContractApi {
    pub contract_address: Address,
}

impl ContractApi {
    const NAME: &'static str = "Contract";

    fn call(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        host_defined!(context, host_defined);
        let mut tx = host_defined
            .get_mut::<Transaction>()
            .expect("Curent transaction undefined");

        let contract = Contract::from_js_value(this)?;
        let request: JsNativeObject<Request> =
            args.get_or_undefined(0).clone().try_into()?;

        contract.call(tx.deref_mut(), &request, context)
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
