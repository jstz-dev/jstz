use std::ops::{Deref, DerefMut};

use boa_engine::{
    js_string,
    object::{Object, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsNativeError, JsResult, JsString, JsValue, NativeFunction,
};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};

use jstz_core::{
    accessor, host::HostRuntime, host_defined, kv::Transaction, native::Accessor,
    runtime, value::IntoJs,
};

use crate::{
    context::account::{Account, Address, Amount},
    error::Result,
};

// Ledger.selfAddress
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
    fn try_from_js<'a>(value: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
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

    fn self_address(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Ledger,
            "selfAddress",
            get:((ledger, context) => Ok(ledger.self_address().into_js(context)))
        )
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

            let ledger = Ledger::try_from_js(this)?;
            let dst = js_value_to_pkh(args.get_or_undefined(0))?;
            let amount = args
                .get_or_undefined(1)
                .as_number()
                .ok_or_else(|| JsNativeError::typ())?;

            ledger.transfer(rt.deref(), tx.deref_mut(), &dst, amount as Amount)?;

            Ok(JsValue::undefined())
        })
    }
}

impl jstz_core::Api for LedgerApi {
    fn init(self, context: &mut boa_engine::Context<'_>) {
        let self_address = LedgerApi::self_address(context);

        let ledger = ObjectInitializer::with_native(
            Ledger {
                contract_address: self.contract_address,
            },
            context,
        )
        .accessor(
            js_string!(self_address.name),
            self_address.get,
            self_address.set,
            Attribute::all(),
        )
        .function(
            NativeFunction::from_fn_ptr(Self::balance),
            js_string!("balance"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(Self::transfer),
            js_string!("transfer"),
            3,
        )
        .build();

        context
            .register_global_property(js_string!(Self::NAME), ledger, Attribute::all())
            .expect("The ledger object shouldn't exist yet");
    }
}
