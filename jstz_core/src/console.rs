use crate::host_ref::HostRef;
use boa_engine::{
    object::{Object, ObjectInitializer},
    Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsValue, NativeFunction,
};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};
use jstz_serde::{Address, BasicConsoleMessage, ConsoleMessage};
use tezos_smart_rollup_host::runtime::Runtime;

pub(super) fn make_console<Host: Runtime + 'static>(
    context: &mut Context<'_>,
    host: &HostRef<Host>,
    address: &Address,
) -> JsObject {
    Console::new(host.clone(), address.clone()).build(context)
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy)]
enum ConsolePrefix {
    Log,
    Error,
    Warning,
    Debug,
    Info,
}
fn create_console_message(
    prefix: ConsolePrefix,
    msg: BasicConsoleMessage,
) -> ConsoleMessage {
    match prefix {
        ConsolePrefix::Log => ConsoleMessage::Log(msg),
        ConsolePrefix::Error => ConsoleMessage::Error(msg),
        ConsolePrefix::Warning => ConsoleMessage::Warning(msg),
        ConsolePrefix::Debug => ConsoleMessage::Debug(msg),
        ConsolePrefix::Info => ConsoleMessage::Info(msg),
    }
}

struct Console<Host> {
    host: HostRef<Host>,
    group: Vec<String>,
    address: Address,
}

impl<Host: Runtime + 'static> Console<Host> {
    fn extract<'a>(this: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        this.as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| JsError::from_native(JsNativeError::typ()))
    }
    fn log_message(&self, msg: &ConsoleMessage) -> Option<()> {
        let msg = jstz_serde::create_log_message(msg)?;
        Some(self.host.write_debug(&msg))
    }
    fn create_basic_message<'a, 'b>(
        &'a self,
        messages: &'b Vec<String>,
    ) -> BasicConsoleMessage<'b>
    where
        'a: 'b,
    {
        BasicConsoleMessage {
            messages,
            address: &self.address,
            group: &self.group,
        }
    }

    fn new(host: HostRef<Host>, address: Address) -> Self {
        Self {
            host,
            address,
            group: Vec::default(),
        }
    }

    fn push_group(&mut self, group: String) -> () {
        self.group.push(group);
    }
    fn pop_group(&mut self) -> () {
        self.group.pop();
    }
}

impl<Host: Runtime + 'static> Console<Host> {
    fn log_with_prefix<'a>(
        this: &JsValue,
        prefix: ConsolePrefix,
        messages: impl IntoIterator<Item = &'a JsValue>,
    ) -> JsResult<JsValue> {
        fn err() -> JsError {
            let err = JsNativeError::error().with_message("couldn't log message");
            JsError::from_native(err)
        }
        let this = Self::extract(this)?;
        let messages: Vec<String> = messages
            .into_iter()
            .map(|arg| format!("{}", arg.display()))
            .collect();

        let msg = create_console_message(prefix, this.create_basic_message(&messages));
        this.log_message(&msg).ok_or_else(err)?;

        Ok(JsValue::default())
    }

    fn log(this: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
        Self::log_with_prefix(this, ConsolePrefix::Log, args)
    }

    fn err(this: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
        Self::log_with_prefix(this, ConsolePrefix::Error, args)
    }

    fn debug(this: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
        Self::log_with_prefix(this, ConsolePrefix::Debug, args)
    }

    fn warn(this: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
        Self::log_with_prefix(this, ConsolePrefix::Warning, args)
    }

    fn info(this: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
        Self::log_with_prefix(this, ConsolePrefix::Info, args)
    }
    fn group(this: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
        fn type_err() -> JsError {
            let native = JsNativeError::typ().with_message("expected string");
            JsError::from_native(native)
        }
        fn string_err(group: &String) -> JsError {
            let native =
                JsNativeError::typ().with_message(format!("invalid group_name: {group}"));
            JsError::from_native(native)
        }
        let group_name = args.get_or_undefined(0).as_string().ok_or_else(type_err)?;
        let group_name = group_name
            .to_std_string()
            .map_err(|_| string_err(&group_name.to_std_string_escaped()))?;

        let mut this = Self::extract(this)?;
        this.push_group(group_name);
        Ok(JsValue::default())
    }
    fn end_group(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
        let mut this = Self::extract(this)?;
        this.pop_group();
        Ok(JsValue::default())
    }

    fn build(self, context: &mut Context) -> JsObject {
        ObjectInitializer::with_native(self, context)
            .function(NativeFunction::from_fn_ptr(Self::log), "log", 0)
            .function(NativeFunction::from_fn_ptr(Self::err), "error", 0)
            .function(NativeFunction::from_fn_ptr(Self::debug), "debug", 0)
            .function(NativeFunction::from_fn_ptr(Self::warn), "warn", 0)
            .function(NativeFunction::from_fn_ptr(Self::info), "info", 0)
            .function(NativeFunction::from_fn_ptr(Self::group), "group", 0)
            .function(
                NativeFunction::from_fn_ptr(Self::group),
                "groupCollapsed",
                0,
            )
            .function(NativeFunction::from_fn_ptr(Self::end_group), "groupEnd", 0)
            .build()
    }
}

impl<Host> Finalize for Console<Host> {}
unsafe impl<Host> Trace for Console<Host> {
    empty_trace!();
}
