use std::ops::DerefMut;

use boa_engine::{
    object::{builtins::JsPromise, Object, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
};
use jstz_api::http::request::Request;
use jstz_core::{
    host::HostRuntime, host_defined, kv::Transaction, native::JsNativeObject, runtime,
};

use crate::{
    context::account::{Account, Address, Amount},
    executor::contract::{headers, Script},
    operation::external::ContractOrigination,
    receipt, Error, Result,
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

    fn create(
        &self,
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        contract_code: String,
        initial_balance: Amount,
    ) -> Result<String> {
        // 1. Check if the contract has sufficient balance
        if Account::balance(hrt, tx, &self.contract_address)? < initial_balance {
            return Err(Error::BalanceOverflow.into());
        }

        // 2. Deploy the contract
        let contract = ContractOrigination {
            contract_code,
            originating_address: self.contract_address.clone(),
            initial_balance,
        };
        let receipt::DeployContract { contract_address } =
            crate::executor::deploy_contract(hrt, tx, contract)?;

        // 3. Transfer the balance to the contract
        Account::transfer(
            hrt,
            tx,
            &self.contract_address,
            &contract_address,
            initial_balance,
        )?;

        Ok(contract_address.to_string())
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

    fn create(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
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
                    &[address.into()],
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
            Contract {
                contract_address: self.contract_address,
            },
            context,
        )
        .function(NativeFunction::from_fn_ptr(Self::call), "call", 2)
        .function(NativeFunction::from_fn_ptr(Self::create), "create", 1)
        .build();

        context
            .register_global_property(Self::NAME, contract, Attribute::all())
            .expect("The contract object shouldn't exist yet")
    }
}
