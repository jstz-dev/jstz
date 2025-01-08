use std::{ffi::c_char, ptr};

use indoc::indoc;
use mozjs::jsapi::{
    JSString, JS_ConcatStrings, JS_DeprecatedStringHasLatin1Chars, JS_GetEmptyString,
    JS_GetLatin1StringCharsAndLength, JS_GetStringCharAt, JS_GetStringLength,
    JS_GetTwoByteStringCharsAndLength, JS_NewStringCopyN, JS_NewUCStringCopyN,
    JS_StringEqualsAscii,
};

use crate::{
    context::{CanAccess, CanAlloc, Context, InCompartment},
    gc::{ptr::AsRawPtr, Compartment},
    gcptr_wrapper, letroot,
};

mod str;

use str::{JsStr, JsStrVariant};

gcptr_wrapper!(
    indoc! {"
        A Latin1 or UTF-16-encoded string. 

        More information:
         - [MDN documentation](mdn)
         - [EMCAScript reference](spec)

        [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/String
        [spec]: https://tc39.es/ecma262/#sec-ecmascript-language-types-string-type
    "},
    JsString,
    *mut JSString
);

impl<'a, C: Compartment> JsString<'a, C> {
    /// Creates a new empty [`JsString`]
    pub fn new<S>(cx: &'a Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe { Self::from_raw(JS_GetEmptyString(cx.as_raw_ptr())) }
    }

    /// Creates a new [`JsString`] from `slice`.
    pub fn from_slice<S>(slice: JsStr<'_>, cx: &'a Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe {
            match slice.variant() {
                JsStrVariant::Latin1(slice) => Self::from_raw(JS_NewStringCopyN(
                    cx.as_raw_ptr(),
                    slice.as_ptr() as *const c_char,
                    slice.len(),
                )),
                JsStrVariant::Utf16(slice) => Self::from_raw(JS_NewUCStringCopyN(
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

    /// Obtains a slice of a [`JsString`] as a [`JsStr`].
    pub fn as_str<'cx, S>(&self, cx: &'cx Context<S>) -> JsStr<'cx>
    where
        S: InCompartment<C> + CanAccess,
        'cx: 'a,
    {
        let mut len = 0;

        // SAFETY: The caller is required to ensure we do not GC
        //         as long as the return type is used. We do this
        //         by having `JsStr` have the lifetime `'cx`.
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

    /// Get the length of the [`JsString`].
    pub fn len(&self) -> usize {
        unsafe { JS_GetStringLength(self.as_raw_ptr()) }
    }

    /// Return true if the [`JsString`] is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Concatenates two [`JsString`]s into a new [`JsString`].
    pub fn concat<'cx, 'b, S>(
        &self,
        other: &JsString<'b, C>,
        cx: &'cx mut Context<S>,
    ) -> JsString<'cx, C>
    where
        S: InCompartment<C> + CanAlloc,
        'cx: 'a,
    {
        // SAFETY: Root both self and other to obtain handles
        //        (since `JS_ConcatStrings` will allocate and maybe GC)
        letroot!(rooted_self = self.clone(); [cx]);
        letroot!(rooted_other = other.clone(); [cx]);

        unsafe {
            JsString::from_raw(JS_ConcatStrings(
                cx.as_raw_ptr(),
                rooted_self.handle(),
                rooted_other.handle(),
            ))
        }
    }

    /// Returns the UTF-16 codepoint at the given character.
    pub fn code_point_at<'cx, S>(&self, index: usize, cx: &'cx Context<S>) -> u16
    where
        S: InCompartment<C> + CanAccess,
        'cx: 'a,
    {
        unsafe {
            let mut char = 0;
            JS_GetStringCharAt(cx.as_raw_ptr(), self.as_raw_ptr(), index, &mut char);
            char
        }
    }

    /// Returns `Some(true)` if the string contains the substring `search_value`.
    ///
    /// # Notes
    ///
    /// Returns `None` if `search_value` is not null-terminated
    pub fn contains<S>(&self, search_value: &str, cx: &Context<S>) -> Option<bool>
    where
        S: InCompartment<C> + CanAccess,
    {
        if !search_value.ends_with('\0') {
            return None;
        }
        let mut match_success = false;

        let result = unsafe {
            JS_StringEqualsAscii(
                cx.as_raw_ptr(),
                self.as_raw_ptr(),
                search_value.as_ptr() as *const c_char,
                &mut match_success,
            )
        };

        if result {
            Some(match_success)
        } else {
            None
        }
    }

    /// Converts a [`JsString`] to an owned Rust string [`String`].
    pub fn to_std_string<S>(&self, cx: &Context<S>) -> anyhow::Result<String>
    where
        S: InCompartment<C> + CanAccess,
    {
        match self.as_str(cx).variant() {
            JsStrVariant::Latin1(slice) => Ok(String::from_utf8(slice.to_vec())?),
            JsStrVariant::Utf16(slice) => Ok(String::from_utf16(slice)?),
        }
    }
}
