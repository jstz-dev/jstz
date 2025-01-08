use bitflags::bitflags;
use indoc::indoc;
use mozjs::jsapi::{
    jsid, JSITER_FORAWAITOF, JSITER_HIDDEN, JSITER_OWNONLY, JSITER_PRIVATE,
    JSITER_SYMBOLS, JSITER_SYMBOLSONLY, JSPROP_ENUMERATE, JSPROP_PERMANENT,
    JSPROP_READONLY, JSPROP_RESOLVING,
};

use crate::{
    context::{CanAlloc, Context, InCompartment},
    gc::Compartment,
    gcptr_wrapper,
};

gcptr_wrapper!(
    indoc! {"
        A key associated with a Property Descriptor, typically the name of a field, method or accessor. 
        This can either be a [`JsString`] or a [`JsSymbol`]. 

        More information:
         - [MDN documentation](mdn)
         - [EMCAScript reference][spec]

        [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Object/defineProperty
        [spec]: https://tc39.es/ecma262/#property-key
    "},
    PropertyKey,
    jsid
);

pub trait IntoPropertyKey {
    /// Converts `self` into a new [`PropertyKey`].
    /// Returns [`None`] when conversion fails.
    fn into_key<C: Compartment, S>(self, cx: &mut Context<S>) -> Option<PropertyKey<C>>
    where
        S: InCompartment<C> + CanAlloc;
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
