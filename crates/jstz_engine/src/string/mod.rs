#![allow(unused)]

use std::{
    env::consts,
    ffi::{c_char, CStr, CString},
    marker::PhantomData,
    pin::Pin,
    ptr,
    sync::Arc,
};

use indoc::indoc;
use mozjs::{
    conversions::ToJSValConvertible,
    jsapi::{
        JSString, JS_CompareStrings, JS_ConcatStrings, JS_DeprecatedStringHasLatin1Chars,
        JS_GetEmptyString, JS_GetLatin1StringCharsAndLength, JS_GetStringCharAt,
        JS_GetStringLength, JS_GetTwoByteStringCharsAndLength, JS_NewStringCopyN,
        JS_NewUCStringCopyN, JS_StringEqualsAscii,
    },
    rust::jsapi_wrapped,
};

use crate::{
    context::{CanAccess, CanAlloc, Context, InCompartment},
    custom_trace,
    gc::{
        compartment::Compartment,
        ptr::{AsRawHandle, AsRawHandleMut, AsRawPtr, GcPtr, Handle, HandleMut},
        root::Rooted,
        Finalize, Prolong, Trace,
    },
    gcptr_wrapper, letroot,
    value::JsValue,
};

mod str;

use str::{JsStr, JsStrVariant};

gcptr_wrapper!(
    indoc! {"
        [`JsString`] represents a Javascript string. Javascript strings are encoded as
        UTF-16.

        Latin-1 was initially supported by SpiderMonkey for backward compatibility. However,
        it is now used to represent more compact strings ie. If the string only contains
        codepoints that are <= 255, then it can be represented as latin1  which uses [u8] 
        rather than [u16].

        The encoding scheme is abstracted from the users once a valid *mut JSString is
        constructed.

         More information:
         - [EMCAScript reference][spec]

        [spec]: https://tc39.es/ecma262/#sec-ecmascript-language-types-string-type
"},
    JsString,
    *mut JSString
);

/// Returns true if `s` is latin1 encodable ie. All codepoints
/// in the string are less than or equal to 255
fn latin1_encodable(s: &str) -> bool {
    s.chars().all(|c| c as u32 <= 0xFF)
}

/// A [`JsString`] that has been rooted in some `context`.
///
/// There is an implicit assumption that the `context` used in any of the
/// fn calls is the `context` that the `&self` was rooted in. Using an
/// alternative context is UB
type RootedJsString<'a, C> = Rooted<'a, JsString<'a, C>>;

impl<'a, C: Compartment> RootedJsString<'a, C> {
    /// Converts a [`JsString`] to an owned Rust string [`String`].
    pub fn to_std_string<S>(&self, cx: &mut Context<S>) -> anyhow::Result<String>
    where
        S: InCompartment<C> + CanAccess,
    {
        match self.as_str(cx).variant() {
            JsStrVariant::Latin1(slice) => Ok(String::from_utf8(slice.to_vec())?),
            JsStrVariant::Utf16(slice) => Ok(String::from_utf16(slice)?),
        }
    }

    /// Obtains a slice of a [`JsString`] as a [`JsStr`].
    pub fn as_str<'cx, S>(&self, cx: &'cx mut Context<S>) -> JsStr<'cx>
    where
        S: InCompartment<C> + CanAccess,
    {
        let mut len = 0;

        // # SAFETY
        //
        // The caller is required to ensure we do not GC as long as
        // the return type is used. We do this by having `JsStr` have
        // the lifetime `'cx`.
        let nogc = ptr::null();

        if self.is_latin1() {
            let raw = unsafe {
                JS_GetLatin1StringCharsAndLength(
                    cx.as_raw_ptr(),
                    nogc,
                    self.as_raw_ptr(),
                    &mut len,
                )
            };
            JsStr::latin1(unsafe { std::slice::from_raw_parts(raw, len) })
        } else {
            let raw = unsafe {
                JS_GetTwoByteStringCharsAndLength(
                    cx.as_raw_ptr(),
                    nogc,
                    self.as_raw_ptr(),
                    &mut len,
                )
            };

            JsStr::utf16(unsafe { std::slice::from_raw_parts(raw, len) })
        }
    }

    /// Concatenates two [`JsString`]s into a new [`JsString`].
    pub fn concat<'cx, S>(
        &self,
        other: &RootedJsString<'_, C>,
        cx: &'cx mut Context<S>,
    ) -> JsString<'cx, C>
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe {
            JsString::from_raw(JS_ConcatStrings(
                cx.as_raw_ptr(),
                self.handle(),
                other.handle(),
            ))
        }
    }

    /// Returns the UTF-16 codepoint at the given character.
    pub fn code_point_at<S>(&self, index: usize, cx: &Context<S>) -> u16
    where
        S: InCompartment<C> + CanAccess,
    {
        unsafe {
            let mut char = 0;
            JS_GetStringCharAt(cx.as_raw_ptr(), self.as_raw_ptr(), index, &mut char);
            char
        }
    }

    /// Returns `Some(true)` if the string equals `std_str`.
    ///
    /// # Notes
    ///
    /// Returns `None` if the string failed to compare
    pub fn equals_std<S>(&self, value: &str, cx: &mut Context<S>) -> Option<bool>
    where
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        letroot!(v = JsString::<C>::new(value, cx); [cx]);
        self.equals(&v, cx)
    }

    pub fn equals<S>(&self, s2: &RootedJsString<'_, C>, cx: &Context<S>) -> Option<bool>
    where
        S: InCompartment<C> + CanAccess,
    {
        let mut match_success: i32 = -1;
        let result = unsafe {
            JS_CompareStrings(
                cx.as_raw_ptr(),
                self.as_raw_ptr(),
                s2.as_raw_ptr(),
                &mut match_success,
            )
        };

        if result {
            Some(match_success == 0)
        } else {
            None
        }
    }
}

/// Note: All functions that create a JsString must take a `&'cx mut Context`
/// and return JsString<'cx, C>. This is to enforce that the return object must
/// be rooted or dropped before `context` can be used again
impl<'a, C: Compartment> JsString<'a, C> {
    /// Creates a new empty [`JsString`]
    pub fn empty<S>(cx: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe { Self::from_raw(JS_GetEmptyString(cx.as_raw_ptr())) }
    }

    /// Creates a new [`JsString`] from `std_str`
    pub fn new<S>(std_str: &'_ str, cx: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        if std_str.is_empty() {
            unsafe { Self::from_raw(JS_GetEmptyString(cx.as_raw_ptr())) }
        } else if latin1_encodable(std_str) {
            Self::from_slice(JsStr::latin1(std_str.as_bytes()), cx)
        } else {
            let mut buf = Vec::with_capacity(std_str.len());
            buf.extend(std_str.encode_utf16());
            Self::from_slice(JsStr::utf16(buf.as_slice()), cx)
        }
    }

    /// Creates a new [`JsString`] from `slice`.
    pub fn from_slice<S>(slice: JsStr<'_>, cx: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe {
            /* https://github.com/servo/mozjs/blob/main/mozjs-sys/mozjs/js/public/String.h#L52
             *
             * String creation.
             *
             * NB: JS_NewUCString takes ownership of bytes on success, avoiding a copy;
             * but on error (signified by null return), it leaves chars owned by the
             * caller. So the caller must free bytes in the error case, if it has no use
             * for them. In contrast, all the JS_New*StringCopy* functions do not take
             * ownership of the character memory passed to them -- they copy it.
             */
            match slice.variant() {
                JsStrVariant::Latin1(slice) => JsString::from_raw(JS_NewStringCopyN(
                    cx.as_raw_ptr(),
                    slice.as_ptr() as *const c_char,
                    slice.len(),
                )),
                JsStrVariant::Utf16(slice) => JsString::from_raw(JS_NewUCStringCopyN(
                    cx.as_raw_ptr(),
                    slice.as_ptr(),
                    slice.len(),
                )),
            }
        }
    }

    /// Checks if the string consists of only Latin-1 characters.
    pub fn is_latin1(&self) -> bool {
        unsafe { JS_DeprecatedStringHasLatin1Chars(self.as_raw_ptr()) }
    }

    /// Checks if the string consists of UTF-16 characters.
    pub fn is_utf16(&self) -> bool {
        !self.is_latin1()
    }

    /// Get the length of the [`JsString`].
    pub fn len(&self) -> usize {
        unsafe { JS_GetStringLength(self.as_raw_ptr()) }
    }

    /// Return true if the [`JsString`] is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod test {

    use mozjs::{
        jsval::StringValue,
        rust::{JSEngine, Runtime},
    };

    use crate::{
        alloc_compartment,
        context::Context,
        setup_cx,
        string::{str::JsStr, JsString},
    };

    use super::*;

    const VALUE: &str = "hello world🚀";

    fn utf16_value() -> Vec<u16> {
        VALUE.encode_utf16().collect()
    }

    #[test]
    fn from_raw() {
        setup_cx!(cx);

        letroot!(js_string = unsafe { JsString::from_raw(JS_GetEmptyString(cx.as_raw_ptr())) }; [cx]);
        assert!(js_string.equals_std("", &mut cx).unwrap())
    }

    #[test]
    fn from_slice_utf16() {
        setup_cx!(cx);

        letroot!(js_string =
            JsString::from_slice(JsStr::utf16(utf16_value().as_slice()), &mut cx); [cx]);
        assert_eq!(Some(true), js_string.equals_std(VALUE, &mut cx));
        assert_eq!(Some(false), js_string.equals_std("hello", &mut cx));
    }

    #[test]
    fn from_slice_latin1() {
        setup_cx!(cx);

        letroot!(js_string =
            JsString::from_slice(JsStr::latin1("hello world".as_bytes()), &mut cx); [cx]);
        assert_eq!(Some(true), js_string.equals_std("hello world", &mut cx));
        assert_eq!(Some(false), js_string.equals_std("hello", &mut cx));
    }

    #[test]
    fn new() {
        setup_cx!(cx);

        // latin1 string
        letroot!(js_string = JsString::new("hello world", &mut cx); [cx]);
        assert!(js_string.is_latin1());
        assert!(!js_string.is_utf16());
        assert!(js_string.equals_std("hello world", &mut cx).unwrap());

        // utf16 string
        letroot!(js_string = JsString::new("hello world🚀", &mut cx); [cx]);
        assert!(js_string.is_utf16());
        assert!(!js_string.is_latin1());
        assert!(js_string.equals_std("hello world🚀", &mut cx).unwrap())
    }

    #[test]
    fn as_str() {
        setup_cx!(cx);

        // utf16 initial value -> utf16
        let value = utf16_value();
        let utf16 = JsStr::utf16(value.as_slice());
        letroot!(js_string = JsString::from_slice(utf16, &mut cx); [cx]);
        let js_str = js_string.as_str(&mut cx);
        assert_eq!(utf16, js_str);

        // latin1 initial value -> latin1
        let latin1 = JsStr::latin1(VALUE.as_bytes());
        letroot!(js_string = JsString::from_slice(latin1, &mut cx); [cx]);
        let js_str = js_string.as_str(&mut cx);
        assert_eq!(latin1, js_str);

        // latin1 encoded as utf16 -> latin1
        let latin1_as_utf16: Vec<u16> = "hello world".encode_utf16().collect();
        let utf16 = JsStr::utf16(latin1_as_utf16.as_slice());
        letroot!(js_string = JsString::from_slice(utf16, &mut cx); [cx]);
        let js_str = js_string.as_str(&mut cx);
        assert_eq!(JsStr::latin1("hello world".as_bytes()), js_str);
    }

    #[test]
    fn code_point_at() {
        setup_cx!(cx);

        letroot!(js_string = JsString::new("a🌟", &mut cx); [cx]);
        assert_eq!(97, js_string.code_point_at(0, &cx));
        assert_eq!(55356, js_string.code_point_at(1, &cx));
    }

    #[test]
    fn concat() {
        setup_cx!(cx);

        // Safety: These are perfecly safe to extend lifetime in this test as s1
        // and s2 are rooted in `concat`
        letroot!(hello = JsString::new("hello", &mut cx); [cx]);
        letroot!(world = JsString::new("world", &mut cx); [cx]);
        letroot!(rocket = JsString::new("🚀", &mut cx); [cx]);

        letroot!(result = hello.concat(&world, &mut cx); [cx]);
        assert_eq!(Some(true), result.equals_std("helloworld", &mut cx));

        letroot!(result = hello.concat(&rocket, &mut cx); [cx]);
        assert_eq!(Some(true), result.equals_std("hello🚀", &mut cx));

        letroot!(result = rocket.concat(&world, &mut cx); [cx]);
        assert_eq!(Some(true), result.equals_std("🚀world", &mut cx));

        letroot!(result = rocket.concat(&rocket, &mut cx); [cx]);
        assert_eq!(Some(true), result.equals_std("🚀🚀", &mut cx));
        assert_eq!(Some(false), result.equals_std("🚀", &mut cx));
    }

    #[test]
    fn equals() {
        setup_cx!(cx);

        // Check utf16 strings are equal
        letroot!(js_string = JsString::from_slice(JsStr::utf16(utf16_value().as_slice()), &mut cx); [cx]);
        assert_eq!(Some(true), js_string.equals_std(VALUE, &mut cx));
        assert_eq!(Some(false), js_string.equals_std("hello", &mut cx));

        // Check latin1 strings are equal
        letroot!(
            js_string =
                JsString::from_slice(JsStr::latin1("hello world".as_bytes()), &mut cx); [cx]
        );
        assert_eq!(Some(true), js_string.equals_std("hello world", &mut cx));

        // Check utf16 encoded latin1 strings are equal;
        letroot!(js_string = JsString::from_slice(
            JsStr::utf16(
                "hello world"
                    .encode_utf16()
                    .collect::<Vec<u16>>()
                    .as_slice(),
            ),
            &mut cx,
        ); [cx]);
        assert_eq!(Some(true), js_string.equals_std("hello world", &mut cx));
    }

    #[test]
    fn is_empty() {
        setup_cx!(cx);

        let js_string = JsString::empty(&mut cx);
        assert!(js_string.is_empty())
    }

    #[test]
    fn is_latin1() {
        setup_cx!(cx);

        // Raw 8-bit char strings are encoded as Latin-1 athough this will produce
        // gibberish when console.logged
        let v = "hello world";
        let v_u8 = v.as_bytes();
        let js_string = JsString::from_slice(JsStr::latin1(v_u8), &mut cx);
        assert!(js_string.is_latin1());

        // Since Latin-1 code points are subset of utf16 code points, a utf16
        // encoded string that uses only Latin-1 code points (0-255) will be represented
        // as Latin-1 string
        let v_utf16: Vec<u16> = v.encode_utf16().collect();
        let js_string = JsString::from_slice(JsStr::utf16(v_utf16.as_slice()), &mut cx);
        assert!(js_string.is_latin1());
    }

    #[test]
    fn is_utf16() {
        setup_cx!(cx);

        let js_string =
            JsString::from_slice(JsStr::utf16(utf16_value().as_slice()), &mut cx);
        assert!(js_string.is_utf16())
    }

    #[test]
    fn len() {
        setup_cx!(cx);

        let js_string =
            JsString::from_slice(JsStr::utf16(utf16_value().as_slice()), &mut cx);
        assert_eq!(utf16_value().len(), js_string.len())
    }

    #[test]
    fn to_std_string() {
        setup_cx!(cx);

        letroot!(js_string = JsString::from_slice(JsStr::utf16(utf16_value().as_slice()), &mut cx); [cx]);
        assert_eq!(VALUE.to_string(), js_string.to_std_string(&mut cx).unwrap())
    }
}
