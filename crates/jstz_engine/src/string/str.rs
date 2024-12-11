/// Inner representation of a [`JsStr`].
#[derive(Debug, Clone, Copy)]
pub enum JsStrVariant<'a> {
    /// Latin1 string representation.
    Latin1(&'a [u8]),

    /// U16 string representation.
    Utf16(&'a [u16]),
}

/// This is equivalent to Rust's `&str`.
#[derive(Debug, Clone, Copy)]
pub struct JsStr<'a> {
    inner: JsStrVariant<'a>,
}

impl<'a> JsStr<'a> {
    /// This represents an empty string.
    pub const EMPTY: Self = Self::latin1("".as_bytes());

    /// Creates a [`JsStr`] from codepoints that can fit in a `u8`.
    #[inline]
    #[must_use]
    pub const fn latin1(value: &'a [u8]) -> Self {
        Self {
            inner: JsStrVariant::Latin1(value),
        }
    }

    /// Creates a [`JsStr`] from utf16 encoded string.
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

    /// Convert the [`JsStr`] into a [`Vec<U16>`].
    #[inline]
    #[must_use]
    pub fn to_vec(&self) -> Vec<u16> {
        match self.variant() {
            JsStrVariant::Latin1(v) => v.iter().copied().map(u16::from).collect(),
            JsStrVariant::Utf16(v) => v.to_vec(),
        }
    }
}
