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

pub trait Compartment: Copy + Debug + Eq + Hash {}

/// A wildcard compartment with an erased lifetime
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Any;
impl Compartment for Any {}

/// A compartment that is alive for a given lifetime
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ref<'a>(PhantomData<&'a mut ()>);
impl<'a> Compartment for Ref<'a> {}
