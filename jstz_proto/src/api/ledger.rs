use std::ops::{Deref, DerefMut};

use boa_engine::{
    object::{Object, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsNativeError, JsResult, JsString, JsValue, NativeFunction,
};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};

use jstz_core::{host::HostRuntime, host_defined, kv::Transaction, runtime};

use crate::{
    context::account::{Account, Address, Amount},
    error::Result,
    operation::external::ContractOrigination,
    receipt, Error,
};

// Ledger.selfAddress()
// Ledger.balance(pkh)
// Ledger.transfer(dst, amount)

struct Ledger {
    contract_address: Address,
}

impl Finalize for Ledger {}

unsafe impl Trace for Ledger {
    empty_trace!();
}

impl Ledger {
    fn self_address(&self) -> String {
        self.contract_address.to_string()
    }

    fn balance(
        rt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &Address,
    ) -> Result<u64> {
        let balance = Account::balance(rt, tx, addr)?;

        Ok(balance)
    }

    fn transfer(
        &self,
        rt: &impl HostRuntime,
        tx: &mut Transaction,
        dst: &Address,
        amount: Amount,
    ) -> Result<()> {
        Account::transfer(rt, tx, &self.contract_address, dst, amount)?;

        Ok(())
    }
    fn create_contract(
        &self,
        rt: &impl HostRuntime,
        tx: &mut Transaction,
        contract_code: String,
        initial_balance: Amount,
    ) -> JsResult<String> {
        if Self::balance(rt, tx, &self.contract_address)? < initial_balance {
            return Err(Error::BalanceOverflow.into());
        }
        let contract = ContractOrigination {
            contract_code,
            originating_address: self.contract_address.clone(),
            initial_balance,
        };
        let receipt::DeployContract { contract_address } =
            crate::executor::deploy_contract(rt, tx, contract)?;
        self.transfer(rt, tx, &contract_address, initial_balance)?;
        Ok(contract_address.to_string())
    }
}

pub struct LedgerApi {
    pub contract_address: Address,
}

pub(crate) fn js_value_to_pkh(value: &JsValue) -> Result<Address> {
    let pkh_string = value
        .as_string()
        .ok_or_else(|| {
            JsNativeError::typ()
                .with_message("Failed to convert js value into rust type `String`")
        })
        .map(JsString::to_std_string_escaped)?;

    Ok(Address::from_base58(&pkh_string)?)
}

impl Ledger {
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
}

impl LedgerApi {
    const NAME: &'static str = "Ledger";

    fn self_address(
        this: &JsValue,
        _args: &[JsValue],
        _context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let ledger = Ledger::from_js_value(this)?;

        Ok(ledger.self_address().into())
    }

    fn balance(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        runtime::with_global_host(|rt| {
            host_defined!(context, host_defined);

            let mut tx = host_defined.get_mut::<Transaction>().unwrap();

            let pkh = js_value_to_pkh(args.get_or_undefined(0))?;

            let balance = Ledger::balance(rt.deref(), tx.deref_mut(), &pkh)?;

            Ok(balance.into())
        })
    }

    fn transfer(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        runtime::with_global_host(|rt| {
            host_defined!(context, host_defined);
            let mut tx = host_defined.get_mut::<Transaction>().unwrap();

            let ledger = Ledger::from_js_value(this)?;
            let dst = js_value_to_pkh(args.get_or_undefined(0))?;
            let amount = args
                .get_or_undefined(1)
                .as_number()
                .ok_or_else(|| JsNativeError::typ())?;

            ledger.transfer(rt.deref(), tx.deref_mut(), &dst, amount as Amount)?;

            Ok(JsValue::undefined())
        })
    }
    fn create_contract(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        runtime::with_global_host(|rt| {
            host_defined!(context, host_defined);
            let mut tx = host_defined.get_mut::<Transaction>().unwrap();

            let ledger = Ledger::from_js_value(this)?;
            let contract_code = args
                .get(0)
                .ok_or_else(|| {
                    JsNativeError::typ()
                        .with_message("Expected at least 1 argument but 0 provided")
                })?
                .to_string(context)?
                .to_std_string_escaped();
            let initial_balance = args.get_or_undefined(1);
            let initial_balance = if initial_balance.is_undefined() {
                0
            } else {
                initial_balance
                    .to_big_uint64(context)?
                    .iter_u64_digits()
                    .next()
                    .unwrap_or_default()
            };

            let address = ledger.create_contract(
                rt,
                &mut tx,
                contract_code,
                initial_balance as Amount,
            )?;
            Ok(address.into())
        })
    }
}

impl jstz_core::Api for LedgerApi {
    fn init(self, context: &mut boa_engine::Context<'_>) {
        let ledger = ObjectInitializer::with_native(
            Ledger {
                contract_address: self.contract_address,
            },
            context,
        )
        .function(
            NativeFunction::from_fn_ptr(Self::self_address),
            "selfAddress",
            0,
        )
        .function(NativeFunction::from_fn_ptr(Self::balance), "balance", 1)
        .function(NativeFunction::from_fn_ptr(Self::transfer), "transfer", 3)
        .function(
            NativeFunction::from_fn_ptr(Self::create_contract),
            "createContract",
            1,
        )
        .build();

        context
            .register_global_property(Self::NAME, ledger, Attribute::all())
            .expect("The ledger object shouldn't exist yet");
    }
}
