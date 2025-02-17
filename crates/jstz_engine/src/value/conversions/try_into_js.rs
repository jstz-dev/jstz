use crate::{
    context::{CanAccess, CanAlloc, Context, InCompartment},
    gc::{ptr::AsRawPtr, Compartment},
    letroot,
    object::property::PropertyKey,
    string::JsString,
    value::JsValue,
};
use mozjs::{jsapi::JS_IdToValue, jsval::StringValue};

// Conversion from a Rust Type into a [`JsValue`]
#[allow(unused)]
pub trait TryIntoJs<'cx, C: Compartment> {
    /// This function tries to convert a `Self` into a [`JsValue`].
    fn try_into_js<S>(&self, cx: &'cx mut Context<S>) -> Option<JsValue<'cx, C>>
    where
        S: InCompartment<C> + CanAccess + CanAlloc;
}

impl<'cx, C: Compartment> TryIntoJs<'cx, C> for &str {
    fn try_into_js<S>(&self, cx: &'cx mut Context<S>) -> Option<JsValue<'cx, C>>
    where
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        let js_string = JsString::new(self, cx);
        Some(unsafe { JsValue::from_raw(StringValue(&*js_string.as_raw_ptr())) })
    }
}

impl<'cx, C: Compartment> TryIntoJs<'cx, C> for PropertyKey<'cx, C> {
    fn try_into_js<S>(&self, cx: &'cx mut Context<S>) -> Option<JsValue<'cx, C>>
    where
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        letroot!(rval = JsValue::undefined(cx); [cx]);
        unsafe { JS_IdToValue(cx.as_raw_ptr(), self.as_raw_ptr(), rval.handle_mut()) };
        if rval.is_undefined() {
            return None;
        }
        Some(rval.into_inner(cx))
    }
}

#[cfg(test)]
mod test {
    use crate::{letroot, object::property::IntoPropertyKey, setup_cx, value::TryFromJs};

    use super::TryIntoJs;

    #[test]
    fn str_try_into_js() {
        setup_cx!(cx);
        letroot!(v = "hello".try_into_js(&mut cx).unwrap(); [cx]);
        assert_eq!("hello", String::try_from_js(&v, (), &mut cx).unwrap())
    }

    #[test]
    fn property_key_try_into_js() {
        setup_cx!(cx);
        letroot!(p = "hello".into_key(&mut cx).unwrap(); [cx]);
        letroot!(value = p.try_into_js(&mut cx).unwrap(); [cx]);

        assert!(value.is_string());
        assert_eq!("hello", String::try_from_js(&value, (), &mut cx).unwrap())
    }
}
