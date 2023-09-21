use std::ops::DerefMut;

use boa_engine::{
    object::{FunctionObjectBuilder, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsNativeError, JsResult, JsValue, NativeFunction, Source,
};
use boa_gc::{Finalize, Trace};
use jstz_core::{host_defined, kv::Transaction, runtime};
use jstz_crypto::public_key_hash::PublicKeyHash;

use crate::{
    api::ledger::js_value_to_pkh, context::account::Account, executor::contract::Script,
};

#[derive(Finalize, Trace)]
struct Contract;

impl Contract {
    fn call(
        contract_address: PublicKeyHash,
        contract_code: &String,
        request: &JsValue,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let script = Script::parse(Source::from_bytes(contract_code), context)?;

        // 4. Evaluate the contract's module
        let script_promise = script.init(contract_address, context)?;

        // 5. Once evaluated, call the module's handler
        let result = script_promise.then(
            Some(
                FunctionObjectBuilder::new(context, unsafe {
                    NativeFunction::from_closure_with_captures(
                        |_, _, (script, request), context| script.run(request, context),
                        (script, request.clone()),
                    )
                })
                .build(),
            ),
            None,
            context,
        )?;

        Ok(result.into())
    }
}

pub struct ContractApi;

impl ContractApi {
    const NAME: &'static str = "Contract";

    fn call(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        host_defined!(context, host_defined);
        let mut tx = host_defined
            .get_mut::<Transaction>()
            .expect("Curent transaction undefined");
        let contract_address = js_value_to_pkh(args.get_or_undefined(0))?;

        let contract_code = runtime::with_global_host(|rt| {
            Account::contract_code(rt, tx.deref_mut(), &contract_address)
        })?
        .ok_or_else(|| {
            JsNativeError::eval().with_message(format!(
                "No code associated with address: {contract_address}"
            ))
        })?;
        let request = args.get_or_undefined(1);

        Contract::call(contract_address, contract_code, request, context)
    }
}

impl jstz_core::Api for ContractApi {
    fn init(self, context: &mut Context<'_>) {
        let contract = ObjectInitializer::with_native(Contract, context)
            .function(NativeFunction::from_fn_ptr(Self::call), "call", 2)
            .build();

        context
            .register_global_property(Self::NAME, contract, Attribute::all())
            .expect("The contract object shouldn't exist yet")
    }
}
