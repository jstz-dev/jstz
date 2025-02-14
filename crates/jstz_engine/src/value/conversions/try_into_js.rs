use crate::{
    context::{CanAccess, CanAlloc, Context, InCompartment},
    gc::{ptr::AsRawPtr, Compartment},
    string::JsString,
    value::JsValue,
};
use mozjs::jsval::StringValue;

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

#[cfg(test)]
mod test {
    use crate::{letroot, setup_cx, value::TryFromJs};

    use super::TryIntoJs;

    #[test]
    fn str_try_into_js() {
        setup_cx!(cx);
        letroot!(v = "hello".try_into_js(&mut cx).unwrap(); [cx]);
        assert_eq!("hello", String::try_from_js(&v, (), &mut cx).unwrap())
    }
}
