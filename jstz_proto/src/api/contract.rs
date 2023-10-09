use boa_engine::{
    js_string,
    object::{builtins::JsPromise, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
};

use jstz_api::http::request::Request;
use jstz_core::{api::Jstz, with_jstz};
use jstz_core::{host::HostRuntime, native::JsNativeObject, value::IntoJs};

use crate::{
    context::account::{Account, Address, Amount},
    executor::contract::{headers, Script},
    operation::external::ContractOrigination,
    receipt, Error, Result,
};

use boa_gc::{Finalize, Trace};
#[derive(Trace, Finalize)]
struct Contract;

impl Contract {
    fn create(
        jstz: &mut Jstz,
        hrt: &impl HostRuntime,
        contract_code: String,
        initial_balance: Amount,
    ) -> Result<String> {
        let addr = jstz.self_address().clone();
        let tx = jstz.transaction_mut();
        // 1. Check if the contract has sufficient balance
        if Account::balance(hrt, tx, &addr)? < initial_balance {
            return Err(Error::BalanceOverflow.into());
        }

        // 2. Deploy the contract
        let contract = ContractOrigination {
            contract_code,
            originating_address: addr.clone(),
            initial_balance,
        };
        let receipt::DeployContract { contract_address } =
            crate::executor::deploy_contract(hrt, tx, contract)?;

        // 3. Transfer the balance to the contract
        Account::transfer(hrt, tx, &addr, &contract_address, initial_balance)?;

        Ok(contract_address.to_string())
    }

    fn call(
        jstz: &Jstz,
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
        headers::test_and_set_referrer(&request.deref(), &jstz.self_address())?;

        // 3. Load, init and run!
        Script::load_init_run(
            jstz.contract_call_data(&address),
            &request.inner(),
            context,
        )
    }
}

pub struct Api;

impl Api {
    const NAME: &'static str = "Contract";

    fn call(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let request: JsNativeObject<Request> =
            args.get_or_undefined(0).clone().try_into()?;

        with_jstz!(context, [Contract::call](&jstz, &request, context))
    }
    fn create(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
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
                let address = with_jstz!(
                    context,
                    [Contract::create](
                        &mut jstz,
                        &mut hrt,
                        contract_code,
                        initial_balance
                    )
                )?;
                resolvers.resolve.call(
                    &JsValue::Undefined,
                    &[address.into_js(context)],
                    context,
                )?;
                Ok(JsValue::Undefined)
            },
            context,
        )?;
        Ok(promise.into())
        //        Ok()
    }
    /*
        host_defined!(context, host_defined);
        let mut tx = host_defined.get_mut::<Transaction>().unwrap();

        let contract = Contract::from_js_value(this)?;
        let contract_code: String = args
            .get(0)
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Expected at least 1 argument but 0 provided")
            })?
            .try_js_into(context)?;


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
    */
}

impl jstz_core::GlobalApi for Api {
    fn init(context: &mut Context) {
        let contract = ObjectInitializer::with_native(Contract, context)
            .function(
                NativeFunction::from_fn_ptr(Self::call),
                js_string!("call"),
                1,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::create),
                js_string!("create"),
                1,
            )
            .build();
        context
            .register_global_property(js_string!(Self::NAME), contract, Attribute::all())
            .expect("The contract object shouldn't exist yet")
    }
}
