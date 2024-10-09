//! `jstz`'s implementation of JavaScript's `File` API.
//!
//! More information:
//!  - [MDN documentation][mdn]
//!  - [W3C `File` specification][spec]
//!  - [WHATWG `Infra` specification][infra-spec]
//!
//! [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Blob
//! [spec]: https://w3c.github.io/FileAPI/
//! [infra-spec]: https://infra.spec.whatwg.org/

use boa_engine::{
    js_string,
    object::{
        builtins::{JsDate, JsPromise},
        ErasedObject,
    },
    property::Attribute,
    value::TryFromJs,
    Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};
use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject, NativeClass,
    },
    value::IntoJs,
};

use super::blob::{Blob, BlobClass, BlobParts, BlobPropertyBag};

#[derive(Trace, Finalize, JsData, Clone)]
pub struct File {
    blob: Blob,
    name: String,
    last_modified: i64,
}

impl File {
    // https://w3c.github.io/FileAPI/#file-constructor
    pub fn new(
        file_bits: BlobParts,
        file_name: String,
        options: &Option<FilePropertyBag>,
        context: &mut Context,
    ) -> JsResult<Self> {
        // 1. Let bytes be the result of processing blob parts given fileBits and options.
        //    NOTE: we use Blob constructor instead
        let blob_property_bag = options
            .as_ref()
            .map(|options| options.blob_property_bag.clone());
        let blob = Blob::new(Some(file_bits), blob_property_bag, context)?;
        // 2. Let n be the fileName argument to the constructor.
        let n = file_name;
        // 3. Process FilePropertyBag dictionary argument by running the following substeps:
        //    1 and 2. are already done by Blob's constructor
        //    3. If the lastModified member is provided, let d be set to the lastModified dictionary member.
        //       If it is not provided, set d to the current date and time represented as the number of
        //       milliseconds since the Unix Epoch (which is the equivalent of Date.now() [ECMA-262]).
        let d = options
            .as_ref()
            .and_then(|options| options.last_modified)
            .unwrap_or_else(|| context.host_hooks().utc_now());
        // 4. Return a new File object F such that:
        Ok(Self {
            // 2. F refers to the bytes byte sequence.
            // 3. F.size is set to the number of total bytes in bytes.
            // 5. F.type is set to t.
            blob,
            // 4. F.name is set to n.
            name: n,
            // 6. F.lastModified is set to d.
            last_modified: d,
        })
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn last_modified(&self) -> i64 {
        self.last_modified
    }

    pub fn size(&self) -> u64 {
        self.blob.size()
    }

    pub fn type_(&self) -> String {
        self.blob.type_()
    }

    pub fn text(&mut self, context: &mut Context) -> JsResult<JsPromise> {
        self.blob.text(context)
    }

    pub fn array_buffer(&mut self, context: &mut Context) -> JsResult<JsPromise> {
        self.blob.array_buffer(context)
    }

    pub fn slice(
        &self,
        start: Option<i64>,
        end: Option<i64>,
        content_type: Option<String>,
    ) -> Blob {
        self.blob.slice(start, end, content_type)
    }
}

impl File {
    pub fn try_from_js(value: &JsValue) -> JsResult<GcRefMut<'_, ErasedObject, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `Blob`")
                    .into()
            })
    }
}

#[derive(Default)]
pub struct FilePropertyBag {
    blob_property_bag: BlobPropertyBag,
    last_modified: Option<i64>,
}

impl TryFromJs for FilePropertyBag {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        let blob_property_bag = BlobPropertyBag::try_from_js(value, context)?;

        let obj = value.as_object().ok_or_else(|| {
            JsError::from_native(JsNativeError::typ().with_message("Expected object"))
        })?;

        let last_modified = obj.get(js_string!("lastModified"), context)?;
        let last_modified: Option<i64> = if last_modified.is_object() {
            let obj = last_modified.as_object().unwrap();
            if let Ok(date) = JsDate::from_object(obj.clone()) {
                let time = f64::try_from_js(&date.get_time(context).unwrap(), context)?;
                Some(time as i64)
            } else {
                Some(0)
            }
        } else if last_modified.is_number() {
            Some(f64::try_from_js(&last_modified, context)? as i64)
        } else {
            Some(0)
        };

        Ok(Self {
            blob_property_bag,
            last_modified,
        })
    }
}

pub struct FileClass;

impl FileClass {
    fn name(context: &mut Context) -> Accessor {
        accessor!(
            context,
            File,
            "name",
            get:((file, context) => Ok(file.name().into_js(context)))
        )
    }

    fn last_modified(context: &mut Context) -> Accessor {
        accessor!(
            context,
            File,
            "last_modified",
            get:((file, _context) => Ok(file.last_modified().into()))
        )
    }

    fn size(context: &mut Context) -> Accessor {
        accessor!(
            context,
            File,
            "size",
            get:((file, _context) => Ok(file.size().into()))
        )
    }

    fn type_(context: &mut Context) -> Accessor {
        accessor!(
            context,
            File,
            "type",
            get:((file, context) => Ok(file.type_().into_js(context)))
        )
    }

    fn text(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut file = File::try_from_js(this)?;

        Ok(file.text(context)?.into())
    }

    fn array_buffer(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut file = File::try_from_js(this)?;

        Ok(file.array_buffer(context)?.into())
    }

    fn slice(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let file = File::try_from_js(this)?;
        let start: Option<i64> = args.get_or_undefined(0).try_js_into(context)?;
        let end: Option<i64> = args.get_or_undefined(1).try_js_into(context)?;
        let content_type: Option<String> =
            args.get_or_undefined(2).try_js_into(context)?;
        let blob = file.slice(start, end, content_type);
        let blob = JsNativeObject::new::<BlobClass>(blob, context)?;

        Ok(blob.to_inner())
    }
}

impl NativeClass for FileClass {
    type Instance = File;

    const NAME: &'static str = "File";

    fn data_constructor(
        _target: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<Self::Instance> {
        let blob_parts: BlobParts = args.get_or_undefined(0).try_js_into(context)?;
        let file_name: String = args.get_or_undefined(1).try_js_into(context)?;
        let options: Option<FilePropertyBag> =
            args.get_or_undefined(2).try_js_into(context)?;

        File::new(blob_parts, file_name, &options, context)
    }

    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
        let name = Self::name(class.context());
        let last_modified = Self::last_modified(class.context());
        let size = Self::size(class.context());
        let type_ = Self::type_(class.context());

        class
            .accessor(js_string!("name"), name, Attribute::all())
            .accessor(js_string!("lastModified"), last_modified, Attribute::all())
            .accessor(js_string!("size"), size, Attribute::all())
            .accessor(js_string!("type"), type_, Attribute::all())
            .method(
                js_string!("text"),
                0,
                NativeFunction::from_fn_ptr(Self::text),
            )
            .method(
                js_string!("arrayBuffer"),
                0,
                NativeFunction::from_fn_ptr(Self::array_buffer),
            )
            .method(
                js_string!("slice"),
                0,
                NativeFunction::from_fn_ptr(Self::slice),
            );

        Ok(())
    }
}

pub struct FileApi;

impl jstz_core::Api for FileApi {
    fn init(self, context: &mut Context) {
        register_global_class::<FileClass>(context)
            .expect("The `File` class shouldn't exist yet")
    }
}
