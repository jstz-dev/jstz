use std::fmt;

use crate::host_ref::HostRef;
use boa_engine::{
    object::{Object, ObjectInitializer},
    Context, JsError, JsNativeError, JsObject, JsResult, JsValue, NativeFunction,
};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};
use tezos_smart_rollup_host::runtime::Runtime;




pub(super) fn make_console<Host: Runtime + 'static>(
    context: &mut Context<'_>,
    host: &HostRef<Host>,
) -> JsObject {
    Console::new(host.clone()).build(context)
}





#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy)]
enum ConsolePrefix {
    Log,
    Err,
}
impl fmt::Display for ConsolePrefix {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let str = match self {
            ConsolePrefix::Log => "LOG",
            ConsolePrefix::Err => "ERR",
        };
        write!(f, "{}", str)
    }
}

struct Console<Host> {
    host: HostRef<Host>,
}

impl<Host: Runtime + 'static> Console<Host> {
    fn extract<'a>(obj: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        obj.as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| JsError::from_native(JsNativeError::typ()))
    }

    fn log_items(&self, prefix: ConsolePrefix, args: &[JsValue]) -> () {
        self.host.write_debug(
            &args
                .iter()
                .map(|arg| format!("{:?}{}\n", prefix, arg.display()))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }

    fn log(this: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
        let this = Self::extract(this)?;
        this.log_items(ConsolePrefix::Log, args);
        Ok(JsValue::default())
    }

    fn err(this: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
        let this = Self::extract(this)?;
        this.log_items(ConsolePrefix::Err, args);
        Ok(JsValue::default())
    }

    fn new(host: HostRef<Host>) -> Self {
        Self { host }
    }
    fn build(self, context: &mut Context) -> JsObject {
        ObjectInitializer::with_native(self, context)
            .function(NativeFunction::from_fn_ptr(Self::log), "log", 0)
            .function(NativeFunction::from_fn_ptr(Self::err), "error", 0)
            .build()
    }
}

impl<Host> Finalize for Console<Host> {}
unsafe impl<Host> Trace for Console<Host> {
    empty_trace!();
}
