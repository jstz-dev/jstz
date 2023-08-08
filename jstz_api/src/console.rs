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

use boa_engine::{
    object::{Object, ObjectInitializer},
    property::Attribute,
    value::Numeric,
    Context, JsArgs, JsNativeError, JsResult, JsValue, NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};
use jstz_core::host::Host;
use tezos_smart_rollup_host::runtime::Runtime;

/// This represents the different types of log messages.
#[derive(Debug)]
enum LogMessage {
    Log(String),
    Info(String),
    Warn(String),
    Error(String),
}

impl LogMessage {
    fn log<H: Runtime>(self, console: &Console<H>) {
        let indent = 2 * console.groups.len();

        match self {
            LogMessage::Error(msg) => {
                console.host.write_debug(&format!("[🔴]{msg:>indent$}\n"))
            }
            LogMessage::Warn(msg) => {
                console.host.write_debug(&format!("[🟠]{msg:>indent$}\n"))
            }
            LogMessage::Info(msg) => {
                console.host.write_debug(&format!("[🟢]{msg:>indent$}\n"))
            }
            LogMessage::Log(msg) => {
                console.host.write_debug(&format!("{msg:>indent$}\n"))
            }
        }
    }
}

/// This represents the `console` formatter.
///
/// More information:
///  - [WHATWG `formatter` specification][https://console.spec.whatwg.org/#formatter]
fn formatter(data: &[JsValue], context: &mut Context<'_>) -> JsResult<String> {
    match data {
        [] => Ok(String::new()),
        [val] => Ok(val.to_string(context)?.to_std_string_escaped()),
        data => {
            let mut formatted = String::new();
            let mut arg_index = 1;
            let target = data
                .get_or_undefined(0)
                .to_string(context)?
                .to_std_string_escaped();
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
                            let arg =
                                data.get_or_undefined(arg_index).to_number(context)?;
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
                };
            }

            /* unformatted data */
            for rest in data.iter().skip(arg_index) {
                formatted.push_str(&format!(
                    " {}",
                    rest.to_string(context)?.to_std_string_escaped()
                ));
            }

            Ok(formatted)
        }
    }
}

#[derive(Trace, Finalize)]
struct Console<H: Runtime + 'static> {
    host: Host<H>,
    groups: Vec<String>,
}

impl<H: Runtime + 'static> Console<H> {
    fn new(host: Host<H>) -> Self {
        Self {
            host,
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
        context: &mut Context<'_>,
    ) -> JsResult<()> {
        if !assertion {
            let mut args: Vec<JsValue> = Vec::from(data);
            let message = "Assertion failed".to_string();
            if args.is_empty() {
                args.push(JsValue::new(message));
            } else if !args[0].is_string() {
                args.insert(0, JsValue::new(message));
            } else {
                let concat = format!("{message}: {}", args[0].display());
                args[0] = JsValue::new(concat);
            }

            LogMessage::Error(formatter(&args, context)?).log(self)
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
    fn debug(&self, data: &[JsValue], context: &mut Context<'_>) -> JsResult<()> {
        LogMessage::Log(formatter(data, context)?).log(self);
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
    fn warn(&self, data: &[JsValue], context: &mut Context<'_>) -> JsResult<()> {
        LogMessage::Warn(formatter(data, context)?).log(self);
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
    fn error(&self, data: &[JsValue], context: &mut Context<'_>) -> JsResult<()> {
        LogMessage::Error(formatter(data, context)?).log(self);
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
    fn info(&self, data: &[JsValue], context: &mut Context<'_>) -> JsResult<()> {
        LogMessage::Info(formatter(data, context)?).log(self);
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
    fn log(&self, data: &[JsValue], context: &mut Context<'_>) -> JsResult<()> {
        LogMessage::Log(formatter(data, context)?).log(self);
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
    fn group(&mut self, data: &[JsValue], context: &mut Context<'_>) -> JsResult<()> {
        let group_label = formatter(data, context)?;
        LogMessage::Log(format!("group: {group_label}")).log(self);
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

impl<H: Runtime> Console<H> {
    fn from_js_value<'a>(value: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| JsNativeError::typ().with_message("").into())
    }
}

macro_rules! vardic_console_function {
    ($name:ident) => {
        fn $name<H: Runtime + 'static>(
            this: &JsValue,
            args: &[JsValue],
            context: &mut Context<'_>,
        ) -> JsResult<JsValue> {
            let console = Console::<H>::from_js_value(this)?;
            console.$name(args, context)?;
            Ok(JsValue::undefined())
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

    fn assert<H: Runtime + 'static>(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let console = Console::<H>::from_js_value(this)?;

        let assertion = args.get_or_undefined(0).to_boolean();
        let data = if args.len() >= 1 { &args[1..] } else { &[] };
        console.assert(assertion, data, context)?;

        Ok(JsValue::undefined())
    }

    fn group<H: Runtime + 'static>(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut console = Console::<H>::from_js_value(this)?;
        console.group(args, context)?;
        Ok(JsValue::undefined())
    }

    fn group_end<H: Runtime + 'static>(
        this: &JsValue,
        _args: &[JsValue],
        _context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut console = Console::<H>::from_js_value(this)?;
        console.group_end();
        Ok(JsValue::undefined())
    }

    fn clear<H: Runtime + 'static>(
        this: &JsValue,
        _args: &[JsValue],
        _context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut console = Console::<H>::from_js_value(this)?;
        console.clear();
        Ok(JsValue::undefined())
    }
}

impl jstz_core::host::Api for ConsoleApi {
    fn init<H: Runtime>(context: &mut Context<'_>, host: Host<H>) {
        let console = ObjectInitializer::with_native(Console::new(host), context)
            .function(NativeFunction::from_fn_ptr(Self::log::<H>), "log", 0)
            .function(NativeFunction::from_fn_ptr(Self::error::<H>), "error", 0)
            .function(NativeFunction::from_fn_ptr(Self::debug::<H>), "debug", 0)
            .function(NativeFunction::from_fn_ptr(Self::warn::<H>), "warn", 0)
            .function(NativeFunction::from_fn_ptr(Self::info::<H>), "info", 0)
            .function(NativeFunction::from_fn_ptr(Self::assert::<H>), "assert", 0)
            .function(NativeFunction::from_fn_ptr(Self::group::<H>), "group", 0)
            .function(
                NativeFunction::from_fn_ptr(Self::group::<H>),
                "groupCollapsed",
                0,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::group_end::<H>),
                "groupEnd",
                0,
            )
            .function(NativeFunction::from_fn_ptr(Self::clear::<H>), "clear", 0)
            .build();

        context
            .register_global_property(Self::NAME, console, Attribute::all())
            .expect("console api should only be registered once!")
    }
}
