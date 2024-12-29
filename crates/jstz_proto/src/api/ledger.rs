use std::ops::Deref;

use boa_engine::{
    js_string,
    object::{ErasedObject, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsData, JsNativeError, JsResult, JsString, JsValue, NativeFunction,
};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};

use jstz_core::{
    accessor, host::HostRuntime, kv::Transaction, native::Accessor, runtime,
    value::IntoJs,
};
use jstz_crypto::hash::JstzHash;

use crate::{
    context::account::{Account, Address, Amount},
    error::Result,
};

// Ledger.selfAddress
// Ledger.balance(pkh)
// Ledger.transfer(dst, amount)

#[derive(JsData)]
struct Ledger {
    address: Address,
}

impl Finalize for Ledger {}

unsafe impl Trace for Ledger {
    empty_trace!();
}

impl Ledger {
    fn self_address(&self) -> String {
        self.address.to_string()
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
        Account::transfer(rt, tx, &self.address, dst, amount)?;

        Ok(())
    }
}

pub struct LedgerApi {
    pub address: Address,
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
    fn try_from_js(value: &JsValue) -> JsResult<GcRefMut<'_, ErasedObject, Self>> {
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

    fn self_address(context: &mut Context) -> Accessor {
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
        _context: &mut Context,
    ) -> JsResult<JsValue> {
        let pkh = js_value_to_pkh(args.get_or_undefined(0))?;

        let balance = runtime::with_js_hrt_and_tx(|hrt, tx| {
            Ledger::balance(hrt.deref(), tx, &pkh)
        })?;

        Ok(balance.into())
    }

    fn transfer(
        this: &JsValue,
        args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<JsValue> {
        let ledger = Ledger::try_from_js(this)?;
        let dst = js_value_to_pkh(args.get_or_undefined(0))?;
        let amount = args
            .get_or_undefined(1)
            .as_number()
            .ok_or_else(JsNativeError::typ)?;

        runtime::with_js_hrt_and_tx(|hrt, tx| {
            ledger.transfer(hrt.deref(), tx, &dst, amount as Amount)
        })?;

        Ok(JsValue::undefined())
    }
}

impl jstz_core::Api for LedgerApi {
    fn init(self, context: &mut boa_engine::Context) {
        let self_address = LedgerApi::self_address(context);

        let ledger = ObjectInitializer::with_native_data(
            Ledger {
                address: self.address,
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
