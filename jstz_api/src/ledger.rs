use std::ops::{Deref, DerefMut};

use boa_engine::{
    object::ObjectInitializer, property::Attribute, Context, JsArgs, JsNativeError,
    JsResult, JsString, JsValue, NativeFunction,
};
use boa_gc::{Finalize, Trace};
use tezos_smart_rollup_host::runtime::Runtime;

use jstz_core::{host::{self, Host}, host_defined, kv::Transaction};
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_ledger::account::{Account, Amount};

use crate::error::Result;

// Ledger.balance(pkh)
// Ledger.transfer(src, dst, amount)

#[derive(Trace, Finalize)]
struct Ledger;

impl Ledger {
    fn balance(
        rt: &impl Runtime,
        tx: &mut Transaction,
        public_key_hash: PublicKeyHash,
    ) -> Result<u64> {
        let balance = Account::balance(rt, tx, public_key_hash)?;

        Ok(balance)
    }

    fn transfer(
        rt: &impl Runtime,
        tx: &mut Transaction,
        src: PublicKeyHash,
        dst: PublicKeyHash,
        amount: Amount,
    ) -> Result<()> {
        Account::transfer(rt, tx, src, dst, amount)?;

        Ok(())
    }
}

pub struct LedgerApi;

fn js_value_to_pkh(value: &JsValue) -> Result<PublicKeyHash> {
    let pkh_string = value
        .as_string()
        .ok_or_else(|| {
            JsNativeError::typ()
                .with_message("Failed to convert js value into rust type `String`")
        })
        .map(JsString::to_std_string_escaped)?;

    Ok(PublicKeyHash::from_base58(&pkh_string)?)
}

impl LedgerApi {
    const NAME: &'static str = "Ledger";

    fn balance<H: Runtime + 'static>(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        host_defined!(context, host_defined);
        let rt = host_defined.get::<Host<H>>().unwrap();
        let mut tx = host_defined.get_mut::<Transaction>().unwrap();

        let pkh = js_value_to_pkh(args.get_or_undefined(0))?;

        let balance = Ledger::balance(rt.deref(), tx.deref_mut(), pkh)?;

        Ok(balance.into())
    }

    fn transfer<H: Runtime + 'static>(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        host_defined!(context, host_defined);
        let rt = host_defined.get::<Host<H>>().unwrap();
        let mut tx = host_defined.get_mut::<Transaction>().unwrap();

        let src = js_value_to_pkh(args.get_or_undefined(0))?;
        let dst = js_value_to_pkh(args.get_or_undefined(1))?;
        let amount = args
            .get_or_undefined(2)
            .as_number()
            .ok_or_else(|| JsNativeError::typ())?;

        Ledger::transfer(rt.deref(), tx.deref_mut(), src, dst, amount as Amount)?;

        Ok(JsValue::undefined())
    }
}

impl jstz_core::host::Api for LedgerApi {
    fn init<H: Runtime + 'static>(context: &mut boa_engine::Context<'_>) {
        let ledger = ObjectInitializer::with_native(Ledger, context)
            .function(
                NativeFunction::from_fn_ptr(Self::balance::<H>),
                "balance",
                1,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::transfer::<H>),
                "transfer",
                3,
            )
            .build();

        context
            .register_global_property(Self::NAME, ledger, Attribute::all())
            .expect("The ledger object shouldn't exist yet");
    }
}
