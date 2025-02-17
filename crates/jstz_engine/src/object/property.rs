use bitflags::bitflags;
use mozjs::jsapi::{
    jsid, JSITER_FORAWAITOF, JSITER_HIDDEN, JSITER_OWNONLY, JSITER_PRIVATE,
    JSITER_SYMBOLS, JSITER_SYMBOLSONLY, JSPROP_ENUMERATE, JSPROP_PERMANENT,
    JSPROP_READONLY, JSPROP_RESOLVING,
};

use crate::{
    context::{CanAlloc, Context, InCompartment},
    gc::{compartment::Compartment, Prolong},
    gcptr_wrapper, letroot,
    string::JsString,
};
use indoc::indoc;

gcptr_wrapper!(
    indoc! {"
        [`PropertyKey`] represents an object property key in the Javascript Engine.
    "},
    PropertyKey,
    jsid
);

impl<'a, C: Compartment> PropertyKey<'a, C> {
    pub fn empty<S>(_cx: &mut Context<S>) -> PropertyKey<C>
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe { PropertyKey::from_raw(jsid { asBits_: 0 }) }
    }
}

pub trait IntoPropertyKey<C: Compartment> {
    /// Converts `self` into a new [`PropertyKey`].
    /// Returns [`None`] when conversion fails.
    fn into_key<S>(self, cx: &mut Context<S>) -> Option<PropertyKey<C>>
    where
        S: InCompartment<C> + CanAlloc;
}

impl<'a, C: Compartment> IntoPropertyKey<C> for PropertyKey<'a, C> {
    fn into_key<S>(self, _cx: &mut Context<S>) -> Option<PropertyKey<C>>
    where
        S: InCompartment<C> + CanAlloc,
    {
        // # Safety
        //
        // `self` was created from `_cx` so safe
        Some(unsafe { self.extend_lifetime() })
    }
}

impl<'a, C: Compartment> IntoPropertyKey<C> for &'a str {
    fn into_key<S>(self, cx: &mut Context<S>) -> Option<PropertyKey<C>>
    where
        S: InCompartment<C> + CanAlloc,
    {
        letroot!(string = JsString::new(self, cx); [cx]);
        string.to_property(cx)
    }
}

bitflags! {
    /// Represents the flags of properties on an `[JsObject]`
    #[derive(Clone, Copy, Debug)]
    pub struct PropertyFlags: u16 {
        /// Allows enumeration through `Object.keys()`, `for...in` and other functions.
        /// See [Enumerability of Properties](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Enumerability_and_ownership_of_properties#traversing_object_properties).
        const ENUMERATE = JSPROP_ENUMERATE as u16;

        /// Prevents reassignment of the property.
        const READ_ONLY = JSPROP_READONLY as u16;

        /// Prevents deletion and attribute modification of the property.
        const PERMANENT = JSPROP_PERMANENT as u16;
        const RESOLVING = JSPROP_RESOLVING as u16;

        const CONSTANT = PropertyFlags::READ_ONLY.bits() | PropertyFlags::PERMANENT.bits();
        const CONSTANT_ENUMERATED = PropertyFlags::CONSTANT.bits() | PropertyFlags::ENUMERATE.bits();
    }
}

bitflags! {
    /// Represents the flags when iterating over an [Object](crate::Object).
    #[derive(Clone, Copy, Debug, Default)]
    pub struct PropertyIteratorFlags: u32 {
        /// Allows iterating over private properties.
        const PRIVATE = JSITER_PRIVATE;
        /// Disallows iterating over inherited properties.
        const OWN_ONLY = JSITER_OWNONLY;
        /// Allows iteration over non-enumerable properties.
        const HIDDEN = JSITER_HIDDEN;
        /// Allows iteration over symbol keys.
        const SYMBOLS = JSITER_SYMBOLS;
        /// Disallows iteration over string keys.
        const SYMBOLS_ONLY = JSITER_SYMBOLSONLY;
        /// Iteration over async iterable objects and async generators.
        const FOR_AWAIT_OF = JSITER_FORAWAITOF;
    }
}

#[cfg(test)]
mod test {

    use crate::{
        letroot,
        object::property::IntoPropertyKey,
        setup_cx,
        value::{TryFromJs, TryIntoJs},
    };

    #[test]
    fn string_to_property() {
        setup_cx!(cx);

        letroot!(prop = "key1".into_key(&mut cx).unwrap(); [cx]);
        letroot!(value = prop.try_into_js(&mut cx) .unwrap(); [cx]);
        let s = String::try_from_js(&value, (), &mut cx).unwrap();
        assert_eq!("key1", s);
    }
}
