use std::ops::DerefMut;

use boa_engine::{
    js_string,
    object::{builtins::JsPromise, Object, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
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

use crate::{
    context::account::{Account, Address, Amount},
    executor::contract::{headers, Script},
    operation::OperationHash,
    Error, Result,
};

use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};

pub struct TraceData {
    pub contract_address: Address,
    pub operation_hash: OperationHash,
}

impl Finalize for TraceData {}

unsafe impl Trace for TraceData {
    empty_trace!();
}

struct SmartFunction {
    address: Address,
}
impl Finalize for SmartFunction {}

unsafe impl Trace for SmartFunction {
    empty_trace!();
}

impl SmartFunction {
    fn from_js_value(value: &JsValue) -> JsResult<GcRefMut<'_, Object, Self>> {
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
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        contract_code: String,
        initial_balance: Amount,
    ) -> Result<String> {
        // 1. Check if the contract has sufficient balance
        {
            let balance =
                Account::balance(hrt, tx, &self.address).expect("Could not get balance");
            if balance < initial_balance {
                return Err(Error::BalanceOverflow);
            }
        } // The mutable borrow of `tx` in `balance` is released here

        // 2. Deploy the contract
        let address =
            Script::deploy(hrt, tx, &self.address, contract_code, initial_balance)?; // The mutable borrow of `tx` in `Script::deploy` is released here

        // 3. Increment nonce of current account
        {
            let nonce = Account::nonce(hrt, tx, &self.address)?;
            nonce.increment();
        } // The mutable borrow of `tx` in `Account::nonce` is released here

        // 4. Transfer the balance to the contract
        Account::transfer(hrt, tx, &self.address, &address, initial_balance)?;

        Ok(address.to_string())
    }

    fn call(
        self_address: &Address,
        tx: &mut Transaction,
        request: &JsNativeObject<Request>,
        operation_hash: OperationHash,
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
        headers::test_and_set_referrer(&request.deref(), self_address)?;

        // 3. Load, init and run!
        Script::load_init_run(tx, address, operation_hash, request.inner(), context)
    }
}

pub struct ContractApi {
    pub address: Address,
}

impl ContractApi {
    const NAME: &'static str = "SmartFunction";

    fn fetch(
        address: &Address,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        host_defined!(context, host_defined);
        let mut tx = host_defined
            .get_mut::<Transaction>()
            .expect("Curent transaction undefined");
        let trace_data = host_defined
            .get::<TraceData>()
            .expect("trace data undefined");

        let request: JsNativeObject<Request> =
            args.get_or_undefined(0).clone().try_into()?;

        SmartFunction::call(
            address,
            tx.deref_mut(),
            &request,
            trace_data.operation_hash.clone(),
            context,
        )
    }

    fn call(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let smart_function = SmartFunction::from_js_value(this)?;
        Self::fetch(&smart_function.address, args, context)
    }

    fn create(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        host_defined!(context, host_defined);
        let mut tx = host_defined.get_mut::<Transaction>().unwrap();

        let contract = SmartFunction::from_js_value(this)?;
        let contract_code: String = args
            .get(0)
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Expected at least 1 argument but 0 provided")
            })?
            .try_js_into(context)?;

        let initial_balance = match args.get(1) {
            None => 0,
            Some(balance) => balance
                .to_big_uint64(context)?
                .iter_u64_digits()
                .next()
                .unwrap_or_default(),
        };

        let promise = JsPromise::new(
            move |resolvers, context| {
                let address = runtime::with_global_host(|rt| {
                    contract.create(rt, &mut tx, contract_code, initial_balance as Amount)
                })?;

                resolvers.resolve.call(
                    &JsValue::undefined(),
                    &[address.into_js(context)],
                    context,
                )?;
                Ok(JsValue::undefined())
            },
            context,
        )?;

        Ok(promise.into())
    }
}

impl jstz_core::Api for ContractApi {
    fn init(self, context: &mut Context<'_>) {
        let contract = ObjectInitializer::with_native(
            SmartFunction {
                address: self.address.clone(),
            },
            context,
        )
        .function(
            NativeFunction::from_fn_ptr(Self::call),
            js_string!("call"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(Self::create),
            js_string!("create"),
            1,
        )
        .build();

        context
            .register_global_property(js_string!(Self::NAME), contract, Attribute::all())
            .expect("The smart function object shouldn't exist yet");

        context
            .register_global_builtin_callable(
                js_string!("fetch"),
                2,
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
