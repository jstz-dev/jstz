//! This module provides the mechanisms to manage roots in SpiderMonkey's garbage collection (GC) system.
//! Rooting ensures that specific objects are kept alive during GC cycles by marking them as reachable.
//! It is a critical component of GC, preventing unintended collection of active or important objects.
//!
//! In languages with native support for GC (such as JavaScript), rooting is supported by the compiler/interpreter,
//! which can provide metadata for each stack frame allowing it to be traced. However, as implementators of native
//! objects/functions for JavaScript, we have no such metadata. Instead rooting has to be performed explicitly.
//!
//! This explicit rooting is needed whenever an object is needed to outlive the borrow of the JS
//! context that produced it. For example, a function that compiles and then evaluates a script:
//! ```no_run rust
//! pub fn compile_and_evaluate<S>(
//!     path: &Path,
//!     src: &str,
//!     mut cx: &mut Context<S>,
//! ) -> Option<JsValue<'_, C>> where S: InCompartment<C> + CanAlloc {
//!     let script = Script::compile(path, src, &mut cx)?;
//!
//!     script.evaluate(&mut cx)
//! }
//! ```
//! This is the natural way to write such a function, however, it is in fact not safe since the script
//! is not rooted. If `evaluate` triggers a GC before the script is rooted during evaluation, then `script`
//! may be reclaimed, causing a later use-after-free error.
//!
//! Fortunately, our approach catches these safety problems as lifetime errors:
//! ```notrust
//! error[E0499]: cannot borrow `cx` as mutable more than once at a time
//!    --> crates/jstz_engine/src/script.rs:111:25
//!     |
//! 109 |         let script = Script::compile(path, src, &mut cx)?;
//!     |                                                 ------- first mutable borrow occurs here
//! 110 |
//! 111 |         script.evaluate(&mut cx)
//!     |                -------- ^^^^^^^ second mutable borrow occurs here
//!     |                |
//!     |                first borrow later used by call
//!
//! For more information about this error, try `rustc --explain E0499`.
//! ```
//! The fix is to explicit root `script`. To do so, we use the `letroot!` macro implemented in
//! this module.
//! ```no_run rust
//! pub fn compile_and_evaluate<S>(
//!     path: &Path,
//!     src: &str,
//!     mut cx: &mut Context<S>,
//! ) -> Option<JsValue<'_, C>> where S: InCompartment<C> + CanAlloc {
//!     letroot!(script = Script::compile(path, src, &mut cx)?; [cx]);
//!
//!     script.evaluate(&mut cx)
//! }
//! ```
//! The declaration of a root allocates space on the stack for a new root. Note that it is just the reference
//! that is copied to the stack, the JS cell is still stored on the heap.
//!
//! Roots have type `Rooted<'a,T>` where `'a` is the lifetime of the root, and T is the
//! type being rooted. Once the local variables are rooted, the code typechecks, because
//! rooting changes the lifetime of the value. The rule is giving as follows:
//!
//!  p: T<'a, C>  r: Pin<&'b mut Root<T<'b, C>>
//! --------------------------------------------- 'b : 'a
//!         r.init(p) : T<'b, C>  
//!
//! where `T` is some JS type that contains the lifetime `'a` and is in the compartment `C`.
//! Note that `T<'b, C>` represents *substituting* the lifetime `'b` for the lifetime `'a` recursively on the
//! structure of `T`.
//!
//! Before rooting, the JS value had lifetime `'a`, which is usually the lifetime of the borrow of
//! the `Context` that created or accessed it. After rooting, the JS value has lifetime `'b`, which
//! is the lifetime of the root itself. Since roots are considered reachable by GC, the contents of a root
//! are guaranteed not to be GC’d during its lifetime, so this rule is sound.
//!
//! Note that this use of substitution `T<'b, C>` is being used to extend the lifetime of the JS value
//! since `'b : 'a`.

use std::{
    any::TypeId,
    cell::Cell,
    ffi::c_void,
    marker::PhantomPinned,
    mem,
    num::{
        NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize,
        NonZeroU128, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize,
    },
    ops::Deref,
    path::PathBuf,
    pin::Pin,
    ptr::NonNull,
    rc::Rc,
    sync::Arc,
};

use super::{Trace, Tracer};

/// Shadow stack implementation. This is a singly linked list of on stack rooted values
#[derive(Debug)]
pub(crate) struct ShadowStack {
    head: Cell<Option<NonNull<ShadowStackEntry>>>,
}

impl ShadowStack {
    /// Creates a new shadow stack
    pub fn new() -> Self {
        Self {
            head: Cell::new(None),
        }
    }

    /// Trace all rooted values in the shadow stack.
    ///
    /// # Safety
    ///
    /// Calling this function outside the context of the garbage collector
    /// can result in undefined behaviour
    pub unsafe fn trace(&self, trc: *mut Tracer) {
        let mut head = self.head.get();
        while let Some(some_head) = head {
            let head_ref = some_head.as_ref();
            let next = head_ref.prev;
            (*head_ref.value).trace(trc);
            head = next;
        }
    }
}

static DEFAULT_VALUE: &'static (dyn Trace + Sync) = &();

/// Entry in the GC shadow stack
///
/// This type internally stores the shadow stack pointer, the previous pointer
/// and a pointer to the vtable and data that is to be rooted.
#[derive(Debug)]
pub(crate) struct ShadowStackEntry {
    /// Shadowstack itself
    stack: NonNull<ShadowStack>,
    /// Previous rooted entry
    prev: Option<NonNull<ShadowStackEntry>>,
    /// Pointer to vtable and data to `Trace` the rooted value
    value: *const dyn Trace,
    // This removes the auto-implemented `Unpin` bound since this struct has
    // some address-sensitive state
    _marker: PhantomPinned,
}

impl ShadowStackEntry {
    /// Constructs internal shadow stack value.
    pub fn new(stack: Pin<&ShadowStack>) -> Self {
        Self {
            stack: stack.get_ref().into(),
            prev: None,
            value: DEFAULT_VALUE as _,
            _marker: PhantomPinned,
        }
    }

    pub fn link(&mut self) {
        unsafe {
            // SAFETY: self.stack is pinned on construction
            self.prev = self.stack.as_ref().head.get();

            // SAFETY: self.stack is pinned on construction
            // SAFETY: self is pinned when calling link
            // SAFETY: `self as *mut _` is guarenteed to be non-null
            //         (since its a valid reference)
            self.stack
                .as_ref()
                .head
                .set(Some(NonNull::new_unchecked(self as *mut _)));
        }
    }
}

impl Drop for ShadowStackEntry {
    fn drop(&mut self) {
        // Drop current shadow stack entry and update shadow stack state.
        unsafe { self.stack.as_mut().head.set(self.prev) }
    }
}

/// A stack cell for a rooted value.
#[derive(Debug)]
pub struct Root<T: Trace> {
    /// Shadow stack entry
    stack_entry: ShadowStackEntry,
    /// Value that is rooted.
    /// [`None`] if no value has been rooted yet. See [`Root::init`].
    value: Option<T>,
}

impl<T: Trace> Root<T> {
    /// Creates a new root.
    pub(crate) fn new(stack: Pin<&ShadowStack>) -> Self {
        Self {
            stack_entry: ShadowStackEntry::new(stack),
            value: None,
        }
    }

    /// Initialises the root by rooting the given value.
    pub fn init<'a, U>(self: Pin<&'a mut Self>, value: U) -> Rooted<'a, T>
    where
        U: Prolong<'a, Aged = T>,
    {
        let inited_self = unsafe {
            // SAFETY: we do not move out of self
            Pin::map_unchecked_mut(self, |self_mut| {
                // SAFETY: we can safely prolong `value`'s lifetime to the lifetime of the root
                self_mut.value = Some(value.extend_lifetime());
                self_mut.stack_entry.link();

                // SAFETY: We know the lifetime of `stack_entry.value` will not outlive `value`, so
                // we can safely take a `dyn` pointer of it. Additionally we know this pointer will
                // remain valid until the `Pin` (aka Root) is dropped, which is bound to the lifetime
                // of `value`
                self_mut.stack_entry.value =
                    mem::transmute((&mut self_mut.value) as &mut dyn Trace);

                self_mut
            })
        };

        Rooted {
            pinned: inited_self,
        }
    }
}

// Rooted value on the stack. This is non-copyable type that is used to hold GC thing on stack.
#[derive(Debug)]
pub struct Rooted<'a, T: Trace + 'a> {
    pinned: Pin<&'a mut Root<T>>,
}

impl<'a, T: Trace> Deref for Rooted<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.pinned.value.as_ref().unwrap()
    }
}

pub(crate) unsafe extern "C" fn unsafe_ffi_trace_context_roots(
    trc: *mut Tracer,
    shadow_stack: *mut c_void,
) {
    let shadow_stack = Box::from_raw(shadow_stack as *mut ShadowStack);
    shadow_stack.trace(trc);

    // Don't free the box, this is done by `Context::drop`
    mem::forget(shadow_stack)
}

#[macro_export]
macro_rules! letroot {
    ($var_name: ident = $value: expr; [$cx: expr]) => {
        #[allow(unused_mut)]
        let mut $var_name = core::pin::pin!($cx.root());

        #[allow(unused_mut)]
        let mut $var_name = $var_name.init($value);
    };
}

/// A trait for extending or prolonging the lifetime of a value to `'a`.
///
/// # Safety
///
/// Usaged of `extend_lifetime` outside the context of the garbage collector is considered
/// undefined behaviour
pub unsafe trait Prolong<'a> {
    type Aged;

    unsafe fn extend_lifetime(self) -> Self::Aged
    where
        Self: Sized,
    {
        // SAFETY: We `transmute_copy` the value to change the lifetime without
        //         causing the destructor to run. `forget`ting the value is safe
        //         because the value is still alive.
        let result = std::mem::transmute_copy(&self);
        std::mem::forget(self);
        result
    }
}

macro_rules! impl_move_prolong {
    ($($T:ty),*) => {
        $(
            // SAFETY:
            // All of these types can be moved safely
            unsafe impl<'a> Prolong<'a> for $T {
                type Aged = $T;
            }
        )*
    }
}

impl_move_prolong!(
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

// SAFETY: The inner type of the reference is correctly prolonged.
//         The lifetime of the reference is correctly prolonged.
unsafe impl<'a, 'b, T: Prolong<'a, Aged: 'a>> Prolong<'a> for &'b T {
    type Aged = &'a T::Aged;
}

// SAFETY: The inner type of the reference is correctly prolonged.
//         The lifetime of the reference is correctly prolonged.
unsafe impl<'a, 'b, T: Prolong<'a, Aged: 'a>> Prolong<'a> for &'b mut T {
    type Aged = &'a mut T::Aged;
}

// SAFETY: The inner type of `Pin` is correctly prolonged.
unsafe impl<'a, P: Prolong<'a>> Prolong<'a> for Pin<P> {
    type Aged = Pin<P::Aged>;
}

// SAFETY: The inner type of `Rc` is correctly prolonged.
unsafe impl<'a, T: Prolong<'a>> Prolong<'a> for Rc<T> {
    type Aged = Rc<T::Aged>;
}

// SAFETY: The inner type of `Arc` is correctly prolonged.
unsafe impl<'a, T: Prolong<'a>> Prolong<'a> for Arc<T> {
    type Aged = Arc<T::Aged>;
}

// SAFETY: The inner type of the `Box` is correctly prolonged.
unsafe impl<'a, T: Prolong<'a>> Prolong<'a> for Box<T> {
    type Aged = Box<T::Aged>;
}

// SAFETY: The inner type of `Vec` is correctly prolonged.
unsafe impl<'a, T: Prolong<'a>> Prolong<'a> for Vec<T> {
    type Aged = Vec<T::Aged>;
}

// SAFETY: The inner type of `Option` is correctly prolonged.
unsafe impl<'a, T: Prolong<'a>> Prolong<'a> for Option<T> {
    type Aged = Option<T::Aged>;
}
