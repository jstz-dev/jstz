use std::{ffi::CStr, marker::PhantomData};

use indoc::indoc;
use mozjs::jsapi::{
    mozilla::{Range, RangedPtr},
    BigInt as JSBigInt, BigIntFitsNumber, BigIntFromBool, BigIntFromInt64,
    BigIntFromUint64, BigIntIsInt64, BigIntIsNegative, BigIntIsUint64, BigIntToNumber,
    BigIntToString, NumberToBigInt, SimpleStringToBigInt, StringToBigInt1,
};

use crate::{
    context::{CanAccess, CanAlloc, Context, InCompartment},
    gc::{ptr::AsRawPtr, Compartment},
    gcptr_wrapper, letroot,
    string::JsString,
};

gcptr_wrapper!(indoc! {""}, JsBigInt, *mut JSBigInt);

impl<'a, C: Compartment> JsBigInt<'a, C> {
    pub fn zero<S>(cx: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        Self::from_bool(false, cx)
    }

    pub fn from_bool<S>(b: bool, cx: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe { Self::from_raw(BigIntFromBool(cx.as_raw_ptr(), b)) }
    }

    pub fn from_i64<S>(n: i64, cx: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe { Self::from_raw(BigIntFromInt64(cx.as_raw_ptr(), n)) }
    }

    pub fn from_u64<S>(n: u64, cx: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe { Self::from_raw(BigIntFromUint64(cx.as_raw_ptr(), n)) }
    }

    pub fn from_f64<S>(n: f64, cx: &'a mut Context<S>) -> Option<Self>
    where
        S: InCompartment<C> + CanAlloc,
    {
        let big_int = unsafe { NumberToBigInt(cx.as_raw_ptr(), n) };
        if !big_int.is_null() {
            Some(unsafe { Self::from_raw(big_int) })
        } else {
            None
        }
    }

    pub fn from_string<S>(str: &str, cx: &'a mut Context<S>) -> Option<Self>
    where
        S: InCompartment<C> + CanAlloc,
    {
        let mut string: Vec<u16> = str.encode_utf16().collect();
        let range = string.as_mut_ptr_range();
        let raw_range = Range {
            mStart: RangedPtr {
                mPtr: range.start,
                _phantom_0: PhantomData,
            },
            mEnd: RangedPtr {
                mPtr: range.end,
                _phantom_0: PhantomData,
            },
            _phantom_0: PhantomData,
        };

        let raw_big_int = unsafe { StringToBigInt1(cx.as_raw_ptr(), &raw_range) };
        if raw_big_int.is_null() {
            None
        } else {
            Some(unsafe { Self::from_raw(raw_big_int) })
        }
    }

    pub fn to_string<'cx, S>(&self, cx: &'cx mut Context<S>) -> Option<JsString<'cx, C>>
    where
        'cx: 'a,
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        Self::to_string_radix(&self, 10, cx)
    }

    pub fn to_string_radix<'cx, S>(
        &self,
        radix: u8,
        cx: &'cx mut Context<S>,
    ) -> Option<JsString<'cx, C>>
    where
        'cx: 'a,
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        if !(2..=36).contains(&radix) {
            None
        } else {
            letroot!(rooted_self = self.clone(); [cx]);
            let raw_string =
                unsafe { BigIntToString(cx.as_raw_ptr(), rooted_self.handle(), radix) };
            Some(unsafe { JsString::from_raw(raw_string) })
        }
    }

    pub fn from_string_radix<S>(
        buf: &CStr,
        radix: u8,
        cx: &'a mut Context<S>,
    ) -> Option<Self>
    where
        S: InCompartment<C> + CanAlloc,
    {
        let bytes = buf.to_bytes_with_nul();
        let range = bytes.as_ptr_range();
        let raw_range = [range.start as u64, range.end as u64];

        let raw_big_int =
            unsafe { SimpleStringToBigInt(cx.as_raw_ptr(), raw_range, radix) };
        if raw_big_int.is_null() {
            None
        } else {
            Some(unsafe { Self::from_raw(raw_big_int) })
        }
    }

    /// Converts a [BigInt] to a 64-bit signed integer if possible.
    pub fn to_i64(&self) -> Option<i64> {
        let mut result = 0;
        unsafe { BigIntIsInt64(self.as_raw_ptr(), &mut result).then_some(result) }
    }

    /// Converts a [BigInt] to a 64-bit unsigned integer if possible.
    pub fn to_u64(&self) -> Option<u64> {
        let mut result = 0;
        unsafe { BigIntIsUint64(self.as_raw_ptr(), &mut result).then_some(result) }
    }

    /// Converts a [BigInt] to a double.
    /// Returns `Infinity` or `-Infinity` if it does not fit in a double.
    pub fn to_f64(&self) -> f64 {
        unsafe { BigIntToNumber(self.as_raw_ptr()) }
    }

    /// Converts a [BigInt] to a double if it fits in a double.
    pub fn fits_f64(&self) -> Option<f64> {
        let mut result = 0.0;
        unsafe { BigIntFitsNumber(self.as_raw_ptr(), &mut result).then_some(result) }
    }

    /// Checks if the [BigInt] is negative.
    pub fn is_negative(&self) -> bool {
        unsafe { BigIntIsNegative(self.as_raw_ptr()) }
    }
}
