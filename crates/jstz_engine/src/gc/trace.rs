use std::{
    any::TypeId,
    marker::PhantomData,
    num::{
        NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize,
        NonZeroU128, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize,
    },
    ops::Deref,
    path::{Path, PathBuf},
    pin::Pin,
    rc::Rc,
    sync::Arc,
};

use mozjs::jsapi::{
    gc::{
        TraceExternalEdge, TraceExternalEdge1, TraceExternalEdge3, TraceExternalEdge5,
        TraceExternalEdge6, TraceExternalEdge7, TraceExternalEdge8, TraceExternalEdge9,
    },
    jsid as JSId, BigInt as JSBigInt, GCTraceKindToAscii, JSFunction, JSObject, JSScript,
    JSString, Symbol as JSSymbol, TraceKind, Value as JSValue,
};

/// Visitor passed to trace methods. ALL managed pointers
/// must be traced by this visitor.
pub use mozjs::jsapi::JSTracer as Tracer;

use super::ptr::GcPtr;

/// The Trace trait, which needs to be implemented on garbage-collected objects.
///
/// # Safety
///
/// - An incorrect implementation of the trait can result in heap overflows, data corruption,
///   use-after-free, or Undefined Behaviour in general.
///
/// - Calling any of the functions marked as `unsafe` outside of the context of the garbage collector
///   can result in Undefined Behaviour.
pub unsafe trait Trace: Finalize {
    /// Marks all contained traceable objects.
    unsafe fn trace(&self, trc: *mut Tracer);

    /// Runs [`Finalize::finalize`] on the object and its children.
    fn run_finalizer(&self);
}

/// Substitute for the [`Drop`] trait for garbage collected types.
pub trait Finalize {
    /// Cleanup logic for a type.
    fn finalize(&self) {}
}

/// Utility macro to define an empty implementation of [`Trace`].
///
/// Use this for marking types as not containing any `Trace` types.
#[macro_export]
macro_rules! empty_trace {
    () => {
        unsafe fn trace(&self, _trc: *mut $crate::gc::Tracer) {}

        fn run_finalizer(&self) {
            $crate::gc::Finalize::finalize(self);
        }
    };
}

/// Utility macro to manually implement [`Trace`] on a type.
///
/// You define a `this` parameter name and pass in a body, which should call `mark` on every
/// traceable element inside the body. The mark implementation will automatically delegate to the
/// correct method on the argument.
///
/// # Safety
///
/// Misusing the `mark` function may result in Undefined Behaviour.
#[macro_export]
macro_rules! custom_trace {
    ($this:ident, $marker:ident, $body:expr) => {
        unsafe fn trace(&self, trc: *mut $crate::gc::Tracer) {
            let $marker = |it: &dyn $crate::gc::Trace| {
                // SAFETY: The implementor must ensure that `trace` is correctly implemented.
                unsafe {
                    $crate::gc::Trace::trace(it, trc);
                }
            };
            let $this = self;
            $body
        }

        fn run_finalizer(&self) {
            let $marker = |it: &dyn $crate::gc::Finalize| {
                $crate::gc::Finalize::finalize(it);
            };
            let $this = self;
            $body
        }
    };
}

impl<T: ?Sized> Finalize for &'static T {}
// SAFETY: 'static references don't need to be traced, since they live indefinitely.
unsafe impl<T: ?Sized> Trace for &'static T {
    empty_trace!();
}

macro_rules! impl_empty_finalize_trace {
    ($($T:ty),*) => {
        $(
            impl Finalize for $T {}

            // SAFETY:
            // Primitive types and string types don't have inner nodes that need to be marked.
            unsafe impl Trace for $T { empty_trace!(); }
        )*
    }
}

impl_empty_finalize_trace!(
    (),
    bool,
    isize,
    usize,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    i64,
    u64,
    i128,
    u128,
    f32,
    f64,
    char,
    TypeId,
    String,
    Box<str>,
    Rc<str>,
    Path,
    PathBuf,
    NonZeroIsize,
    NonZeroUsize,
    NonZeroI8,
    NonZeroU8,
    NonZeroI16,
    NonZeroU16,
    NonZeroI32,
    NonZeroU32,
    NonZeroI64,
    NonZeroU64,
    NonZeroI128,
    NonZeroU128
);

impl<T: Trace, const N: usize> Finalize for [T; N] {}

// SAFETY:
// All elements inside the array are correctly marked.
unsafe impl<T: Trace, const N: usize> Trace for [T; N] {
    custom_trace!(this, mark, {
        for v in this {
            mark(v);
        }
    });
}

impl<T: Trace> Finalize for Rc<T> {}

// SAFETY: The inner value of the `Rc` is correctly marked.
unsafe impl<T: Trace> Trace for Rc<T> {
    #[inline]
    unsafe fn trace(&self, trc: *mut Tracer) {
        // SAFETY: The implementor must ensure that `trace` is correctly implemented.
        Trace::trace(&**self, trc);
    }

    #[inline]
    fn run_finalizer(&self) {
        Finalize::finalize(self);
        Trace::run_finalizer(&**self);
    }
}

impl<T: Trace> Finalize for Arc<T> {}

// SAFETY: The inner value of the `Arc` is correctly marked.
unsafe impl<T: Trace> Trace for Arc<T> {
    #[inline]
    unsafe fn trace(&self, trc: *mut Tracer) {
        // SAFETY: The implementor must ensure that `trace` is correctly implemented.
        Trace::trace(&**self, trc);
    }

    #[inline]
    fn run_finalizer(&self) {
        Finalize::finalize(self);
        Trace::run_finalizer(&**self);
    }
}

// SAFETY: The dereferenced value of the `Pin` is correctly marked.
impl<P: Deref<Target = T>, T: Trace> Finalize for Pin<P> {}

unsafe impl<P: Deref<Target = T>, T: Trace> Trace for Pin<P> {
    #[inline]
    unsafe fn trace(&self, trc: *mut Tracer) {
        // SAFETY: The implementor must ensure that `trace` is correctly implemented.
        Trace::trace(self.as_ref().get_ref(), trc);
    }

    #[inline]
    fn run_finalizer(&self) {
        Finalize::finalize(self);
        Trace::run_finalizer(self.as_ref().get_ref());
    }
}

impl<T: Trace + ?Sized> Finalize for Box<T> {}

// SAFETY: The inner value of the `Box` is correctly marked.
unsafe impl<T: Trace + ?Sized> Trace for Box<T> {
    #[inline]
    unsafe fn trace(&self, trc: *mut Tracer) {
        // SAFETY: The implementor must ensure that `trace` is correctly implemented.
        Trace::trace(&**self, trc);
    }

    #[inline]
    fn run_finalizer(&self) {
        Finalize::finalize(self);
        Trace::run_finalizer(&**self);
    }
}

impl<T: Trace> Finalize for Vec<T> {}

// SAFETY: All the inner elements of the `Vec` are correctly marked.
unsafe impl<T: Trace> Trace for Vec<T> {
    custom_trace!(this, mark, {
        for e in this {
            mark(e);
        }
    });
}

impl<T: Trace> Finalize for Option<T> {}

// SAFETY: The inner value of the `Option` is correctly marked.
unsafe impl<T: Trace> Trace for Option<T> {
    custom_trace!(this, mark, {
        if let Some(ref v) = *this {
            mark(v);
        }
    });
}

impl<T: Trace, E: Trace> Finalize for Result<T, E> {}

// SAFETY: Both inner values of the `Result` are correctly marked.
unsafe impl<T: Trace, E: Trace> Trace for Result<T, E> {
    custom_trace!(this, mark, {
        match *this {
            Ok(ref v) => mark(v),
            Err(ref v) => mark(v),
        }
    });
}

impl<T> Finalize for PhantomData<T> {}

// SAFETY: A `PhantomData` doesn't have inner data that needs to be marked.
unsafe impl<T> Trace for PhantomData<T> {
    empty_trace!();
}

macro_rules! fn_finalize_trace_one {
    ($ty:ty $(,$args:ident)*) => {
        impl<Ret $(,$args)*> Finalize for $ty {}
        // SAFETY:
        // Function pointers don't have inner nodes that need to be marked.
        unsafe impl<Ret $(,$args)*> Trace for $ty { empty_trace!(); }
    }
}
macro_rules! fn_finalize_trace_group {
    () => {
        fn_finalize_trace_one!(extern "Rust" fn () -> Ret);
        fn_finalize_trace_one!(extern "C" fn () -> Ret);
        fn_finalize_trace_one!(unsafe extern "Rust" fn () -> Ret);
        fn_finalize_trace_one!(unsafe extern "C" fn () -> Ret);
    };
    ($($args:ident),*) => {
        fn_finalize_trace_one!(extern "Rust" fn ($($args),*) -> Ret, $($args),*);
        fn_finalize_trace_one!(extern "C" fn ($($args),*) -> Ret, $($args),*);
        fn_finalize_trace_one!(extern "C" fn ($($args),*, ...) -> Ret, $($args),*);
        fn_finalize_trace_one!(unsafe extern "Rust" fn ($($args),*) -> Ret, $($args),*);
        fn_finalize_trace_one!(unsafe extern "C" fn ($($args),*) -> Ret, $($args),*);
        fn_finalize_trace_one!(unsafe extern "C" fn ($($args),*, ...) -> Ret, $($args),*);
    }
}

macro_rules! tuple_finalize_trace {
    () => {}; // This case is handled above, by simple_finalize_empty_trace!().
    ($($args:ident),*) => {
        impl<$($args),*> Finalize for ($($args,)*) {}
        // SAFETY:
        // All elements inside the tuple are correctly marked.
        unsafe impl<$($args: Trace),*> Trace for ($($args,)*) {
            custom_trace!(this, mark, {
                #[allow(non_snake_case, unused_unsafe, unused_mut)]
                let mut avoid_lints = |&($(ref $args,)*): &($($args,)*)| {
                    // SAFETY: The implementor must ensure a correct implementation.
                    unsafe { $(mark($args);)* }
                };
                avoid_lints(this)
            });
        }
    }
}

macro_rules! type_arg_tuple_based_finalize_trace_impls {
    ($(($($args:ident),*),)*) => {
        $(
            fn_finalize_trace_group!($($args),*);
            tuple_finalize_trace!($($args),*);
        )*
    }
}

type_arg_tuple_based_finalize_trace_impls!(
    (),
    (A),
    (A, B),
    (A, B, C),
    (A, B, C, D),
    (A, B, C, D, E),
    (A, B, C, D, E, F),
    (A, B, C, D, E, F, G),
    (A, B, C, D, E, F, G, H),
    (A, B, C, D, E, F, G, H, I),
    (A, B, C, D, E, F, G, H, I, J),
    (A, B, C, D, E, F, G, H, I, J, K),
    (A, B, C, D, E, F, G, H, I, J, K, L),
);

macro_rules! impl_gcptr_finalize_trace {

    ($((*mut $T:ty, $tracer:ident, $kind:expr)),*) => {
        $(
            impl Finalize for GcPtr<*mut $T> {}

            // SAFETY: The function is correctly traced using SM's API.
            unsafe impl Trace for GcPtr<*mut $T> {
                unsafe fn trace(&self, trc: *mut Tracer) {
                    if self.get().is_null() {
                        return;
                    }
                    $tracer(trc, self.get_unsafe(), GCTraceKindToAscii($kind))
                }

                fn run_finalizer(&self) {
                    Finalize::finalize(self);
                }
            }
        )*
    };

    ($(($T:ty, $tracer:ident, $kind:expr)),*) => {
       $(
            impl Finalize for GcPtr<$T> {}

            // SAFETY: The function is correctly traced using SM's API.
            unsafe impl Trace for GcPtr<$T> {
                unsafe fn trace(&self, trc: *mut Tracer) {
                    $tracer(trc, self.get_unsafe(), GCTraceKindToAscii($kind))
                }

                fn run_finalizer(&self) {
                    Finalize::finalize(self);
                }
            }
       )*
    };
}

impl_gcptr_finalize_trace!(
    (*mut JSBigInt, TraceExternalEdge, TraceKind::BigInt),
    (*mut JSSymbol, TraceExternalEdge1, TraceKind::Symbol),
    (*mut JSFunction, TraceExternalEdge3, TraceKind::Object),
    (*mut JSObject, TraceExternalEdge5, TraceKind::Object),
    (*mut JSScript, TraceExternalEdge6, TraceKind::Script),
    (*mut JSString, TraceExternalEdge7, TraceKind::String),
    (JSId, TraceExternalEdge9, TraceKind::Object),
    (JSValue, TraceExternalEdge8, TraceKind::Object)
);
