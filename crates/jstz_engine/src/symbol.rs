use indoc::indoc;
use mozjs::jsapi::{
    GetSymbolCode, GetSymbolDescription, GetSymbolFor, GetWellKnownSymbol, NewSymbol,
    Symbol as JSSymbol,
};

use crate::{
    context::{CanAccess, CanAlloc, Context, InCompartment},
    gc::{
        ptr::{AsRawHandle, AsRawPtr},
        Compartment,
    },
    gcptr_wrapper, letroot,
    string::JsString,
};

pub use mozjs::jsapi::SymbolCode as RawSymbolCode;

gcptr_wrapper!(
    indoc! {"
    Represents a symbol in the JavaScript engine

    More information:
     - [MDN documentation](mdn)

    [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Symbol
  "},
    JsSymbol,
    *mut JSSymbol
);

impl<'a, C: Compartment> JsSymbol<'a, C> {
    pub fn new<'b, S>(description: &JsString<'b, C>, cx: &'a mut Context<S>) -> Self
    where
        'a: 'b,
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        letroot!(rooted_description = description.clone(); [cx]);

        unsafe { Self::from_raw(NewSymbol(cx.as_raw_ptr(), rooted_description.handle())) }
    }

    /// Gets a [Symbol] from the symbol registry with the given key.
    pub fn from_key<'b, S>(key: &JsString<'b, C>, cx: &'a mut Context<S>) -> Self
    where
        'a: 'b,
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        letroot!(rooted_key = key.clone(); [cx]);

        unsafe { Self::from_raw(GetSymbolFor(cx.as_raw_ptr(), rooted_key.handle())) }
    }

    /// Creates a well-known symbol with its corresponding code.
    pub fn from_well_known<S>(code: WellKnownSymbolCode, cx: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        unsafe { Self::from_raw(GetWellKnownSymbol(cx.as_raw_ptr(), code.into())) }
    }

    /// Returns the identifying code of a [Symbol].
    pub fn code(&self) -> SymbolCode {
        unsafe { GetSymbolCode(self.as_raw_handle()).into() }
    }

    /// Returns the description of a [Symbol].
    /// Returns [None] for well-known symbols.
    pub fn description(&self) -> Option<JsString<'a, C>> {
        let raw_description = unsafe { GetSymbolDescription(self.as_raw_handle()) };
        if !raw_description.is_null() {
            Some(unsafe { JsString::from_raw(raw_description) })
        } else {
            None
        }
    }
}

/// Represents a well-known symbol code.
///
/// Each of these refer to a property on the `Symbol` global object.
/// Refer to [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Symbol#static_properties) for more details.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[repr(u32)]
pub enum WellKnownSymbolCode {
    IsConcatSpreadable,
    Iterator,
    Match,
    Replace,
    Search,
    Species,
    HasInstance,
    Split,
    ToPrimitive,
    ToStringTag,
    Unscopables,
    AsyncIterator,
    MatchAll,
}

impl WellKnownSymbolCode {
    /// Converts a [WellKnownSymbolCode] into its corresponding identifier.
    /// These identifiers refer to the property names on the `Symbol` global object.
    pub const fn identifier(&self) -> &'static str {
        use WellKnownSymbolCode as Wksc;
        match self {
            Wksc::IsConcatSpreadable => "isConcatSpreadable",
            Wksc::Iterator => "iterator",
            Wksc::Match => "match",
            Wksc::Replace => "replace",
            Wksc::Search => "search",
            Wksc::Species => "species",
            Wksc::HasInstance => "hasInstance",
            Wksc::Split => "split",
            Wksc::ToPrimitive => "toPrimitive",
            Wksc::ToStringTag => "toStringTag",
            Wksc::Unscopables => "unscopables",
            Wksc::AsyncIterator => "asyncIterator",
            Wksc::MatchAll => "matchAll",
        }
    }
}

/// Represents the code of a [Symbol].
/// The code can be a [WellKnownSymbolCode], a private name symbol, a symbol within the registry, or a unique symbol.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum SymbolCode {
    WellKnown(WellKnownSymbolCode),
    PrivateNameSymbol,
    InSymbolRegistry,
    UniqueSymbol,
}

impl SymbolCode {
    /// Checks if a [SymbolCode] is a well-known symbol code.
    pub fn as_well_known(&self) -> Option<WellKnownSymbolCode> {
        if let SymbolCode::WellKnown(code) = self {
            Some(*code)
        } else {
            None
        }
    }
}

impl From<WellKnownSymbolCode> for SymbolCode {
    fn from(code: WellKnownSymbolCode) -> SymbolCode {
        SymbolCode::WellKnown(code)
    }
}

impl From<WellKnownSymbolCode> for RawSymbolCode {
    fn from(code: WellKnownSymbolCode) -> Self {
        unsafe { std::mem::transmute(code) }
    }
}

impl From<SymbolCode> for RawSymbolCode {
    fn from(code: SymbolCode) -> RawSymbolCode {
        use RawSymbolCode as Rsc;
        match code {
            SymbolCode::WellKnown(code) => code.into(),
            SymbolCode::PrivateNameSymbol => Rsc::PrivateNameSymbol,
            SymbolCode::InSymbolRegistry => Rsc::InSymbolRegistry,
            SymbolCode::UniqueSymbol => Rsc::UniqueSymbol,
        }
    }
}

impl From<RawSymbolCode> for SymbolCode {
    fn from(raw_code: RawSymbolCode) -> SymbolCode {
        if (raw_code as u32) < RawSymbolCode::Limit as u32 {
            SymbolCode::WellKnown(unsafe { std::mem::transmute(raw_code) })
        } else {
            use RawSymbolCode as Rsc;
            match raw_code {
                Rsc::PrivateNameSymbol => SymbolCode::PrivateNameSymbol,
                Rsc::InSymbolRegistry => SymbolCode::InSymbolRegistry,
                Rsc::UniqueSymbol => SymbolCode::UniqueSymbol,
                _ => unreachable!(),
            }
        }
    }
}
