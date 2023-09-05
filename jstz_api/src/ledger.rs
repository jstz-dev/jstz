use std::ops::{Deref, DerefMut};

use boa_engine::{
    object::{Object, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsNativeError, JsResult, JsString, JsValue, NativeFunction,
};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};
use tezos_smart_rollup_host::runtime::Runtime;

use jstz_core::{host_defined, kv::Transaction};
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_ledger::account::{Account, Amount};

use crate::error::Result;

// Ledger.selfAddress()
// Ledger.balance(pkh)
// Ledger.transfer(dst, amount)

struct Ledger {
    contract_address: PublicKeyHash,
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
        rt: &impl Runtime,
        tx: &mut Transaction,
        public_key_hash: &PublicKeyHash,
    ) -> Result<u64> {
        let balance = Account::balance(rt, tx, public_key_hash)?;

        Ok(balance)
    }

    fn transfer(
        &self,
        rt: &impl Runtime,
        tx: &mut Transaction,
        dst: &PublicKeyHash,
        amount: Amount,
    ) -> Result<()> {
        Account::transfer(rt, tx, &self.contract_address, dst, amount)?;

        Ok(())
    }
}

pub struct LedgerApi {
    pub contract_address: PublicKeyHash,
}

pub(crate) fn js_value_to_pkh(value: &JsValue) -> Result<PublicKeyHash> {
    let pkh_string = value
        .as_string()
        .ok_or_else(|| {
            JsNativeError::typ()
                .with_message("Failed to convert js value into rust type `String`")
        })
        .map(JsString::to_std_string_escaped)?;

    Ok(PublicKeyHash::from_base58(&pkh_string)?)
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
        jstz_core::runtime::with_global_host(|rt| {
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
        jstz_core::runtime::with_global_host(|rt| {
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
}

impl jstz_core::realm::Api for LedgerApi {
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
        .build();

        context
            .register_global_property(Self::NAME, ledger, Attribute::all())
            .expect("The ledger object shouldn't exist yet");
    }
}
