use crate::host::storage::Storage;
use crate::host::HostRef;
use boa_engine::object::Object;
use boa_engine::{object::ObjectInitializer, Context, JsObject, JsValue};
use boa_engine::{JsArgs, JsError, JsNativeError, JsResult, JsString, NativeFunction};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};
use jstz_serde::Address;
use tezos_smart_rollup_host::path::OwnedPath;
use tezos_smart_rollup_host::path::PathError;
use tezos_smart_rollup_host::runtime::{Runtime, RuntimeError};

fn key_error(js_path: &String) -> JsError {
    let msg = format!("Invalid Key: {js_path}");
    let native = JsNativeError::range().with_message(msg);
    JsError::from_native(native)
}
fn runtime_error(err: RuntimeError) -> JsError {
    let native = JsNativeError::error().with_message(format!("{err}"));
    JsError::from_native(native)
}
fn error_or_undefined(val: Result<JsValue, RuntimeError>) -> JsResult<JsValue> {
    match val {
        Ok(val) => Ok(val),
        Err(RuntimeError::PathNotFound) => Ok(JsValue::default()),
        Err(err) => Err(runtime_error(err)),
    }
}
fn to_byte_repr(str: &JsString) -> Vec<u8> {
    str.as_slice()
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect()
}
fn from_byte_repr(bytes: &[u8]) -> JsString {
    let str: Vec<u16> = bytes
        .chunks(2)
        .map(|xs| u16::from_le_bytes([xs[0], xs[1]]))
        .collect();
    str.into()
}
fn extract_string(str: &JsValue) -> JsResult<String> {
    let str = str.as_string().ok_or_else(|| {
        JsError::from_native(JsNativeError::typ().with_message("key must be a string"))
    })?;
    let result = str
        .to_std_string()
        .map_err(|_| key_error(&str.to_std_string_escaped()))?;
    Ok(result)
}
struct JsStorage<Host> {
    data: Storage<Host>,
}

impl<Host: Runtime + 'static> JsStorage<Host> {
    fn new(host: HostRef<Host>, prefix: String) -> Self {
        let data = Storage::new(host, prefix);
        Self { data }
    }

    fn extract<'a>(obj: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        obj.as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| JsError::from_native(JsNativeError::typ()))
    }
    fn write_value(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut this = Self::extract(this)?;
        let key = extract_string(args.get_or_undefined(0))?;
        let value = to_byte_repr(&args.get_or_undefined(1).to_string(context)?);
        this.data.write_value(&key, &value).map_err(runtime_error)?;
        Ok(JsValue::default())
    }
    fn read_value(
        this: &JsValue,
        args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<JsValue> {
        let this = Self::extract(this)?;
        let js_path = extract_string(args.get_or_undefined(0))?;
        fn bytes_to_string(bytes: Vec<u8>) -> JsValue {
            JsValue::String(from_byte_repr(bytes.as_slice()))
        }
        error_or_undefined(this.data.read_value(&js_path).map(bytes_to_string))
    }
    fn remove_value(
        this: &JsValue,
        args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut this = Self::extract(this)?;
        let js_path = extract_string(args.get_or_undefined(0))?;
        error_or_undefined(this.data.remove_value(&js_path).map(|_| JsValue::default()))
    }
    fn sub_storage(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let this = Self::extract(this)?;
        let prefix = extract_string(args.get_or_undefined(0))?;
        let err = |_: PathError| key_error(&prefix);
        let prefix = format!("{}/{}", &this.data.prefix(), &prefix);
        let _: OwnedPath = prefix.clone().try_into().map_err(err)?;
        let result = Self::new(this.data.host().clone(), prefix).build(context);
        Ok(result.into())
    }
    fn build(self, context: &mut Context) -> JsObject {
        ObjectInitializer::with_native(self, context)
            .function(NativeFunction::from_fn_ptr(Self::write_value), "put", 2)
            .function(NativeFunction::from_fn_ptr(Self::read_value), "get", 1)
            .function(NativeFunction::from_fn_ptr(Self::remove_value), "delete", 1)
            .function(
                NativeFunction::from_fn_ptr(Self::sub_storage),
                "sub_storage",
                1,
            )
            .build()
    }
}

impl<Host> Finalize for JsStorage<Host> {}
unsafe impl<Host> Trace for JsStorage<Host> {
    empty_trace!();
}

pub(super) fn make_storage<Host: Runtime + 'static>(
    context: &mut Context<'_>,
    host: &HostRef<Host>,
    prefix: &Address,
) -> JsObject {
    JsStorage::new(host.clone(), prefix.to_string()).build(context)
}
