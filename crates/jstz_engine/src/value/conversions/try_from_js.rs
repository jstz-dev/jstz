use crate::{
    context::{CanAccess, CanAlloc, Context, InCompartment},
    gc::{ptr::AsRawPtr, Compartment},
    letroot,
    string::JsString,
    value::JsValue,
};

/// This trait adds a fallible and efficient conversions from a [`JsValue`] to Rust types.
///
/// Note that the Output type is not directly linked to
#[allow(unused)]
pub trait TryFromJs<'a, 'cx: 'a, C: Compartment> {
    type Output;
    type Config;

    /// This function tries to convert a JavaScript value into `Self::Output`.
    /// `config` determines the behaviour of the conversion.
    fn try_from_js<S>(
        value: &JsValue<'a, C>,
        config: Self::Config,
        cx: &'cx mut Context<S>,
    ) -> Option<Self::Output>
    where
        S: InCompartment<C> + CanAccess + CanAlloc;
}

impl<'a, 'cx: 'a, C: Compartment> TryFromJs<'a, 'cx, C> for i32 {
    type Output = i32;

    type Config = ();

    fn try_from_js<S>(
        value: &JsValue<'a, C>,
        _: Self::Config,
        _: &'cx mut Context<S>,
    ) -> Option<Self::Output>
    where
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        if !value.is_i32() {
            return None;
        }
        Some(unsafe { value.as_raw_ptr().to_int32() })
    }
}

impl<'a, 'cx: 'a, C: Compartment> TryFromJs<'a, 'cx, C> for f64 {
    type Output = f64;

    type Config = ();

    fn try_from_js<S>(
        value: &JsValue<'a, C>,
        _: Self::Config,
        _: &'cx mut Context<S>,
    ) -> Option<Self::Output>
    where
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        if !value.is_double() {
            return None;
        }
        Some(unsafe { value.as_raw_ptr().to_double() })
    }
}

impl<'a, 'cx: 'a, C: Compartment> TryFromJs<'a, 'cx, C> for String {
    type Output = Self;

    type Config = ();

    fn try_from_js<S>(
        value: &JsValue<'a, C>,
        _config: Self::Config,
        cx: &'cx mut Context<S>,
    ) -> Option<Self::Output>
    where
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        if !value.is_string() {
            return None;
        }
        let js_string: JsString<'a, C> =
            unsafe { JsString::from_raw(value.as_raw_ptr().to_string()) };
        letroot!(rooted = js_string; [cx]);
        rooted.to_std_string(cx).ok()
    }
}

#[cfg(test)]
mod test {
    use crate::{
        letroot, setup_cx,
        value::{conversions::try_from_js::TryFromJs, JsValue, TryIntoJs},
    };

    #[test]
    fn i32_try_from_js() {
        setup_cx!(cx);
        letroot!(val = JsValue::i32(42, &mut cx); [cx]);
        let n = i32::try_from_js(&val, (), &mut cx).unwrap();
        assert_eq!(42, n);

        assert!(f64::try_from_js(&val, (), &mut cx).is_none())
    }

    #[test]
    fn f64_try_from_js() {
        setup_cx!(cx);
        letroot!(val = JsValue::f64(42.0, &mut cx); [cx]);
        let n = f64::try_from_js(&val, (), &mut cx).unwrap();
        assert_eq!(42.0, n);

        assert!(i32::try_from_js(&val, (), &mut cx).is_none())
    }

    #[test]
    fn string_try_from_js() {
        setup_cx!(cx);

        letroot!(val = "hello".try_into_js(&mut cx).unwrap(); [cx]);
        let s = String::try_from_js(&val, (), &mut cx).unwrap();
        assert_eq!("hello", s);

        letroot!(val = JsValue::f64(42.0, &mut cx); [cx]);
        assert!(String::try_from_js(&val, (), &mut cx).is_none());
    }
}
