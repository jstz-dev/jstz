use std::{
    fmt::{self, Display},
    result,
};

use bincode::{Decode, Encode};
use boa_engine::{
    js_string,
    object::{builtins::JsPromise, FunctionObjectBuilder},
    Context, JsError, JsNativeError, JsResult, JsValue, NativeFunction, Source,
};
use boa_gc::{Finalize, Trace};
use derive_more::{Deref, DerefMut};
use jstz_api::js_log::set_js_logger;
use jstz_core::{Module, Realm};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::js_logger::JsonLogger;

// Invariant: if code is present it parses successfully
#[derive(
    Default,
    PartialEq,
    Eq,
    Debug,
    Clone,
    Serialize,
    Deserialize,
    ToSchema,
    Encode,
    Decode,
    Deref,
)]
#[schema(
    format = "javascript",
    example = "export default (request) => new Response('Hello world!')"
)]
pub struct ParsedCode(pub String);

impl From<ParsedCode> for String {
    fn from(ParsedCode(code): ParsedCode) -> Self {
        code
    }
}

impl Display for ParsedCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> result::Result<(), fmt::Error> {
        Display::fmt(&self.0, formatter)
    }
}

impl TryFrom<String> for ParsedCode {
    type Error = crate::Error;

    fn try_from(code: String) -> crate::Result<Self> {
        let src = Source::from_bytes(code.as_bytes());
        let mut context = Context::default();
        Module::parse(src, None, &mut context)?;
        Ok(Self(code))
    }
}

impl ParsedCode {
    /// Creates a new `ParsedCode`.
    ///
    /// # Safety
    ///
    /// `code` must be well-formed JavaScript code
    pub unsafe fn new_unchecked(code: String) -> ParsedCode {
        ParsedCode(code)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
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
            Source::from_bytes(&src.0),
            Some(Realm::new(context)?),
            context,
        )?;
        Ok(Self(module))
    }

    pub fn realm(&self) -> &Realm {
        self.0.realm()
    }

    /// Initialize the script, evaluating its module.
    pub fn init(&self, context: &mut Context) -> JsPromise {
        self.realm().eval_module(self, context)
    }

    /// Runs the script
    pub fn run(&self, request: &JsValue, context: &mut Context) -> JsResult<JsValue> {
        // 3. Set logger
        set_js_logger(&JsonLogger);

        // 4. Invoke the script's handler
        self.invoke_handler(&JsValue::undefined(), &[request.clone()], context)
    }

    /// Loads, initializes and runs the script
    pub fn load_init_run<T: jstz_core::Api>(
        src: &ParsedCode,
        api: T,
        request: &JsValue,
        context: &mut Context,
    ) -> JsResult<JsValue> {
        // 1. Load script
        let script = Script::load(src, context)?;

        // 2. Register the API for the script's realm
        script.realm().register_api(api, context);

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
