#![allow(unused)]
use std::{cell::RefCell, rc::Rc};

use bincode::{Decode, Encode};
use deno_core::v8;
use deno_error::{JsErrorBox, JsErrorClass};
use derive_more::{Deref, DerefMut, Display};
use jstz_runtime::{sys::FromV8, JstzRuntime, JstzRuntimeOptions};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Clone,
    Debug,
    Default,
    Deref,
    Display,
    Encode,
    Eq,
    Decode,
    Deserialize,
    PartialEq,
    Serialize,
    ToSchema,
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

impl TryFrom<String> for ParsedCode {
    type Error = crate::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Ok(ParsedCode::parse(value).map_err(super::Error::from)?)
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

    /**
     * TODO: Support caching
     * https://linear.app/tezos/issue/JSTZ-649/
     * ```
     * let unbounded = module.get_unbound_module_script(scope);
     * unbounded.create_code_cache()
     * ...
     * ```
     */
    /// Parses the given JavaScript code. Checks that the code is valid JavaScript
    /// syntax, compiles into an ES Module, and checks that the module has a default
    /// export handler
    ///
    /// Note that even if code is parsable, it may not be a valid smart function if it
    /// does not have the correct argument and return types
    pub fn parse(code: String) -> Result<ParsedCode> {
        let mut runtime = JstzRuntime::new(JstzRuntimeOptions {
            // Explicitly switch off protocol
            protocol: None,
            ..Default::default()
        });
        let scope = &mut runtime.handle_scope();

        let script_origin = script_origin(scope, "code".to_string()).unwrap();
        let local_code = v8::String::new(scope, &code).unwrap();
        let mut source =
            v8::script_compiler::Source::new(local_code, Some(&script_origin));

        let tc_scope = &mut v8::TryCatch::new(scope);

        // Compiles the source code, catching syntax and reference errors
        let module = v8::script_compiler::compile_module(tc_scope, &mut source);
        if tc_scope.has_caught() {
            let exception = tc_scope.exception().unwrap();
            return Err(CompileModuleError::from_caught(tc_scope, exception)?.into());
        }

        let module = module.unwrap();

        // By creating a scope from the previous scope, we tell rust that
        // &module will outlive handles created in the new scope.
        let scope = &mut v8::HandleScope::new(tc_scope);

        instantiate_module(scope, &module)?;

        // Runs top level module code. Top level attempted protocol access
        // and unbounded loops are caught here
        let _ = module.evaluate(scope);
        check_error_status(scope, &module)?;

        let ns = module.get_module_namespace();
        has_valid_default_export(scope, ns)?;

        Ok(ParsedCode(code))
    }
}

/// Flag to detect import
#[derive(Deref, DerefMut)]
struct ImportDetected(bool);

/// Instantiation normally involves resolving module dependencies but since Jstz
/// only supports bundled modules, we error if module resolution is requested
fn instantiate_module<'s>(
    scope: &mut v8::HandleScope<'s>,
    module: &v8::Local<v8::Module>,
) -> Result<()> {
    let mut scope = v8::HandleScope::new(scope);

    // Validate that we are not in error state
    check_error_status(&mut scope, module)?;

    // Use scope slot (aka annexed memory) to pass import buffer to callback
    let import_detected = Rc::new(RefCell::new(ImportDetected(false)));
    scope.set_slot(import_detected.clone());

    let result = module.instantiate_module(&mut scope, resolve_module_callback);
    if result.is_none() {
        let has_imports = import_detected.borrow();
        if has_imports.0 {
            return Err(ParseError::ImportsNotSupported);
        } else {
            return Err(ParseError::InstantiationFailed);
        }
    }
    Ok(())
}

/// See [`v8::ResolveModuleCallback`]
fn resolve_module_callback<'s>(
    context: v8::Local<'s, v8::Context>,
    specifier: v8::Local<'s, v8::String>,
    import_attributes: v8::Local<'s, v8::FixedArray>,
    referrer: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
    // # Safety: Appropriate usage. See CallbackScope doc
    let scope = &mut unsafe { v8::CallbackScope::new(context) };
    let specifier = specifier.to_rust_string_lossy(scope);
    if let Some(import_detected) = scope.get_slot_mut::<Rc<RefCell<ImportDetected>>>() {
        **import_detected.borrow_mut() = true;
    };
    None
}

fn script_origin<'s>(
    scope: &mut v8::HandleScope<'s>,
    resource_name: String,
) -> Option<v8::ScriptOrigin<'s>> {
    let resource_name = v8::String::new(scope, &resource_name)?;
    Some(v8::ScriptOrigin::new(
        // scope
        scope,
        // resource_name
        resource_name.into(),
        // resource_line_offset
        0,
        // resource_column_offset
        0,
        // resource_is_shared_cross_origin
        false,
        // script_id
        0,
        // source_map_url
        None,
        // resource_is_opaque
        false,
        // is_wasm
        false,
        // is_module
        true,
        // host_defined_options
        None,
    ))
}

fn has_valid_default_export(
    scope: &mut v8::HandleScope,
    ns: v8::Local<v8::Value>,
) -> Result<()> {
    let ns_object: v8::Local<v8::Object> =
        ns.try_cast().map_err(|_| ParseError::NotNsObject)?;
    let default_str = v8::String::new_external_onebyte_static(scope, b"default").unwrap();
    if let Some(default_handler) = ns_object.get(scope, default_str.into()) {
        if default_handler.is_null_or_undefined() {
            Err(ParseError::NoDefaultHandler)
        } else if !default_handler.is_function() {
            Err(ParseError::NonCallableDefault(
                default_handler.type_repr().to_string(),
            ))
        } else {
            Ok(())
        }
    } else {
        Err(ParseError::NoDefaultHandler)
    }
}

fn check_error_status<'s>(
    scope: &mut v8::HandleScope<'s>,
    module: &'s v8::Local<'s, v8::Module>,
) -> Result<()> {
    if let v8::ModuleStatus::Errored = module.get_status() {
        let exception = module.get_exception();
        return Err(ParseError::CompileModuleError(
            CompileModuleError::from_exception(scope, exception)?,
        ));
    }
    Ok(())
}

type Result<T> = std::result::Result<T, ParseError>;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ParseError {
    #[class("CompileModuleError")]
    #[error(transparent)]
    CompileModuleError(#[from] CompileModuleError),

    #[class(type)]
    #[error("Not a module namespace object")]
    NotNsObject,

    #[class(type)]
    #[error("No default handler")]
    NoDefaultHandler,

    #[class(type)]
    #[error(
        "Module namespace object expected 'default' property to be function but was {0}"
    )]
    NonCallableDefault(String),

    #[class(inherit)]
    #[error(transparent)]
    Other(#[from] jstz_runtime::error::RuntimeError),

    #[class(not_supported)]
    #[error("Import specifiers are not supported")]
    ImportsNotSupported,

    #[class(generic)]
    #[error("Failed to instantiate module")]
    InstantiationFailed,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(inherit)]
#[error("{}: {}", .0.get_class(), .0.get_message())]
pub struct CompileModuleError(JsErrorBox);

impl CompileModuleError {
    fn from_exception<'s>(
        scope: &mut v8::HandleScope<'s>,
        value: v8::Local<'s, v8::Value>,
    ) -> Result<Self> {
        let js_error = deno_core::error::JsError::from_v8_exception(scope, value);
        let js_error_box = JsErrorBox::new(
            js_error.name.unwrap_or("Error".to_string()),
            js_error.exception_message,
        );
        Ok(CompileModuleError(js_error_box))
    }

    fn from_caught<'s>(
        scope: &mut v8::HandleScope<'s>,
        value: v8::Local<'s, v8::Value>,
    ) -> Result<Self> {
        let js_error = jstz_runtime::sys::error::Error::from_v8(scope, value)?;
        let class = js_error.name(scope)?;
        let message = js_error.message(scope)?;
        let js_error_box = JsErrorBox::new(class, message);
        Ok(CompileModuleError(js_error_box))
    }
}

#[cfg(test)]
mod test {
    use deno_error::JsErrorClass;

    use super::*;

    #[test]
    fn parse_valid_smart_function() {
        let code = "export default () => 42";
        let parsed_code = ParsedCode::parse(code.to_string()).unwrap();
        assert_eq!(parsed_code.as_str(), code);

        let code = "export default async () => 42";
        let parsed_code = ParsedCode::parse(code.to_string()).unwrap();
        assert_eq!(parsed_code.as_str(), code);
    }

    #[test]
    // See https://v8.dev/features/top-level-await
    // Since promises are eagerly executed and long computation is subject to the usual
    // gas boundaries (WIP), TLAs should be ok. The documentation makes a note on possible
    // non-determinism when TLA is mixed with resolving module dependencies but since Jstz
    // doesn't support module dependencies, we should be ok here too
    fn parse_top_level_async_succeeds() {
        let code = r#"
            const lazy = async () => {};
            (async () => await lazy())();
            export default () => 42
            (async () => await lazy())();
        "#;
        let parsed_code = ParsedCode::parse(code.to_string()).unwrap();
        assert_eq!(parsed_code.0, code);
    }

    #[test]
    fn parse_invalid_js_fails() {
        let code = "invalid js";
        let error = ParsedCode::parse(code.to_string()).unwrap_err();
        assert!(matches!(error, ParseError::CompileModuleError(_)));
        assert_eq!(error.get_class(), "CompileModuleError");
        assert_eq!(
            error.get_message(),
            "SyntaxError: Unexpected identifier 'js'"
        );

        let code = "export default () => return new Response()";
        let error = ParsedCode::parse(code.to_string()).unwrap_err();
        assert!(matches!(error, ParseError::CompileModuleError(_)));
        assert_eq!(error.get_class(), "CompileModuleError");
        assert_eq!(
            error.get_message(),
            "SyntaxError: Unexpected token 'return'"
        );
    }

    #[test]
    fn parse_empty_js_fails() {
        let code = "";
        let error = ParsedCode::parse(code.to_string()).unwrap_err();
        assert!(matches!(error, ParseError::NoDefaultHandler));
        assert_eq!(error.get_class(), "TypeError");
        assert_eq!(error.get_message(), "No default handler");
    }

    #[test]
    fn parse_no_default_export_fails() {
        let code = "export const foo = () => 42";
        let error = ParsedCode::parse(code.to_string()).unwrap_err();
        assert!(matches!(error, ParseError::NoDefaultHandler));
        assert_eq!(error.get_class(), "TypeError");
        assert_eq!(error.get_message(), "No default handler");
    }

    #[test]
    fn parse_non_callable_default_fails() {
        let code = "const foo = 42; export default foo;";
        let error = ParsedCode::parse(code.to_string()).unwrap_err();
        println!("{:?}", error);
        assert!(matches!(error, ParseError::NonCallableDefault(_)));
        assert_eq!(error.get_class(), "TypeError");
        assert_eq!(error.get_message(), "Module namespace object expected 'default' property to be function but was Number");
    }

    #[test]
    fn parse_with_imports_fails() {
        let code = r#"
            import { foo } from "foo";
            export default () => 42
        "#;
        let error = ParsedCode::parse(code.to_string()).unwrap_err();
        println!("{:?}", error);
        assert!(matches!(error, ParseError::ImportsNotSupported));
        assert_eq!(error.get_class(), "NotSupported");
        assert_eq!(error.get_message(), "Import specifiers are not supported");
    }

    #[test]
    fn parse_with_attempted_protocol_access_fails() {
        let code = r#"
            Kv.set("test", "test")
            let t = Kv.get("test")
            export default () => 42
        "#;
        let error = ParsedCode::parse(code.to_string()).unwrap_err();
        assert!(matches!(
            error,
            ParseError::CompileModuleError(CompileModuleError(_))
        ));
        assert_eq!(error.get_class(), "CompileModuleError");
        assert_eq!(
            error.get_message(),
            "NotSupported: Uncaught NotSupported: Kv is not supported"
        );
    }

    #[test]
    fn parse_throw_string_literal_fails() {
        let code = r#"
        throw "just a string";
        export default () => 42;
    "#;

        let error = ParsedCode::parse(code.to_string()).unwrap_err();

        assert!(matches!(error, ParseError::CompileModuleError(_)));
        assert_eq!(error.get_class(), "CompileModuleError");
        assert_eq!(error.get_message(), "Error: Uncaught just a string");
    }
}
