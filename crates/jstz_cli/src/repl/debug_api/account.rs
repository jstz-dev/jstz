use std::ops::Deref;

use boa_engine::{
    js_string, object::ObjectInitializer, Context, JsArgs, JsNativeError, JsObject,
    JsResult, JsValue, NativeFunction,
};
use jstz_core::runtime;
use jstz_proto::context::{new_account::Account, new_account::NewAddress};

fn try_parse_address(account: &str) -> JsResult<NewAddress> {
    NewAddress::from_base58(account).map_err(|_| {
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
        let account: String = args.get_or_undefined(0).try_js_into(context)?;

        let address = try_parse_address(account.as_str())?;

        let result = runtime::with_js_hrt_and_tx(|hrt, tx| {
            Account::balance(hrt.deref(), tx, &address)
        })?;

        Ok(JsValue::from(result))
    }

    fn set_balance(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let account: String = args.get_or_undefined(0).try_js_into(context)?;

        let balance: u64 = args.get_or_undefined(1).try_js_into(context)?;

        let address = try_parse_address(account.as_str())?;

        runtime::with_js_hrt_and_tx(|hrt, tx| {
            Account::set_balance(hrt.deref(), tx, &address, balance)
        })?;
        Ok(JsValue::undefined())
    }

    fn code(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let account: String = args.get_or_undefined(0).try_js_into(context)?;

        let address = try_parse_address(account.as_str())?;

        runtime::with_js_hrt_and_tx(|hrt, tx| -> JsResult<JsValue> {
            match Account::function_code(hrt.deref(), tx, &address)? {
                "" => Ok(JsValue::null()),
                value => Ok(JsValue::String(value.to_string().into())),
            }
        })
    }

    fn set_code(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let account: String = args.get_or_undefined(0).try_js_into(context)?;
        let code: String = args.get_or_undefined(1).try_js_into(context)?;

        let address = try_parse_address(account.as_str())?;

        runtime::with_js_hrt_and_tx(|hrt, tx| {
            Account::set_function_code(hrt.deref(), tx, &address, code)
        })?;

        Ok(JsValue::undefined())
    }

    pub fn namespace(context: &mut boa_engine::Context) -> JsObject {
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

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_TZ1: &str = "tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU";
    const TEST_KT1: &str = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5";

    #[test]
    fn test_try_parse_address() {
        // Test valid tz1 address
        let result = try_parse_address(TEST_TZ1).unwrap();
        assert!(matches!(result, NewAddress::User(_)));
        assert_eq!(result.to_base58(), TEST_TZ1);

        // Test valid KT1 address
        let result = try_parse_address(TEST_KT1).unwrap();
        assert!(matches!(result, NewAddress::SmartFunction(_)));
        assert_eq!(result.to_base58(), TEST_KT1);
    }

    #[test]
    fn test_try_parse_address_invalid() {
        // Test empty string
        assert!(try_parse_address("").is_err());

        // Test invalid format
        assert!(try_parse_address("invalid").is_err());
        assert!(try_parse_address("tz1invalid").is_err());
        assert!(try_parse_address("KT1invalid").is_err());

        // Test invalid checksum
        let invalid_tz1 = "tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjV"; // Changed last char
        assert!(try_parse_address(invalid_tz1).is_err());

        let invalid_kt1 = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU6"; // Changed last char
        assert!(try_parse_address(invalid_kt1).is_err());
    }
}
