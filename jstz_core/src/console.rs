use crate::host::HostRef;
use boa_engine::{
    object::{Object, ObjectInitializer},
    Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsValue, NativeFunction,
};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};
use jstz_serde::{Address, ConsoleMessage, ConsolePrefix};
use tezos_smart_rollup_host::runtime::Runtime;

pub fn make_console<Host: Runtime + 'static>(
    context: &mut Context<'_>,
    host: &HostRef<Host>,
    address: &Address,
) -> JsObject {
    Console::new(host.clone(), address.clone()).build(context)
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
    fn create_console_message<'a, 'b>(
        &'a self,
        message_type: ConsolePrefix,
        messages: &'b Vec<String>,
    ) -> ConsoleMessage<'b>
    where
        'a: 'b,
    {
        ConsoleMessage {
            message_type,
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

        let msg = this.create_console_message(prefix, &messages);
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
    fn assert(this: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
        let failed: JsValue = "assertion failed:".into();
        let console_assert: Option<JsValue> = if args.len() < 2 {
            Some("consle.assert".into())
        } else {
            None
        };
        let condition = args.get_or_undefined(0).to_boolean();
        if condition {
            Ok(JsValue::default())
        } else {
            let messages = std::iter::once(&failed)
                .chain(console_assert.iter())
                .chain((if args.len() < 2 { &[] } else { &args[1..] }).iter());
            Self::log_with_prefix(this, ConsolePrefix::Error, messages)
        }
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
            .function(NativeFunction::from_fn_ptr(Self::assert), "assert", 0)
            .function(NativeFunction::from_fn_ptr(Self::group), "group", 1)
            .function(
                NativeFunction::from_fn_ptr(Self::group),
                "groupCollapsed",
                1,
            )
            .function(NativeFunction::from_fn_ptr(Self::end_group), "groupEnd", 0)
            .build()
    }
}

impl<Host> Finalize for Console<Host> {}
unsafe impl<Host> Trace for Console<Host> {
    empty_trace!();
}
