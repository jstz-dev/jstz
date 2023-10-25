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
use boa_gc::{Finalize, GcRefMut, Trace};
use itertools::Itertools;
use jstz_core::{host::HostRuntime, runtime, value::IntoJs};

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
        let indent = 2 * console.groups.len();

        match self {
            LogMessage::Error(msg) => rt.write_debug(&format!("[🔴] {msg:>indent$}\n")),
            LogMessage::Warn(msg) => rt.write_debug(&format!("[🟠] {msg:>indent$}\n")),
            LogMessage::Info(msg) => rt.write_debug(&format!("[🟢] {msg:>indent$}\n")),
            LogMessage::Log(msg) => rt.write_debug(&format!("[🪵] {msg:>indent$}\n")),
        }
    }
}
fn escape(unescaped: &str) -> String {
    fn split(
        (in_string, escaped, done, iter): &mut (
            bool,
            bool,
            bool,
            impl Iterator<Item = char>,
        ),
    ) -> Option<String> {
        if *done {
            return None;
        };
        let mut accumulator = String::default();
        for chr in iter {
            *escaped = false;
            match chr {
                '\\' => {
                    *escaped = true;
                    accumulator.push(chr)
                }
                '"' => {
                    if !*escaped {
                        *in_string = !*in_string;
                    }
                    accumulator.push(chr)
                }
                '\n' => {
                    if !*in_string {
                        return Some(accumulator.trim().to_string());
                    }
                    accumulator.push_str("\\n")
                }
                _ => accumulator.push(chr),
            }
        }
        if accumulator.len() > 0 {
            *done = true;
            Some(accumulator.trim().to_string())
        } else {
            None
        }
    }
    itertools::unfold((false, false, false, unescaped.chars()), split).join(" ")
}
fn display_js(value: &JsValue) -> String {
    match value.as_string() {
        Some(value) => value.to_std_string_escaped(),
        None => escape(&value.display().to_string()),
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
                    match c {
                        '%' => {
                            let fmt = chars.next().unwrap_or('%');
                            match fmt {
                                /* integer */
                                'd' | 'i' => {
                                    let arg = match data
                                        .get_or_undefined(arg_index)
                                        .to_numeric(context)?
                                    {
                                        Numeric::Number(r) => {
                                            (r.floor() + 0.0).to_string()
                                        }
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
                                '\n' => formatted.push_str("\\n"),
                                c => {
                                    formatted.push('%');
                                    formatted.push(c);
                                }
                            }
                        }
                        '\n' => formatted.push_str("\\n"),
                        c => {
                            formatted.push(c);
                        }
                    };
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

#[derive(Trace, Finalize)]
struct Console {
    groups: Vec<String>,
}

impl Console {
    fn new() -> Self {
        Self {
            groups: Vec::default(),
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
        self.groups.clear()
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
        self.groups.push(group_label);
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
        self.groups.pop();
    }
}

/// `ConsoleApi` implements `jstz_core::host::Api`, permitting it to be registered
/// as a `jstz` runtime API.  
pub struct ConsoleApi;

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

macro_rules! vardic_console_function {
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

    vardic_console_function!(log);
    vardic_console_function!(error);
    vardic_console_function!(debug);
    vardic_console_function!(warn);
    vardic_console_function!(info);

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
}

impl jstz_core::Api for ConsoleApi {
    fn init(self, context: &mut Context<'_>) {
        let console = ObjectInitializer::with_native(Console::new(), context)
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
