//! `jstz`'s implementation of JavaScript's `console` Web API object.
//!
//! The `console` object can be accessed from any global object.
//!
//! The specifics of how it works varies from browser to browser, but there is a de facto set of features that are typically provided.
//!
//! More information:
//!  - [MDN documentation][mdn]
//!  - [WHATWG `console` specification][spec]
//!
//! [spec]: https://console.spec.whatwg.org/
//! [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Console
//!
//! The implementation is heavily inspired by https://github.com/boa-dev/boa/blob/main/boa_runtime/src/console/mod.rs

use std::ops::Deref;

use boa_engine::{
    js_string,
    object::{Object, ObjectInitializer},
    property::Attribute,
    value::Numeric,
    Context, JsArgs, JsNativeError, JsResult, JsValue, NativeFunction,
};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};
use jstz_core::{host::HostRuntime, runtime, value::IntoJs};
use jstz_crypto::{hash::Blake2b, public_key_hash::PublicKeyHash};
use serde::{Deserialize, Serialize};
use serde_json;
use tezos_smart_rollup::prelude::debug_msg;

pub const LOG_PREFIX: &str = "[JSTZ:SMART_FUNCTION:LOG] ";

#[derive(Serialize, Deserialize)]
pub struct LogRecord {
    pub contract_address: PublicKeyHash,
    request_id: String,
    level: String,
    text: String,
}

impl LogRecord {
    fn new(
        contract_address: PublicKeyHash,
        request_id: String,
        level: String,
        text: String,
    ) -> Self {
        Self {
            contract_address,
            request_id,
            level,
            text,
        }
    }

    fn to_string(&self) -> String {
        serde_json::to_string(&self).expect("Failed to serialize JSONLog")
    }

    pub fn from_string(json: &str) -> Self {
        serde_json::from_str(&json).expect("Failed to deserialize JSONLog")
    }
}

/// This represents the different types of log messages.
#[derive(Debug)]
enum LogMessage {
    Log(String),
    Info(String),
    Warn(String),
    Error(String),
}

impl LogMessage {
    fn log(self, rt: &impl HostRuntime, console: &Console) {
        match console {
            Console::Proto {
                groups,
                contract_address,
                operation_hash,
            } => {
                let indent = 2 * groups.len();
                let log_record = LogRecord::new(
                    contract_address.clone(),
                    operation_hash.to_string(),
                    self.level().to_string(),
                    " ".repeat(indent) + self.message(),
                )
                .to_string();
                rt.write_debug(&(LOG_PREFIX.to_string() + &log_record + "\n"));
            }
            Console::Cli { groups } => {
                let indent = 2 * groups.len();
                let symbol = self.symbol();
                for line in self.message().lines() {
                    debug_msg!(rt, "[{symbol}] {:>indent$}{line}\n", "");
                }
            }
        }
    }

    fn message(&self) -> &str {
        match self {
            LogMessage::Error(msg) => msg,
            LogMessage::Warn(msg) => msg,
            LogMessage::Info(msg) => msg,
            LogMessage::Log(msg) => msg,
        }
    }

    fn symbol(&self) -> char {
        match self {
            LogMessage::Error(_) => '🔴',
            LogMessage::Warn(_) => '🟠',
            LogMessage::Info(_) => '🟢',
            LogMessage::Log(_) => '🪵',
        }
    }

    fn level(&self) -> &str {
        match self {
            LogMessage::Error(_) => "error",
            LogMessage::Warn(_) => "warn",
            LogMessage::Info(_) => "info",
            LogMessage::Log(_) => "log",
        }
    }
}

fn display_js(value: &JsValue) -> String {
    match value.as_string() {
        Some(value) => value.to_std_string_escaped(),
        None => value.display().to_string(),
    }
}
/// This represents the `console` formatter.
///
/// More information:
///  - [WHATWG `formatter` specification][https://console.spec.whatwg.org/#formatter]
fn formatter(data: &[JsValue], context: &mut Context<'_>) -> JsResult<String> {
    match data {
        [] => Ok(String::new()),
        [val] => Ok(display_js(val)),
        data => {
            let mut formatted = String::new();
            let mut arg_index = 0;
            if let Some(target) = data
                .get_or_undefined(0)
                .as_string()
                .map(|x| x.to_std_string_escaped())
            {
                arg_index = 1;
                let mut chars = target.chars();
                while let Some(c) = chars.next() {
                    if c == '%' {
                        let fmt = chars.next().unwrap_or('%');
                        match fmt {
                            /* integer */
                            'd' | 'i' => {
                                let arg = match data
                                    .get_or_undefined(arg_index)
                                    .to_numeric(context)?
                                {
                                    Numeric::Number(r) => (r.floor() + 0.0).to_string(),
                                    Numeric::BigInt(int) => int.to_string(),
                                };
                                formatted.push_str(&arg);
                                arg_index += 1;
                            }
                            /* float */
                            'f' => {
                                let arg = data
                                    .get_or_undefined(arg_index)
                                    .to_number(context)?;
                                formatted.push_str(&format!("{arg:.6}"));
                                arg_index += 1;
                            }
                            /* object */
                            'o' | 'O' => {
                                let arg = data.get_or_undefined(arg_index);
                                formatted.push_str(&arg.display().to_string());
                                arg_index += 1;
                            }
                            /* string */
                            's' => {
                                let arg = data
                                    .get_or_undefined(arg_index)
                                    .to_string(context)?
                                    .to_std_string_escaped();
                                formatted.push_str(&arg);
                                arg_index += 1;
                            }
                            '%' => formatted.push('%'),
                            c => {
                                formatted.push('%');
                                formatted.push(c);
                            }
                        }
                    } else {
                        formatted.push(c);
                    }
                }
            }

            /* unformatted data */
            for rest in data.iter().skip(arg_index) {
                formatted.push_str(&format!(" {}", display_js(rest)));
            }

            Ok(formatted)
        }
    }
}

#[derive(Finalize)]
enum Console {
    // Json log
    Proto {
        groups: Vec<String>,
        // TODO: Remove these once `Jstz` object is implemented
        contract_address: PublicKeyHash,
        operation_hash: Blake2b,
    },
    // pretty log
    Cli {
        groups: Vec<String>,
    },
}

unsafe impl Trace for Console {
    empty_trace!();
}

impl Console {
    fn groups(&mut self) -> &mut Vec<String> {
        match self {
            Console::Proto { groups, .. } => groups,
            Console::Cli { groups } => groups,
        }
    }
    /// `console.clear()`
    ///
    /// Removes all groups and clears console if possible.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///  - [WHATWG `console` specification][spec]
    ///
    /// [spec]: https://console.spec.whatwg.org/#clear
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/console/clear
    fn clear(&mut self) {
        self.groups().clear()
    }

    /// `console.assert(condition, ...data)`
    ///
    /// Prints a JavaScript value to the standard error if first argument evaluates to `false` or there
    /// were no arguments.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///  - [WHATWG `console` specification][spec]
    ///
    /// [spec]: https://console.spec.whatwg.org/#assert
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/console/assert
    fn assert(
        &self,
        assertion: bool,
        data: &[JsValue],
        rt: &impl HostRuntime,
        context: &mut Context<'_>,
    ) -> JsResult<()> {
        if !assertion {
            let mut args: Vec<JsValue> = Vec::from(data);
            let message = "Assertion failed".to_string();
            if args.is_empty() {
                args.push(message.into_js(context));
            } else if !args[0].is_string() {
                args.insert(0, message.into_js(context));
            } else {
                let concat = format!("{message}: {}", args[0].display());
                args[0] = concat.into_js(context);
            }

            LogMessage::Error(formatter(&args, context)?).log(rt, self)
        }

        Ok(())
    }

    /// `console.debug(...data)`
    ///
    /// Prints a JavaScript values with "debug" logLevel.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///  - [WHATWG `console` specification][spec]
    ///
    /// [spec]: https://console.spec.whatwg.org/#debug
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/console/debug
    fn debug(
        &self,
        data: &[JsValue],
        rt: &impl HostRuntime,
        context: &mut Context<'_>,
    ) -> JsResult<()> {
        LogMessage::Log(formatter(data, context)?).log(rt, self);
        Ok(())
    }

    /// `console.warn(...data)`
    ///
    /// Prints a JavaScript values with "warn" logLevel.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///  - [WHATWG `console` specification][spec]
    ///
    /// [spec]: https://console.spec.whatwg.org/#warn
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/console/warn
    fn warn(
        &self,
        data: &[JsValue],
        rt: &impl HostRuntime,
        context: &mut Context<'_>,
    ) -> JsResult<()> {
        LogMessage::Warn(formatter(data, context)?).log(rt, self);
        Ok(())
    }

    /// `console.error(...data)`
    ///
    /// Prints a JavaScript values with "error" logLevel.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///  - [WHATWG `console` specification][spec]
    ///
    /// [spec]: https://console.spec.whatwg.org/#error
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/console/error
    fn error(
        &self,
        data: &[JsValue],
        rt: &impl HostRuntime,
        context: &mut Context<'_>,
    ) -> JsResult<()> {
        LogMessage::Error(formatter(data, context)?).log(rt, self);
        Ok(())
    }

    /// `console.info(...data)`
    ///
    /// Prints a JavaScript values with "info" logLevel.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///  - [WHATWG `console` specification][spec]
    ///
    /// [spec]: https://console.spec.whatwg.org/#info
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/console/info
    fn info(
        &self,
        data: &[JsValue],
        rt: &impl HostRuntime,
        context: &mut Context<'_>,
    ) -> JsResult<()> {
        LogMessage::Info(formatter(data, context)?).log(rt, self);
        Ok(())
    }

    /// `console.log(...data)`
    ///
    /// Prints a JavaScript values with "log" logLevel.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///  - [WHATWG `console` specification][spec]
    ///
    /// [spec]: https://console.spec.whatwg.org/#log
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/console/log
    fn log(
        &self,
        data: &[JsValue],
        rt: &impl HostRuntime,
        context: &mut Context<'_>,
    ) -> JsResult<()> {
        LogMessage::Log(formatter(data, context)?).log(rt, self);
        Ok(())
    }

    /// `console.group(...data)`
    ///
    /// Adds new group with name from formatted data to stack.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///  - [WHATWG `console` specification][spec]
    ///
    /// [spec]: https://console.spec.whatwg.org/#group
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/console/group
    fn group(
        &mut self,
        data: &[JsValue],
        rt: &impl HostRuntime,
        context: &mut Context<'_>,
    ) -> JsResult<()> {
        let group_label = formatter(data, context)?;
        LogMessage::Log(format!("group: {group_label}")).log(rt, self);
        self.groups().push(group_label);
        Ok(())
    }

    /// `console.groupEnd()`
    ///
    /// Removes the last group from the stack.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///  - [WHATWG `console` specification][spec]
    ///
    /// [spec]: https://console.spec.whatwg.org/#groupend
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/console/groupEnd
    fn group_end(&mut self) {
        self.groups().pop();
    }
}

/// `ConsoleApi` implements `jstz_core::host::Api`, permitting it to be registered
/// as a `jstz` runtime API.  
pub enum ConsoleApi {
    // TODO: remove this once `Jstz` object is implemented
    Proto {
        contract_address: PublicKeyHash,
        operation_hash: Blake2b,
    },
    Cli,
}

impl Console {
    fn from_js_value<'a>(value: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `Console`")
                    .into()
            })
    }
}

macro_rules! variadic_console_function {
    ($name:ident) => {
        fn $name(
            this: &JsValue,
            args: &[JsValue],
            context: &mut Context<'_>,
        ) -> JsResult<JsValue> {
            let console = Console::from_js_value(this)?;

            runtime::with_global_host(|rt| {
                console.$name(args, rt.deref(), context)?;
                Ok(JsValue::undefined())
            })
        }
    };
}

impl ConsoleApi {
    const NAME: &'static str = "console";

    variadic_console_function!(log);
    variadic_console_function!(error);
    variadic_console_function!(debug);
    variadic_console_function!(warn);
    variadic_console_function!(info);

    fn assert(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let console = Console::from_js_value(this)?;
        runtime::with_global_host(|rt| {
            let assertion = args.get_or_undefined(0).to_boolean();
            let data = if args.len() >= 1 { &args[1..] } else { &[] };
            console.assert(assertion, data, rt.deref(), context)?;

            Ok(JsValue::undefined())
        })
    }

    fn group(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut console = Console::from_js_value(this)?;
        runtime::with_global_host(|rt| {
            console.group(args, rt, context)?;
            Ok(JsValue::undefined())
        })
    }

    fn group_end(
        this: &JsValue,
        _args: &[JsValue],
        _context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut console = Console::from_js_value(this)?;

        console.group_end();
        Ok(JsValue::undefined())
    }

    fn clear(
        this: &JsValue,
        _args: &[JsValue],
        _context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut console = Console::from_js_value(this)?;
        console.clear();
        Ok(JsValue::undefined())
    }

    fn to_console(self) -> Console {
        match self {
            ConsoleApi::Proto {
                contract_address,
                operation_hash,
            } => Console::Proto {
                groups: Vec::default(),
                contract_address,
                operation_hash,
            },
            ConsoleApi::Cli => Console::Cli {
                groups: Vec::default(),
            },
        }
    }
}

impl jstz_core::Api for ConsoleApi {
    fn init(self, context: &mut Context<'_>) {
        let console = ObjectInitializer::with_native(self.to_console(), context)
            .function(NativeFunction::from_fn_ptr(Self::log), js_string!("log"), 0)
            .function(
                NativeFunction::from_fn_ptr(Self::error),
                js_string!("error"),
                0,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::debug),
                js_string!("debug"),
                0,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::warn),
                js_string!("warn"),
                0,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::info),
                js_string!("info"),
                0,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::assert),
                js_string!("assert"),
                0,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::group),
                js_string!("group"),
                0,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::group),
                js_string!("groupCollapsed"),
                0,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::group_end),
                js_string!("groupEnd"),
                0,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::clear),
                js_string!("clear"),
                0,
            )
            .build();

        context
            .register_global_property(js_string!(Self::NAME), console, Attribute::all())
            .expect("console api should only be registered once!")
    }
}
