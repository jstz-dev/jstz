/// Inner representation of a [`JsStr`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JsStrVariant<'a> {
    /// Latin1 string representation.
    Latin1(&'a [u8]),

    /// U16 string representation.
    Utf16(&'a [u16]),
}

/// This is equivalent to Rust's `&str`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JsStr<'a> {
    inner: JsStrVariant<'a>,
}

impl<'a> JsStr<'a> {
    /// This represents an empty string.
    pub const EMPTY: Self = Self::latin1("".as_bytes());

    /// Creates a [`JsStr`] from codepoints that can fit in a `u8`.
    /// The caller is responsible for ensuring the given string is
    /// indeed a valid Latin-1 string
    #[inline]
    #[must_use]
    pub const fn latin1(value: &'a [u8]) -> Self {
        Self {
            inner: JsStrVariant::Latin1(value),
        }
    }

    /// Creates a [`JsStr`] from utf16 encoded string.
    /// The caller is responsible for ensuring the given string is
    /// indeed a valid utf16 string
    #[inline]
    #[must_use]
    pub const fn utf16(value: &'a [u16]) -> Self {
        Self {
            inner: JsStrVariant::Utf16(value),
        }
    }

    /// Get the length of the [`JsStr`].
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        match self.inner {
            JsStrVariant::Latin1(v) => v.len(),
            JsStrVariant::Utf16(v) => v.len(),
        }
    }

    /// Return the inner [`JsStrVariant`] varient of the [`JsStr`].
    #[inline]
    #[must_use]
    pub const fn variant(self) -> JsStrVariant<'a> {
        self.inner
    }

    /// Check if the [`JsStr`] is latin1 encoded.
    #[inline]
    #[must_use]
    pub const fn is_latin1(&self) -> bool {
        matches!(self.inner, JsStrVariant::Latin1(_))
    }

    /// Returns [`u8`] slice if the [`JsStr`] is latin1 encoded, otherwise [`None`].
    #[inline]
    #[must_use]
    pub const fn as_latin1(&self) -> Option<&[u8]> {
        if let JsStrVariant::Latin1(slice) = self.inner {
            return Some(slice);
        }

        None
    }

    /// Check if the [`JsStr`] is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Convert the [`JsStr`] into a [`Vec<u16>`].
    #[inline]
    #[must_use]
    pub fn code_points(&self) -> Vec<u16> {
        match self.variant() {
            JsStrVariant::Latin1(v) => v.iter().copied().map(u16::from).collect(),
            JsStrVariant::Utf16(v) => v.to_vec(),
        }
    }
}

#[cfg(test)]
mod test {

    use crate::setup_cx;

    use super::*;

    #[test]
    fn latin1() {
        setup_cx!(cx);

        // Any 8 bit string can be represented as Latin-1
        let v = [0xff, 0xff, 0xff];
        let js_str = JsStr::latin1(v.as_slice());
        assert!(matches!(
            js_str,
            JsStr {
                inner: JsStrVariant::Latin1(v)
            }
        ))
    }

    #[test]
    fn utf16() {
        setup_cx!(cx);

        // Any 16 bit string can be represented as utf16
        let v = [0xffab, 0xffcd, 0xffff];
        let js_str = JsStr::utf16(v.as_slice());
        assert!(matches!(
            js_str,
            JsStr {
                inner: JsStrVariant::Utf16(v)
            }
        ));
    }

    #[test]
    fn len() {}

    #[test]
    fn is_latin() {
        assert!(JsStr::latin1("jstz".as_bytes()).is_latin1());
        assert!(!JsStr::utf16(&[]).is_latin1())
    }

    fn as_latin1() {
        assert!(JsStr::latin1("jstz".as_bytes()).as_latin1().is_some());
    }

    fn is_empty() {
        assert!(JsStr::latin1(&[]).is_empty());
        assert!(JsStr::utf16(&[]).is_empty());
    }

    fn code_points() {
        let code_points: Vec<u16> = "jstz".chars().map(|c| c as u16).collect();
        let latin1_cp = JsStr::latin1("jstz".as_bytes()).code_points();
        assert_eq!(code_points, latin1_cp);
        let utf16_cp =
            JsStr::utf16("jstz".encode_utf16().collect::<Vec<u16>>().as_slice())
                .code_points();
        assert_eq!(code_points, utf16_cp);

        let cp: Vec<u16> = "⭐️🤖🚀".encode_utf16().collect();
        let utf16_cp = JsStr::utf16(utf16_cp.as_slice()).code_points();
        assert_eq!(cp, utf16_cp)
    }
}
