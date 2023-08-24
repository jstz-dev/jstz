use boa_engine::{JsValue, JsResult, JsError, Context, JsNativeError};
use derive_more::{Display, Error};

pub trait FromJs : Sized{
  fn from_js(this : &JsValue, context : &mut Context) -> JsResult<Self>;
}
pub trait ToJs {
  fn to_js(self, context: &mut Context) -> JsResult<JsValue>;
}

pub trait ToJsError {
  fn to_js_error(self, context: &mut Context) -> JsError;
}

#[derive(PartialEq, PartialOrd, Eq, Ord, Copy, Clone, Debug, Error, Display)]
pub enum JsTypeError {
  #[display(fmt = "Expected String")]
  String,
}

impl ToJsError for JsTypeError {
  fn to_js_error(self, context: &mut Context) -> JsError {
    JsError::from_native(JsNativeError::typ().with_message(format!("{self}")))
  }
}
impl<T: ToJs, E: ToJsError> ToJs for Result<T,E> {
  fn to_js(self, context: &mut Context) -> JsResult<JsValue> {
    match self {
      Ok(value) => value.to_js(context),
      Err(err) => Err(err.to_js_error(context))
    }
  }
}
impl<T: ToJs> ToJs for Option<T> {
  fn to_js(self, context: &mut Context) -> JsResult<JsValue> {
    match self {
      Some(value) => value.to_js(context),
      None => Ok(JsValue::default())
    }
  }
}
impl<T: FromJs> FromJs for Option<T> {
  fn from_js(this : &JsValue, context : &mut Context) -> JsResult<Self> {
    match this {
      JsValue::Null | JsValue::Undefined => Ok(None),
      this => T::from_js(this, context).map(Some)
    }
  }
}
