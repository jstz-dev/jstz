use std::ptr;

use mozjs::jsapi::{
    JSString, JS_ConcatStrings, JS_DeprecatedStringHasLatin1Chars,
    JS_GetLatin1StringCharsAndLength, JS_GetStringCharAt, JS_GetStringLength,
    JS_GetTwoByteStringCharsAndLength,
};

use crate::{
    compartment::Compartment,
    context::{CanAccess, CanAlloc, Context, InCompartment},
    gc::Gc,
};

mod str;

use str::JsStr;

pub struct JsString<'a, C: Compartment> {
    inner: Gc<'a, C, JSString>,
}

impl<'a, C: Compartment> Clone for JsString<'a, C> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<'a, C: Compartment> JsString<'a, C> {
    pub fn new(inner: *mut JSString) -> Self {
        Self {
            inner: Gc::from(inner),
        }
    }

    pub unsafe fn as_moz_ptr(&self) -> *mut JSString {
        self.inner.inner_ptr.get()
    }

    /// Checks if the string consists of only Latin-1 characters.
    pub fn is_latin1(&self) -> bool {
        unsafe { JS_DeprecatedStringHasLatin1Chars(self.as_moz_ptr()) }
    }

    /// Checks if the string consists of UTF-16 characters.
    pub fn is_utf16(&self) -> bool {
        !self.is_latin1()
    }

    /// Obtains a slice of a [`JsString`] as a [`JsStr`].
    pub fn as_str<'cx, S>(&self, cx: &'cx Context<S>) -> JsStr<'a>
    where
        S: InCompartment<C> + CanAccess,
        'cx: 'a,
    {
        let mut len = 0;
        let nogc = ptr::null();

        if self.is_latin1() {
            let raw = unsafe {
                JS_GetLatin1StringCharsAndLength(
                    cx.as_moz_ptr(),
                    nogc,
                    self.as_moz_ptr(),
                    &mut len,
                )
            };
            JsStr::latin1(unsafe { std::slice::from_raw_parts(raw, len) })
        } else {
            let raw = unsafe {
                JS_GetTwoByteStringCharsAndLength(
                    cx.as_moz_ptr(),
                    nogc,
                    self.as_moz_ptr(),
                    &mut len,
                )
            };

            JsStr::utf16(unsafe { std::slice::from_raw_parts(raw, len) })
        }
    }

    /// Get the length of the [`JsString`].
    pub fn len(&self) -> usize {
        unsafe { JS_GetStringLength(self.as_moz_ptr()) }
    }

    /// Return true if the [`JsString`] is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn concat<'cx, S>(&self, other: &Self, cx: &'cx mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
        'cx: 'a,
    {
        Self::new(unsafe {
            JS_ConcatStrings(cx.as_moz_ptr(), self.as_moz_handle(), other.as_moz_handle())
        })
    }

    pub fn char_at<'cx, S>(&self, index: usize, cx: &'cx Context<S>) -> u16
    where
        S: InCompartment<C> + CanAccess,
        'cx: 'a,
    {
        unsafe {
            let mut char = 0;
            JS_GetStringCharAt(cx.as_moz_ptr(), self.as_moz_ptr(), index, &mut char);
            char
        }
    }

    pub fn to_std_string<'cx, S>(&self, cx: &'cx Context<S>) -> anyhow::Result<String>
    where
        S: InCompartment<C> + CanAccess,
    {
        match self.as_str(cx).variant() {
            str::JsStrVariant::Latin1(slice) => Ok(String::from_utf8(slice.to_vec())?),
            str::JsStrVariant::Utf16(slice) => Ok(String::from_utf16(slice)?),
        }
    }
}
