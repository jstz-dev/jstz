use parking_lot::{ArcMutexGuard, Mutex, RawMutex};
use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    rc::{Rc, Weak},
    sync::Arc,
};

#[derive(Debug, Default)]
pub struct Inner {
    pub counter: i32,
}

impl Inner {
    pub fn inc(&mut self) {
        self.counter += 1;
    }
}

type Guard = ArcMutexGuard<RawMutex, Inner>;
type GuardRc = Rc<Guard>;

#[derive(Debug)]
pub struct Transaction {
    inner: Arc<Mutex<Inner>>,
    guard: RefCell<Weak<Guard>>,
}

impl Default for Transaction {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
            guard: RefCell::new(Weak::new()),
        }
    }
}

impl Transaction {
    fn acquire_guard(&self) -> GuardRc {
        if let Some(rc) = self.guard.borrow().upgrade() {
            return rc;
        }
        let guard: Guard = Arc::clone(&self.inner).lock_arc();
        let rc: GuardRc = Rc::new(guard);
        *self.guard.borrow_mut() = Rc::downgrade(&rc);
        rc
    }

    pub fn inc(&self) -> Guarded<'_, i32> {
        let rc = self.acquire_guard();
        let ptr = NonNull::from(&rc.counter).cast::<i32>();
        unsafe { *ptr.as_ptr() += 1 };
        Guarded::new(rc, ptr)
    }

    pub fn counter(&self) -> Guarded<'_, i32> {
        let rc = self.acquire_guard();
        let ptr = NonNull::from(&rc.counter).cast::<i32>();
        Guarded::new(rc, ptr)
    }
}

pub struct Guarded<'a, T> {
    _guard: GuardRc,
    ptr: NonNull<T>,
    _lt: std::marker::PhantomData<&'a mut T>,
}

impl<'a, T> Guarded<'a, T> {
    fn new(_guard: GuardRc, ptr: NonNull<T>) -> Self {
        Self {
            _guard,
            ptr,
            _lt: std::marker::PhantomData,
        }
    }
}

impl<'a, T> Deref for Guarded<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl<'a, T> DerefMut for Guarded<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn cache_and_relock_behaviour() {
        let tx = Transaction::default();

        let g1 = tx.inc();
        let g2 = tx.counter();
        assert!(Rc::ptr_eq(&g1._guard, &g2._guard));
        drop(g1);
        drop(g2);

        let g3 = tx.counter();
        assert_eq!(*g3, 1);
    }

    #[test]
    fn cache_and_relock_behaviour2() {
        let tx = Transaction::default();

        {
            let mut a = tx.inc();
            *a += 1;
            let b = tx.counter();
            assert!(Rc::ptr_eq(&a._guard, &b._guard));
        }

        let c = tx.counter();
        assert_eq!(*c, 2);
    }
}
