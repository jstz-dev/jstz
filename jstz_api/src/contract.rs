use boa_engine::{
    object::{FunctionObjectBuilder, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction, Source,
};
use boa_gc::{Finalize, Trace};
use jstz_core::{
    executor::Executor,
    realm::{Module, Realm},
};
use jstz_crypto::public_key_hash::PublicKeyHash;

use crate::{ledger::js_value_to_pkh, ConsoleApi, LedgerApi};

// Contract.call(contract_address, code)

#[derive(Finalize, Trace)]
struct Contract;

impl Contract {
    fn call(
        contract_address: PublicKeyHash,
        contract_code: String,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        // 1. Create a new realm for the contract
        let realm = Realm::new(context);

        // 2. Parse the contract
        let module =
            Module::parse(Source::from_bytes(&contract_code), Some(realm), context)?;

        // 3. Initialize apis
        module.realm().register_api(ConsoleApi, context);
        module
            .realm()
            .register_api(LedgerApi { contract_address }, context);
        module.realm().register_api(ContractApi, context);

        // 4. Evaluate the contract's module
        let promise = module.realm().eval_module(&module, context)?;

        // 5. Once evaluated, call the module's handler
        let result = promise.then(
            Some(
                FunctionObjectBuilder::new(context, unsafe {
                    NativeFunction::from_closure_with_captures(
                        |_, _, module, context| Executor::handle_request(module, context),
                        module,
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
        let contract_address = js_value_to_pkh(args.get_or_undefined(0))?;

        let contract_code =
            args.get_or_undefined(1)
                .as_string()
                .ok_or_else(|| {
                    JsError::from_native(JsNativeError::typ().with_message(
                        "Failed to convert js value into rust type `String`",
                    ))
                })?
                .to_std_string_escaped();

        Contract::call(contract_address, contract_code, context)
    }
}

impl jstz_core::realm::Api for ContractApi {
    fn init(self, context: &mut Context<'_>) {
        let contract = ObjectInitializer::with_native(Contract, context)
            .function(NativeFunction::from_fn_ptr(Self::call), "call", 1)
            .build();

        context
            .register_global_property(Self::NAME, contract, Attribute::all())
            .expect("The contract object shouldn't exist yet")
    }
}
