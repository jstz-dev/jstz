use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use derive_more::{Display, Error};

pub trait FromJs: Sized {
    fn from_js(this: &JsValue, context: &mut Context) -> JsResult<Self>;
    fn from_js_args(
        args: &[JsValue],
        index: usize,
        context: &mut Context,
    ) -> JsResult<Self> {
        match args.get_or_undefined(index) {
            JsValue::Undefined => {
                let err = JsNativeError::typ().with_message("Not enough arguments");
                Err(JsError::from_native(err))
            }
            arg => Self::from_js(arg, context),
        }
    }
}
pub trait ToJs {
    fn to_js(self, context: &mut Context) -> JsResult<JsValue>;
}

pub trait ToJsError {
    fn to_js_error(self, context: &mut Context) -> JsError;
}

pub fn std_err_to_js<T: std::error::Error + ?Sized>(err: &T) -> JsError {
    let native = JsNativeError::error().with_message(format!("{err}"));
    let native = match err.source() {
        Some(cause) => native.with_cause(std_err_to_js(cause)),
        None => native,
    };
    JsError::from_native(native)
}
#[derive(PartialEq, PartialOrd, Eq, Ord, Copy, Clone, Debug, Error, Display)]
pub enum JsTypeError {
    #[display(fmt = "Expected String")]
    ExpectedString,
}

impl ToJsError for JsTypeError {
    fn to_js_error(self, _context: &mut Context) -> JsError {
        JsError::from_native(JsNativeError::typ().with_message(format!("{self}")))
    }
}
impl<T: ToJs, E: ToJsError> ToJs for Result<T, E> {
    fn to_js(self, context: &mut Context) -> JsResult<JsValue> {
        match self {
            Ok(value) => value.to_js(context),
            Err(err) => Err(err.to_js_error(context)),
        }
    }
}

impl<T: ToJs> ToJs for Option<T> {
    fn to_js(self, context: &mut Context) -> JsResult<JsValue> {
        match self {
            Some(value) => value.to_js(context),
            None => Ok(JsValue::null()),
        }
    }
}
impl<T: FromJs> FromJs for Option<T> {
    fn from_js(this: &JsValue, context: &mut Context) -> JsResult<Self> {
        match this {
            JsValue::Null | JsValue::Undefined => Ok(None),
            this => T::from_js(this, context).map(Some),
        }
    }
}

impl FromJs for () {
    fn from_js(_this: &JsValue, _context: &mut Context) -> JsResult<Self> {
        Ok(())
    }
}
impl ToJs for () {
    fn to_js(self, _context: &mut Context) -> JsResult<JsValue> {
        Ok(JsValue::default())
    }
}

impl FromJs for String {
    fn from_js(this: &JsValue, context: &mut Context) -> JsResult<Self> {
        this.to_string(context)?
            .to_std_string()
            .map_err(|err| std_err_to_js(&err))
    }
}
impl ToJs for String {
    fn to_js(self, _context: &mut Context) -> JsResult<JsValue> {
        Ok(self.into())
    }
}

impl FromJs for bool {
    fn from_js(this: &JsValue, _context: &mut Context) -> JsResult<Self> {
        Ok(this.to_boolean())
    }
}
impl ToJs for bool {
    fn to_js(self, _context: &mut Context) -> JsResult<JsValue> {
        Ok(self.into())
    }
}

#[repr(transparent)]
pub struct ExpectString(pub String);
impl FromJs for ExpectString {
    fn from_js(this: &JsValue, context: &mut Context) -> JsResult<Self> {
        let inner = this
            .as_string()
            .ok_or_else(|| JsTypeError::ExpectedString.to_js_error(context))?
            .to_std_string()
            .map_err(|err| std_err_to_js(&err))?;
        Ok(Self(inner))
    }
}
#[repr(transparent)]
pub struct EscapedString(pub String);
impl FromJs for EscapedString {
    fn from_js(this: &JsValue, context: &mut Context) -> JsResult<Self> {
        Ok(Self(this.to_string(context)?.to_std_string_escaped()))
    }
}
