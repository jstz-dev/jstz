use crate::host_ref::HostRef;
use boa_engine::object::Object;
use boa_engine::{object::ObjectInitializer, Context, JsObject, JsValue};
use boa_engine::{JsArgs, JsError, JsNativeError, JsResult, JsString, NativeFunction};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};
use tezos_smart_rollup_host::path::PathError;
use tezos_smart_rollup_host::runtime::RuntimeError;
use tezos_smart_rollup_host::{path::OwnedPath, runtime::Runtime};

fn key_error(js_path: &String) -> JsError {
    let msg = format!("Invalid Key: {js_path}");
    let native = JsNativeError::range().with_message(msg);
    JsError::from_native(native)
}
fn runtime_error(err: RuntimeError) -> JsError {
    let native = JsNativeError::error().with_message(format!("{err}"));
    JsError::from_native(native)
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
struct Storage<Host> {
    host: HostRef<Host>,
    prefix: String,
}
impl<Host: Runtime + 'static> Storage<Host> {
    fn new(host: HostRef<Host>, prefix: String) -> Self {
        Self { host, prefix }
    }
    fn create_path(&self, js_path: &String) -> JsResult<OwnedPath> {
        let prefix = &self.prefix;
        let path = format!("/{prefix}/{js_path}").to_string();

        path.try_into().map_err(|_| key_error(js_path))
    }
    fn extract<'a>(obj: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        obj.as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| JsError::from_native(JsNativeError::typ()))
    }
    fn write_value_inner(&mut self, js_path: &String, value: &JsString) -> JsResult<()> {
        let path = self.create_path(js_path)?;
        let value = to_byte_repr(value);
        let result = self
            .host
            .store_write_all(&path, value.as_slice())
            .map_err(runtime_error)?;
        Ok(result)
    }
    fn write_value(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut this = Self::extract(this)?;
        let js_path = extract_string(args.get_or_undefined(0))?;
        let value = args.get_or_undefined(1).to_string(context)?;
        let () = this.write_value_inner(&js_path, &value)?;
        Ok(JsValue::default())
    }
    fn read_value_inner(&mut self, js_path: &String) -> JsResult<JsValue> {
        let path = self.create_path(js_path)?;
        let bytes = self.host.store_read_all(&path).map_err(runtime_error)?;
        Ok(JsValue::String(from_byte_repr(bytes.as_slice())))
    }
    fn read_value(
        this: &JsValue,
        args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut this = Self::extract(this)?;
        let js_path = extract_string(args.get_or_undefined(0))?;
        this.read_value_inner(&js_path)
    }
    fn remove_value_inner(&mut self, js_path: &String) -> JsResult<()> {
        let path = self.create_path(js_path)?;
        self.host.store_delete(&path).map_err(runtime_error)
    }
    fn remove_value(
        this: &JsValue,
        args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut this = Self::extract(this)?;
        let js_path = extract_string(args.get_or_undefined(0))?;
        let () = this.remove_value_inner(&js_path)?;
        Ok(JsValue::default())
    }
    fn sub_storage_inner(
        &self,
        prefix: String,
        context: &mut Context,
    ) -> JsResult<JsObject> {
        let err = |_: PathError| key_error(&prefix);
        let prefix = format!("{}/{}", &self.prefix, &prefix);
        let _: OwnedPath = prefix.clone().try_into().map_err(err)?;
        Ok(Self::new(self.host.clone(), prefix).build(context))
    }
    fn sub_storage(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let this = Self::extract(this)?;
        let prefix = extract_string(args.get_or_undefined(0))?;
        let result = this.sub_storage_inner(prefix, context)?;
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

impl<Host> Finalize for Storage<Host> {}
unsafe impl<Host> Trace for Storage<Host> {
    empty_trace!();
}

pub(super) fn make_storage<Host: Runtime + 'static>(
    context: &mut Context<'_>,
    host: &HostRef<Host>,
    prefix: String,
) -> JsObject {
    Storage::new(host.clone(), prefix).build(context)
}
