//! A garbage-collected heap pointer used to refer to on-heap objects.
//! All garbage-collected pointers should be wrapped in a `GcPtr`
//! for safety purposes.  

use std::{cell::UnsafeCell, marker::PhantomPinned, mem, pin::Pin, ptr, sync::Arc};

use mozjs::{
    jsapi::{
        jsid, HeapBigIntWriteBarriers, HeapObjectWriteBarriers, HeapScriptWriteBarriers,
        HeapStringWriteBarriers, HeapValueWriteBarriers, JSFunction, JSObject, JSScript,
        JSString,
        JS::{BigInt as JSBigInt, Symbol as JSSymbol},
    },
    jsid::VoidId,
    jsval::{JSVal, UndefinedValue},
};

pub use mozjs::jsapi::{Handle, MutableHandle as HandleMut};

pub trait AsRawPtr {
    type Ptr;

    /// Get the raw pointer to the underlying object.
    unsafe fn as_raw_ptr(&self) -> Self::Ptr;
}

pub trait AsRawHandle: AsRawPtr {
    /// Retrieves a SpiderMonkey Rooted Handle to the underlying value.
    ///
    /// # Safety
    ///
    /// This is only safe to do on a rooted object (which [`GcPtr`] is not,
    /// it needs to be additionally rooted).
    unsafe fn as_raw_handle(&self) -> Handle<Self::Ptr>;
}

pub trait AsRawHandleMut: AsRawHandle {
    /// Retrieves a SpiderMonkey Rooted Handle to the underlying value.
    ///
    /// # Safety
    ///
    /// This is only safe to do on a rooted object (which [`GcPtr`] is not,
    /// it needs to be additionally rooted).
    unsafe fn as_raw_handle_mut(&self) -> HandleMut<Self::Ptr>;
}

/// A GC barrier is a mechanism used to ensure that the garbage collector maintains
/// a valid set of reachable objects.
///
/// A write barrier is a mechanism used to ensure that the garbage collector is notified
/// when a reference to an object is changed. In general, a write barrier should be invoked
/// whenever a write can cause the set of things traced by the GC to change.
///
/// Every barriered write should have the following form:
/// ```notrust
///   field = new_value;
///   <write-barrier>
/// ```
///
/// # Safety
///
/// - An incorrect implementation of the trait can result in reachability snapshot when
///   performing incremental garbage collection. This can result in segfauts / use-after-frees
///   if not correctly handled.
///
pub unsafe trait WriteBarrieredPtr: Copy {
    /// Creates a uninitialized value
    unsafe fn uninit() -> Self;

    /// Perform a write barrier on the given GC value
    unsafe fn write_barrier(v: *mut Self, prev: Self, next: Self);
}

/// A garbage-collected pointer used to refer to on-heap objects
///
/// # Safety
///
/// [`GcPtr<T>`] should only be used by values on the heap. Garbage collected pointers
/// on the stack should be rooted.
#[derive(Debug)]
pub struct GcPtr<T: WriteBarrieredPtr> {
    // # Safety
    //
    // For garbage collection to work correctly, when modifying
    // the wrapped value that points to a GC cell, the write barrier
    // must be invoked.
    //
    // This means after calling the `set` method, the `GcPtr` *must not*
    // be moved in memory. Doing so would invalidate the local reference.
    // For safety, we use `Arc::pin` to pin the `GcPtr`.
    inner_ptr: UnsafeCell<T>,
    _marker: PhantomPinned,
}

impl<T: WriteBarrieredPtr> GcPtr<T> {
    /// Creates an uninitialized [`GcPtr`]
    pub fn uninit() -> Self {
        Self {
            inner_ptr: UnsafeCell::new(unsafe { T::uninit() }),
            _marker: PhantomPinned,
        }
    }

    /// Creates a new [`GcPtr`] from an existing pointer.
    ///
    /// # Safety
    ///
    /// The raw pointer `ptr` must point to an object that extends a `js::gc::Cell`.
    pub fn pinned(ptr: T) -> Pin<Arc<Self>> {
        let pinned = Arc::pin(Self::uninit());
        pinned.as_ref().set(ptr);

        pinned
    }

    /// Compares two pointers for equality
    #[allow(dead_code)]
    fn ptr_eq(&self, other: &Self) -> bool {
        self.inner_ptr.get() == other.inner_ptr.get()
    }

    /// Returns the raw pointer
    ///
    /// # Safety
    ///
    /// The caller must guarantee that the pointer is valid for reads and
    /// points to a valid `js::gc::Cell`.
    pub unsafe fn get(&self) -> T {
        // Note: read_unaligned is used since SpiderMonkey doesn't
        // guarantee the expected alignment of Rust pointers.
        self.inner_ptr.get().read_unaligned()
    }

    /// Returns the raw pointer to the internal cell of [`GcPtr`].
    ///
    /// # Notes
    ///
    /// While the operation itself is not unsafe, the caller must guarantee
    /// that any writes to the pointer are barriered and that the [`GcPtr`]
    /// isn't moved for the lifetime of the pointer.
    pub fn get_unsafe(&self) -> *mut T {
        self.inner_ptr.get()
    }

    /// Sets the pointer to a new value
    pub fn set(self: Pin<&Self>, next: T) {
        let self_ptr = self.inner_ptr.get();
        unsafe {
            let prev = *self_ptr;

            *self_ptr = next;
            T::write_barrier(self_ptr, prev, next)
        }
    }
}

impl<T: WriteBarrieredPtr> AsRawPtr for GcPtr<T> {
    type Ptr = T;

    unsafe fn as_raw_ptr(&self) -> Self::Ptr {
        self.get()
    }
}

impl<T: WriteBarrieredPtr> AsRawHandle for GcPtr<T> {
    unsafe fn as_raw_handle(&self) -> Handle<Self::Ptr> {
        Handle::from_marked_location(self.inner_ptr.get() as *const _)
    }
}

impl<T: WriteBarrieredPtr> AsRawHandleMut for GcPtr<T> {
    unsafe fn as_raw_handle_mut(&self) -> HandleMut<Self::Ptr> {
        HandleMut::from_marked_location(self.inner_ptr.get())
    }
}

impl<T: WriteBarrieredPtr> Drop for GcPtr<T> {
    fn drop(&mut self) {
        unsafe {
            let inner_ptr = self.inner_ptr.get();
            T::write_barrier(inner_ptr, *inner_ptr, T::uninit())
        }
    }
}

unsafe impl WriteBarrieredPtr for *mut JSObject {
    unsafe fn uninit() -> Self {
        ptr::null_mut()
    }

    unsafe fn write_barrier(v: *mut Self, prev: Self, next: Self) {
        HeapObjectWriteBarriers(v, prev, next)
    }
}

unsafe impl WriteBarrieredPtr for *mut JSString {
    unsafe fn uninit() -> Self {
        ptr::null_mut()
    }

    unsafe fn write_barrier(v: *mut Self, prev: Self, next: Self) {
        HeapStringWriteBarriers(v, prev, next)
    }
}

unsafe impl WriteBarrieredPtr for *mut JSFunction {
    unsafe fn uninit() -> Self {
        ptr::null_mut()
    }

    unsafe fn write_barrier(v: *mut Self, prev: Self, next: Self) {
        HeapObjectWriteBarriers(
            // SAFETY: JSFunction extends JSObject
            mem::transmute::<*mut *mut JSFunction, *mut *mut JSObject>(v),
            mem::transmute::<*mut JSFunction, *mut JSObject>(prev),
            mem::transmute::<*mut JSFunction, *mut JSObject>(next),
        )
    }
}

unsafe impl WriteBarrieredPtr for *mut JSSymbol {
    unsafe fn uninit() -> Self {
        ptr::null_mut()
    }

    unsafe fn write_barrier(_v: *mut Self, _prev: Self, _next: Self) {
        // No write barrier needed for JSSymbol
    }
}

unsafe impl WriteBarrieredPtr for *mut JSBigInt {
    unsafe fn uninit() -> Self {
        ptr::null_mut()
    }

    unsafe fn write_barrier(v: *mut Self, prev: Self, next: Self) {
        HeapBigIntWriteBarriers(v, prev, next)
    }
}

unsafe impl WriteBarrieredPtr for *mut JSScript {
    unsafe fn uninit() -> Self {
        ptr::null_mut()
    }

    unsafe fn write_barrier(v: *mut Self, prev: Self, next: Self) {
        HeapScriptWriteBarriers(v, prev, next)
    }
}

unsafe impl WriteBarrieredPtr for jsid {
    unsafe fn uninit() -> Self {
        VoidId()
    }

    unsafe fn write_barrier(_v: *mut Self, _prev: Self, _next: Self) {
        // No write barrier needed for jsid
    }
}

unsafe impl WriteBarrieredPtr for JSVal {
    unsafe fn uninit() -> Self {
        UndefinedValue()
    }

    unsafe fn write_barrier(v: *mut Self, prev: Self, next: Self) {
        HeapValueWriteBarriers(v, &prev, &next)
    }
}

#[cfg(test)]
mod test {
    use std::sync::Mutex;

    use crate::gc::ptr::{GcPtr, WriteBarrieredPtr};

    #[derive(Debug, PartialEq, Eq, Copy, Clone)]
    pub struct TestPtr {
        value: i32,
    }

    const TEST_PTR_UNINIT: TestPtr = TestPtr { value: 0 };

    static WRITE_BARRIER_LOG: Mutex<Vec<(TestPtr, TestPtr)>> = Mutex::new(Vec::new());

    unsafe impl WriteBarrieredPtr for TestPtr {
        unsafe fn uninit() -> Self {
            TEST_PTR_UNINIT
        }

        unsafe fn write_barrier(_v: *mut Self, _prev: Self, _next: Self) {
            // No write barrier needed for TestPtr

            WRITE_BARRIER_LOG.lock().unwrap().push((_prev, _next));
        }
    }

    #[test]
    fn test_new_triggers_barrier() {
        WRITE_BARRIER_LOG.lock().unwrap().clear();

        let _ptr = GcPtr::pinned(TestPtr { value: 42 });

        let write_barrier_log = WRITE_BARRIER_LOG.lock().unwrap();
        assert_eq!(write_barrier_log.len(), 1);
        assert_eq!(
            write_barrier_log[0],
            (TEST_PTR_UNINIT, TestPtr { value: 42 })
        );
    }

    #[test]
    fn test_set_calls_write_barrier() {
        WRITE_BARRIER_LOG.lock().unwrap().clear();

        let ptr = GcPtr::pinned(TestPtr { value: 42 });
        let new_ptr = TestPtr { value: 43 };

        ptr.as_ref().set(new_ptr);

        let write_barrier_log = WRITE_BARRIER_LOG.lock().unwrap();
        assert_eq!(write_barrier_log.len(), 2);
        assert_eq!(
            write_barrier_log[0],
            (TEST_PTR_UNINIT, TestPtr { value: 42 })
        );
        assert_eq!(
            write_barrier_log[1],
            (TestPtr { value: 42 }, TestPtr { value: 43 })
        );
    }
}
