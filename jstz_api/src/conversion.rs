


use boa_engine::{JsValue, JsResult, JsError, Context};

pub trait FromJs : Sized{
  fn from_js(this : &JsValue, context : &mut Context) -> JsResult<Self>;
}
pub trait ToJs {
  fn to_js(self, context: &mut Context) -> JsResult<JsValue>;
}

pub trait ToJsError {
  fn to_js_error(self, context: &mut Context) -> JsError;
}


mod from_js_ref {

  pub struct Boxed;
  pub struct Reference;
  pub struct Sized;

  pub trait FromJsRefStrategy
  {
    type JsRefStrategyReturn<'a, T : 'a> : 'a;
  }

  impl FromJsRefStrategy for Boxed{
    type JsRefStrategyReturn<'a, T : 'a> = Box<T>;
  }
  impl FromJsRefStrategy for Reference{
    type JsRefStrategyReturn<'a, T : 'a> = &'a T;
  }
  impl FromJsRefStrategy for Sized{
    type JsRefStrategyReturn<'a, T : 'a> = T;
  }

}

/// this module effectively acts as a type level enum
/// which is why it's CamelCase
#[allow(non_snake_case)]
pub mod JsRefStrategy {
  pub use super::from_js_ref::{Boxed, Reference, Sized};
}

trait JsRefReturn<'a> : 'a{
  type ReturnType: Sized + 'a;
}

use from_js_ref::FromJsRefStrategy;

impl<'a, T> JsRefReturn<'a> for T
where
  T : FromJsRef<'a>,
  <T::Strategy as FromJsRefStrategy>::JsRefStrategyReturn<'a,Self> : Sized {
  type ReturnType = <<Self as FromJsRef<'a>>::Strategy as FromJsRefStrategy>::JsRefStrategyReturn<'a,Self> ;
}

trait FromJsRef<'a> : JsRefReturn<'a>
{
  type Strategy : FromJsRefStrategy;
  fn from_js_ref (this: &'a JsValue, context : &mut Context) -> JsResult<<Self as JsRefReturn<'a>>::ReturnType>;
}


impl<'a> FromJsRef<'a> for u8 {
  type Strategy = JsRefStrategy::Sized;
  fn from_js_ref(this: &'a JsValue, context : &mut Context) -> JsResult<u8> {
    this.to_uint8(context)
  }
}


/*
pub trait FromJsRef {
  fn to_js_ref<'a>(&'a JsValue, context : &'b Context) -> &'a Self {

  }
}
impl ToJs for i32 {
  fn to_js(self, _context: &mut Context) -> JsResult<JsValue>{
    Ok(JsValue::Integer(self))
  }
}
impl FromJs for i32 {
  fn from_js<'a, 'b>(this : &'a JsValue, context : &'b mut Context) -> JsResult<Self>
  where Self: 'a {
    this.to_i32(context)
  }
}
#[repr(transparent)]
struct JsStringInner(pub [u16]);
unsafe impl TransparentWrapper<[u16]> for JsStringInner {}

impl<'c> FromJs for &'c JsStringInner {
  fn from_js<'a,'b>(this : &'a JsValue, _context : &'b mut Context) -> JsResult<Self>
    where 'a : 'c
  {
    let JsValue::String(str) = this else {
      return Err(JsError::from_native(JsNativeError::typ().with_message("expected string")))
    };
    Ok(TransparentWrapper::wrap_ref(str.as_slice()))
  }
}


#[repr(transparent)]
pub struct AnyError(Box<dyn Error>);
impl AnyError {
  pub fn new<T : Error + Clone + 'static>(source: T) -> Self {
      AnyError(Box::new(source.clone()))
  }
}
impl<E : Error + Clone + 'static> From<E> for AnyError {
  fn from(source: E) -> Self {
      AnyError::new(source)
    }
}
fn err_to_js<T: Error + ?Sized> (err: &T) -> JsError {
    let native = JsNativeError::error().with_message(format!("{err}"));
    let native = match err.source() {
      Some(cause) => native.with_cause(err_to_js(cause)),
      None => native
    };
    JsError::from_native(native)
}

impl<T : ToJs, E: ToJsError> ToJs for Result<T, E> {
  fn to_js(self, context: &mut Context) -> JsResult<JsValue> {
    match self {
      Ok(value) => value.to_js(context),
      Err(err) => Err(err.to_js_error(context))
    }
  }
}

pub enum JsRefStrategy {
  Convert, Direct
}



impl FromJs for i32 {
  fn from_js(this : &JsValue, context: &mut Context) -> JsResult<Self> {
    this.to_i32(context)
  }
}
impl ToJs for i32 {
  fn to_js(self, _context: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::Integer(self))
  }
}

macro_rules! from_js_clone {
  ($($ident: ident),*) => {
    $(
      impl FromJs for $ident {
        fn from_js(this: &JsValue, context: &mut Context) -> JsResult<Self> {
          <& $ident as FromJs>::from_js(context).clone()
        }
      }
    )*
  }
}


*/
