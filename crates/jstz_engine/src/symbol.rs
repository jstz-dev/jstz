use indoc::indoc;
use mozjs::jsapi::{
    GetSymbolCode, GetSymbolDescription, GetSymbolFor, GetWellKnownSymbol, NewSymbol,
    Symbol as JSSymbol,
};

use crate::{
    context::{CanAccess, CanAlloc, Context, InCompartment},
    gc::{ptr::AsRawPtr, root::Rooted, Compartment},
    gcptr_wrapper,
    string::{JsString, RootedJsString},
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

pub type RootedJsSymbol<'a, C> = Rooted<'a, JsSymbol<'a, C>>;

impl<'a, C: Compartment> RootedJsSymbol<'a, C> {
    /// Returns the identifying code of a [Symbol].
    pub fn code(&self) -> SymbolCode {
        unsafe { GetSymbolCode(self.handle()).into() }
    }

    /// Returns the description of a [Symbol].
    /// Returns [None] for well-known symbols.
    pub fn description(&self) -> Option<JsString<'a, C>> {
        let raw_description = unsafe { GetSymbolDescription(self.handle()) };
        if !raw_description.is_null() {
            Some(unsafe { JsString::from_raw(raw_description) })
        } else {
            None
        }
    }
}

impl<'a, C: Compartment> JsSymbol<'a, C> {
    /// Creates a new unique [Symbol] with the given `description`
    pub fn new<S>(description: &RootedJsString<'_, C>, cx: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        unsafe { Self::from_raw(NewSymbol(cx.as_raw_ptr(), description.handle())) }
    }

    /// Gets an [Symbol] from the symbol registry with the given key. If not found,
    /// creates a new symbol in the registry and returns it
    pub fn from_key<S>(key: &RootedJsString<'_, C>, cx: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        unsafe { Self::from_raw(GetSymbolFor(cx.as_raw_ptr(), key.handle())) }
    }

    /// Creates a well-known symbol with its corresponding code
    pub fn from_well_known<S>(code: WellKnownSymbolCode, cx: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAccess + CanAlloc,
    {
        unsafe { Self::from_raw(GetWellKnownSymbol(cx.as_raw_ptr(), code.into())) }
    }
}

/// Represents a well-known symbol code.
///
/// Each of these refer to a property on the `Symbol` global object.
/// Refer to [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Symbol#static_properties) for more details.
///
/// # Safety
///
/// The ordering of enum members **must match** the ordering
/// in [`RawSymbolCode`]
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
            SymbolCode::WellKnown(unsafe {
                std::mem::transmute::<RawSymbolCode, WellKnownSymbolCode>(raw_code)
            })
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

#[cfg(test)]
mod test {
    use mozjs::jsapi::SymbolCode as RawSymbolCode;

    use crate::{
        gc::ptr::AsRawPtr,
        letroot, setup_cx,
        string::JsString,
        symbol::{JsSymbol, SymbolCode},
    };

    use super::WellKnownSymbolCode;

    #[test]
    fn well_known_safe_to_raw_roundtrips() {
        setup_cx!(cx);
        use WellKnownSymbolCode as Wksc;

        letroot!(sym = JsSymbol::from_well_known(Wksc::IsConcatSpreadable, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::isConcatSpreadable));
        assert_eq!(SymbolCode::WellKnown(Wksc::IsConcatSpreadable), raw.into());

        letroot!(sym = JsSymbol::from_well_known(Wksc::Iterator, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::iterator));
        assert_eq!(SymbolCode::WellKnown(Wksc::Iterator), raw.into());

        letroot!(sym = JsSymbol::from_well_known(Wksc::Match, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::match_));
        assert_eq!(SymbolCode::WellKnown(Wksc::Match), raw.into());

        letroot!(sym = JsSymbol::from_well_known(Wksc::Replace, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::replace));
        assert_eq!(SymbolCode::WellKnown(Wksc::Replace), raw.into());

        letroot!(sym = JsSymbol::from_well_known(Wksc::Search, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::search));
        assert_eq!(SymbolCode::WellKnown(Wksc::Search), raw.into());

        letroot!(sym = JsSymbol::from_well_known(Wksc::Species, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::species));
        assert_eq!(SymbolCode::WellKnown(Wksc::Species), raw.into());

        letroot!(sym = JsSymbol::from_well_known(Wksc::HasInstance, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::hasInstance));
        assert_eq!(SymbolCode::WellKnown(Wksc::HasInstance), raw.into());

        letroot!(sym = JsSymbol::from_well_known(Wksc::Split, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::split));
        assert_eq!(SymbolCode::WellKnown(Wksc::Split), raw.into());

        letroot!(sym = JsSymbol::from_well_known(Wksc::ToPrimitive, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::toPrimitive));
        assert_eq!(SymbolCode::WellKnown(Wksc::ToPrimitive), raw.into());

        letroot!(sym = JsSymbol::from_well_known(Wksc::ToStringTag, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::toStringTag));
        assert_eq!(SymbolCode::WellKnown(Wksc::ToStringTag), raw.into());

        letroot!(sym = JsSymbol::from_well_known(Wksc::Unscopables, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::unscopables));
        assert_eq!(SymbolCode::WellKnown(Wksc::Unscopables), raw.into());

        letroot!(sym = JsSymbol::from_well_known(Wksc::AsyncIterator, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::asyncIterator));
        assert_eq!(SymbolCode::WellKnown(Wksc::AsyncIterator), raw.into());

        letroot!(sym = JsSymbol::from_well_known(Wksc::MatchAll, &mut cx); [cx]);
        let raw = RawSymbolCode::from(sym.code());
        assert!(matches!(raw, RawSymbolCode::matchAll));
        assert_eq!(SymbolCode::WellKnown(Wksc::MatchAll), raw.into());
    }

    #[test]
    fn sym_registry_symbol() {
        setup_cx!(cx);

        letroot!(key = JsString::new("hello", &mut cx); [cx]);
        letroot!(sym = JsSymbol::from_key(&key, &mut cx); [cx]);
        let code = sym.code();
        let raw = RawSymbolCode::from(code);
        assert!(matches!(code, SymbolCode::InSymbolRegistry));
        assert_eq!(SymbolCode::InSymbolRegistry, raw.into());

        letroot!(sym2 = JsSymbol::from_key(&key, &mut cx); [cx]);
        unsafe { assert_eq!(sym.as_raw_ptr(), sym2.as_raw_ptr()) }

        letroot!(desc = sym.description().unwrap(); [cx]);
        assert_eq!("hello", desc.to_std_string(&mut cx).unwrap())
    }

    #[test]
    fn new_unique_symbol() {
        setup_cx!(cx);

        letroot!(key = JsString::new("hello", &mut cx); [cx]);
        letroot!(sym = JsSymbol::new(&key, &mut cx); [cx]);
        letroot!(sym2 = JsSymbol::new(&key, &mut cx); [cx]);
        let code = sym.code();
        let raw = RawSymbolCode::from(code);
        assert!(matches!(code, SymbolCode::UniqueSymbol));
        assert_eq!(SymbolCode::UniqueSymbol, raw.into());

        assert!(matches!(code, SymbolCode::UniqueSymbol));
        unsafe { assert_ne!(sym.as_raw_ptr(), sym2.as_raw_ptr()) };

        letroot!(desc = sym.description().unwrap(); [cx]);
        assert_eq!("hello", desc.to_std_string(&mut cx).unwrap())
    }
}
