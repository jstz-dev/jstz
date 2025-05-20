use std::ops::BitXor;

use boa_engine::{
    js_string,
    object::{builtins::JsPromise, FunctionObjectBuilder},
    Context, JsError, JsNativeError, JsResult, JsValue, NativeFunction, Source,
};
use boa_gc::{Finalize, Trace};
use derive_more::{Deref, DerefMut};
use jstz_api::js_log::set_js_logger;
use jstz_core::{host_defined, Module, Realm};
use jstz_crypto::{
    hash::{Blake2b, Hash},
    smart_function_hash::SmartFunctionHash,
};

use crate::{
    context::account::ParsedCode,
    js_logger::JsonLogger,
    operation::OperationHash,
    runtime::{ProtocolApi, ProtocolData},
};

fn compute_seed(address: &SmartFunctionHash, operation_hash: &OperationHash) -> u64 {
    let mut seed: u64 = 0;
    for byte in operation_hash.as_array().iter().chain(address.as_bytes()) {
        seed = seed.rotate_left(8).bitxor(*byte as u64)
    }

    seed
}

pub(crate) fn register_http_api(realm: &Realm, context: &mut Context) {
    realm.register_api(jstz_api::http::HttpApi, context);
}

pub fn register_web_apis(realm: &Realm, context: &mut Context) {
    realm.register_api(jstz_api::url::UrlApi, context);
    realm.register_api(jstz_api::urlpattern::UrlPatternApi, context);
    realm.register_api(jstz_api::http::HttpApi, context);
    realm.register_api(jstz_api::encoding::EncodingApi, context);
    realm.register_api(jstz_api::ConsoleApi, context);
    realm.register_api(jstz_api::file::FileApi, context);
}

pub fn register_jstz_apis(
    realm: &Realm,
    address: &SmartFunctionHash,
    _seed: u64,
    context: &mut Context,
) {
    realm.register_api(
        ProtocolApi {
            address: address.clone(),
            operation_hash: Blake2b::from(b"fake_op_hash".as_ref()),
        },
        context,
    );
}

#[derive(Debug, PartialEq, Eq, Clone, Deref, DerefMut, Trace, Finalize)]
pub struct Script(Module);

impl Script {
    fn get_default_export(&self, context: &mut Context) -> JsResult<JsValue> {
        self.namespace(context).get(js_string!("default"), context)
    }

    fn invoke_handler(
        &self,
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let default_export = self.get_default_export(context)?;

        let handler = default_export.as_object().ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ()
                    .with_message("Failed to convert `default` export to js object"),
            )
        })?;

        handler.call(this, args, context)
    }

    pub fn load(src: &ParsedCode, context: &mut Context) -> JsResult<Self> {
        let module = Module::parse(
            Source::from_bytes(src.as_bytes()),
            Some(Realm::new(context)?),
            context,
        )?;
        Ok(Self(module))
    }

    fn register_apis(
        &self,
        address: &SmartFunctionHash,
        operation_hash: &OperationHash,
        context: &mut Context,
    ) {
        register_web_apis(self.realm(), context);
        register_jstz_apis(
            self.realm(),
            address,
            compute_seed(address, operation_hash),
            context,
        );

        let context = &mut self.realm().context_handle(context);

        host_defined!(context, mut host_defined);
        host_defined.insert(ProtocolData {
            address: address.clone(),
            operation_hash: operation_hash.clone(),
        });
    }

    /// Initialize the script, registering all associated runtime APIs
    /// and evaluating the module of the script
    pub fn init(&self, context: &mut Context) -> JsPromise {
        self.realm().eval_module(self, context)
    }

    /// Runs the script
    pub fn run(&self, request: &JsValue, context: &mut Context) -> JsResult<JsValue> {
        set_js_logger(&JsonLogger);
        self.invoke_handler(&JsValue::undefined(), &[request.clone()], context)
    }

    /// Loads, initializes and runs the script
    pub fn load_init_run(
        src: &ParsedCode,
        address: SmartFunctionHash,
        operation_hash: OperationHash,
        request: &JsValue,
        context: &mut Context,
    ) -> JsResult<JsValue> {
        // 1. Load script
        let script = Self::load(src, context)?;

        // 2. Register the APIs for the script's realm
        script.register_apis(&address, &operation_hash, context);

        // 3. Evaluate the script's module
        let script_promise = script.init(context);

        // 4. Once evaluated, call the script's handler
        let result = script_promise.then(
            Some(
                FunctionObjectBuilder::new(context.realm(), unsafe {
                    NativeFunction::from_closure_with_captures(
                        |_, _, (script, request), context| script.run(request, context),
                        (script, request.clone()),
                    )
                })
                .build(),
            ),
            None,
            context,
        );

        Ok(result.into())
    }
}
