use std::ops::Deref;

use boa_engine::{
    js_string, object::ObjectInitializer, Context, JsArgs, JsNativeError, JsObject,
    JsResult, JsValue, NativeFunction,
};
use jstz_core::{host_defined, kv::Transaction, runtime};
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::context::account::Account;

macro_rules! preamble {
    ($args:ident, $context:ident, $tx:ident) => {
        host_defined!($context, host_defined);
        let mut $tx = host_defined
            .get_mut::<Transaction>()
            .expect("Curent transaction undefined");
    };
}

fn get_public_key_hash(account: &str) -> JsResult<PublicKeyHash> {
    PublicKeyHash::from_base58(account).map_err(|_| {
        JsNativeError::typ()
            .with_message("Could not parse the address.")
            .into()
    })
}

pub struct AccountApi;

impl AccountApi {
    fn balance(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble!(args, context, tx);

        let account: String = args.get_or_undefined(0).try_js_into(context)?;

        let pkh = get_public_key_hash(account.as_str())?;

        let result =
            runtime::with_global_host(|rt| Account::balance(rt.deref(), &mut tx, &pkh))?;

        Ok(JsValue::from(result))
    }

    fn set_balance(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble!(args, context, tx);

        let account: String = args.get_or_undefined(0).try_js_into(context)?;

        let balance: u64 = args.get_or_undefined(1).try_js_into(context)?;

        let pkh = get_public_key_hash(account.as_str())?;

        runtime::with_global_host(|rt| {
            Account::set_balance(rt.deref(), &mut tx, &pkh, balance)
        })?;
        Ok(JsValue::undefined())
    }

    fn code(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble!(args, context, tx);

        let account: String = args.get_or_undefined(0).try_js_into(context)?;

        let pkh = get_public_key_hash(account.as_str())?;

        let result = runtime::with_global_host(|rt| {
            Account::contract_code(rt.deref(), &mut tx, &pkh)
        })?;

        match result {
            Some(value) => Ok(JsValue::String(value.to_string().into())),
            None => Ok(JsValue::null()),
        }
    }

    fn set_code(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble!(args, context, tx);

        let account: String = args.get_or_undefined(0).try_js_into(context)?;
        let code: String = args.get_or_undefined(1).try_js_into(context)?;

        let pkh = get_public_key_hash(account.as_str())?;

        runtime::with_global_host(|rt| {
            Account::set_contract_code(rt.deref(), &mut tx, &pkh, code)
        })?;

        Ok(JsValue::undefined())
    }

    pub fn namespace(context: &mut boa_engine::Context<'_>) -> JsObject {
        let storage = ObjectInitializer::new(context)
            .function(
                NativeFunction::from_fn_ptr(Self::balance),
                js_string!("balance"),
                1,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::set_balance),
                js_string!("setBalance"),
                2,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::code),
                js_string!("code"),
                1,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::set_code),
                js_string!("setCode"),
                2,
            )
            .build();

        storage
    }
}
