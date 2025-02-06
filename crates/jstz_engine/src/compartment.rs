//! SpiderMonkey partitions the heap into compartments. Some key properties of
//! compartments are:
//!  - Every GC object (cell) belongs to one compartment
//!  - A GC object cannot hold a pointer to an object in another compartment.
//!    This is fundamental invariant for security checks and garbage collection.
//!  - Garbage collection is done a per-compartment basis -- this is an optimisation
//!    to avoid marking the entire heap.
//!
//! INVARIANT: no references between compartments.
//!
//! This module defines the `Compartment` trait, which is implemented by types that represent compartments
//! at Rust's type level. This allows the type system to enforce the compartment invariant.

use std::{fmt::Debug, hash::Hash, marker::PhantomData};

/// A compartment id that is alive for a given lifetime
///
/// The lifetime of the compartment id is guaranteed to be unique among all
/// other compartments. This will ensure that when creating a compartment,
/// a different compartment will produce a new lifetime that cannot be coerced
/// with an existing compartment.
///
/// `Id` is [invariant](https://doc.rust-lang.org/nomicon/subtyping.html#variance) over the
/// lifetime parameter.
///
/// Any `Id` lifetime can be trusted, so long as they are known to be allocated using
/// [`alloc_compartment!`] and haven't been derived from some unsafe code.  
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id<'a>(
    /// To achieve lifetime invariance, we use `fn(&'a ()) -> &'a ()`. This works
    /// because `fn(T) -> T` is contravariant over `T` and covariant over `T`, which
    /// combines to invariance over `T`.
    PhantomData<fn(&'a ()) -> &'a ()>,
);

impl<'a> Id<'a> {
    /// Construct a new compartment with an unbounded lifetime.
    ///
    /// You should not need to use this function; use [`alloc_compartment!`] instead.
    ///
    /// # Safety
    ///
    /// `Id` holds an invariant lifetime that must be generative. Using this function directly
    /// cannot guarentee that `'a` is generative.
    pub unsafe fn new() -> Self {
        Self(PhantomData)
    }
}

/// DO NOT USE. Used by [`alloc_compartment!`].
pub struct Region<'a>(PhantomData<&'a Id<'a>>);

impl<'a> Drop for Region<'a> {
    #[inline(always)]
    fn drop(&mut self) {
        // Purposefully blank -- this ensures `Region` has drop glue, ensuring this is dropped at
        // the end of the scope. Without it it, the compiler will optimize region away as it contains
        // only PhamtomData, consequently ignoring the lifetime check. Importantly, this ensures the
        // compiler considers `'a` live at the point this region is dropped.
    }
}

impl<'a> Region<'a> {
    /// # Safety
    ///
    /// DO NOT USE. Used by [`alloc_compartment!`].
    pub unsafe fn new(_: &'a Id<'a>) -> Self {
        Self(PhantomData)
    }
}

pub trait Compartment: Debug + Eq + Hash {}

/// A wildcard compartment with an erased lifetime
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Any;
impl Compartment for Any {}

/// A compartment that is alive for a given lifetime
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Ref<'a>(Id<'a>);
impl<'a> Compartment for Ref<'a> {}

impl<'a> Ref<'a> {
    /// # Safety
    ///
    /// DO NOT USE. Used by [`alloc_compartment!`].
    pub unsafe fn new(id: Id<'a>) -> Self {
        Self(id)
    }
}

#[macro_export]
macro_rules! alloc_compartment {
    ($name:ident) => {
        let compartment_id = unsafe { $crate::compartment::Id::new() };
        #[allow(unused)]
        let compartment_region =
            unsafe { $crate::compartment::Region::new(&compartment_id) };
        let $name = unsafe { $crate::compartment::Ref::new(compartment_id) };
    };
}

#[cfg(test)]
mod test {

    #[test]
    fn allow_unify_with_self() {
        alloc_compartment!(a);
        alloc_compartment!(b);
        assert_eq!(a, a);
        assert_eq!(b, b);
    }

    #[test]
    fn ui() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/ui/compartments_are_generative.rs");
    }
}
