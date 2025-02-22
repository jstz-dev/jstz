//! # Garbage Collection in SpiderMonkey
//!
//! This module implements memory-safe abstractions on SpiderMonkey's garbage collection (GC) system,
//! designed for efficient memory management in the JavaScript engine. The GC is a mark-sweep collector
//! with features such as incremental marking, generational collection, and compaction.
//!
//! # Key Concepts
//! - **Cells**: The atomic unit of memory managed by the GC. All GC-allocated objects, such as `JsObject`, derive from `Cell`.
//! - **Compartments & Zones**: Memory is organized into compartments (for security and isolation) and zones (GC boundaries).
//!
//! # Features
//! - **Incremental GC**: Reduces pause times by interleaving marking work with JavaScript execution.
//! - **Write and Read Barriers**: Ensure correctness during incremental GC by maintaining object reachability.
//! - **Generational GC**: Optimizes for short-lived objects, separating them from long-lived ones.
//!
//! # Implementation Notes
//! - Write barriers, triggered during pointer updates, and read barriers for weak references, prevent GC hazards.
//! - Sweeping and additional GC phases like compaction are integrated into the collection process.
//!
//! For further details, see the [GC Implementation Guide](https://udn.realityripple.com/docs/Mozilla/Projects/SpiderMonkey/Internals/Garbage_collection).

pub mod compartment;
pub mod ptr;
pub mod root;
pub mod trace;

pub use compartment::Compartment;
pub use root::Prolong;
pub use trace::{Finalize, Trace, Tracer};

#[macro_export]
macro_rules! gcptr_wrapper {
    ($doc:expr, $name: ident, $ptr_type: ty) => {
        #[doc = $doc]
        pub struct $name<'a, C: $crate::gc::Compartment> {
            inner_ptr: std::pin::Pin<std::sync::Arc<$crate::gc::ptr::GcPtr<$ptr_type>>>,
            marker: std::marker::PhantomData<(&'a (), C)>,
        }

        impl<'a, C: $crate::gc::Compartment> Clone for $name<'a, C> {
            fn clone(&self) -> Self {
                Self {
                    inner_ptr: self.inner_ptr.clone(),
                    marker: self.marker,
                }
            }
        }

        impl<'a, C: $crate::gc::Compartment> $crate::gc::ptr::AsRawPtr for $name<'a, C> {
            type Ptr = $ptr_type;

            unsafe fn as_raw_ptr(&self) -> Self::Ptr {
                self.inner_ptr.as_raw_ptr()
            }
        }

        impl<'a, C: $crate::gc::Compartment> $crate::gc::ptr::AsRawHandle
            for $name<'a, C>
        {
            unsafe fn as_raw_handle(&self) -> $crate::gc::ptr::Handle<Self::Ptr> {
                self.inner_ptr.as_raw_handle()
            }
        }

        impl<'a, C: $crate::gc::Compartment> $crate::gc::ptr::AsRawHandleMut
            for $name<'a, C>
        {
            unsafe fn as_raw_handle_mut(&self) -> $crate::gc::ptr::HandleMut<Self::Ptr> {
                self.inner_ptr.as_raw_handle_mut()
            }
        }

        impl<'a, C: $crate::gc::Compartment> $name<'a, C> {
            /// Converts a raw pointer to the wrapped type.
            ///
            /// # Safety
            ///
            /// If the pointer does not correctly point to a valid GC object (of the expected type)
            /// then `TypeError`s, segmentation faults, or Undefined Behaviour in general could
            /// occur.
            #[allow(dead_code)]
            pub(crate) unsafe fn from_raw(ptr: $ptr_type) -> Self {
                Self {
                    inner_ptr: $crate::gc::ptr::GcPtr::pinned(ptr),
                    marker: std::marker::PhantomData,
                }
            }

            /// Retrieves a SpiderMonkey Rooted Handle to the underlying value.
            ///
            /// # Safety
            ///
            /// This function relies on Rust's borrow checker to ensure that only rooted values
            /// are used, preventing unrooted accesses at the type level. However, **it is the
            /// caller's responsibility** to guarantee that:
            ///
            /// - The lifetimes of both the context and the value are valid.
            /// - The `Context` passed to this function is the **same** context in which the value
            ///   was originally created and rooted.
            ///
            /// Failure to uphold these invariants—whether by bypassing the borrow checker
            /// through `unsafe` code or by using multiple contexts incorrectly—constitutes a
            /// **programming error** and results in **undefined behavior**.
            #[allow(unused)]
            pub(crate) fn handle<'cx, S>(
                &self,
                _cx: &'cx mut Context<S>,
            ) -> $crate::gc::ptr::Handle<<Self as $crate::gc::ptr::AsRawPtr>::Ptr>
            where
                S: $crate::context::InCompartment<C> + $crate::context::CanAccess,
                'cx: 'a,
            {
                // SAFETY: self is guaranteed to be rooted due to the lifetimes, therefore it safe to acquire a handle
                unsafe { $crate::gc::ptr::AsRawHandle::as_raw_handle(self) }
            }

            /// Retrieves a SpiderMonkey Rooted HandleMut to the underlying value.
            ///
            /// # Safety
            ///
            /// This function relies on Rust's borrow checker to ensure that only rooted values
            /// are used, preventing unrooted accesses at the type level. However, **it is the
            /// caller's responsibility** to guarantee that:
            ///
            /// - The lifetimes of both the context and the value are valid.
            /// - The `Context` passed to this function is the **same** context in which the value
            ///   was originally created and rooted.
            ///
            /// Failure to uphold these invariants—whether by bypassing the borrow checker
            /// through `unsafe` code or by using multiple contexts incorrectly—constitutes a
            /// **programming error** and results in **undefined behavior**.
            #[allow(unused)]
            pub(crate) fn handle_mut<'cx, S>(
                &self,
                _cx: &'cx mut Context<S>,
            ) -> $crate::gc::ptr::HandleMut<<Self as $crate::gc::ptr::AsRawPtr>::Ptr>
            where
                S: $crate::context::InCompartment<C> + $crate::context::CanAccess,
                'cx: 'a,
            {
                // SAFETY: self is guaranteed to be rooted due to lifetimes, therefore it safe to acquire a handle
                unsafe { $crate::gc::ptr::AsRawHandleMut::as_raw_handle_mut(self) }
            }
        }

        unsafe impl<'a, 'b, C: $crate::gc::Compartment> $crate::gc::Prolong<'a>
            for $name<'b, C>
        {
            type Aged = $name<'a, C>;
        }

        impl<'a, C: $crate::gc::Compartment> $crate::gc::Finalize for $name<'a, C> {
            fn finalize(&self) {
                self.inner_ptr.finalize()
            }
        }

        unsafe impl<'a, C: $crate::gc::Compartment> $crate::gc::Trace for $name<'a, C> {
            $crate::custom_trace!(this, mark, {
                mark(&this.inner_ptr);
            });
        }
    };
}
