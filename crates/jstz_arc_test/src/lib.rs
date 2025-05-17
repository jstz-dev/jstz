use parking_lot::{ArcMutexGuard, Mutex, RawMutex};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    rc::{Rc, Weak},
    sync::Arc,
};

#[derive(Debug, Default)]
pub struct Inner(BTreeMap<u64, u64>);

type GuardInner = ArcMutexGuard<RawMutex, Inner>;
type RcGuardInner = Rc<RefCell<GuardInner>>;

#[derive(Debug)]
pub struct Transaction {
    inner: Arc<Mutex<Inner>>,
    guard: RefCell<Weak<RefCell<GuardInner>>>,
}

impl Default for Transaction {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
            guard: RefCell::new(Weak::new()),
        }
    }
}

impl Clone for Transaction {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            guard: RefCell::new(Weak::new()),
        }
    }
}

impl Transaction {
    pub fn new(inner: Inner) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
            ..Default::default()
        }
    }

    fn acquire_guard(&self) -> Option<RcGuardInner>{
        if let Some(rc) = self.guard.borrow().upgrade() {
            return Some(rc);
        }
        let guard: GuardInner = Arc::clone(&self.inner).try_lock_arc()?;
        let rc: RcGuardInner = Rc::new(RefCell::new(guard));
        *self.guard.borrow_mut() = Rc::downgrade(&rc);
        Some(rc)
    }

    pub fn get<'a>(&'a self, key: &u64) -> Option<Guarded<'a, u64>> {
        let rc = self.acquire_guard()?;
        let guard = rc.clone();
        let mut rc_borrowed = rc.borrow_mut();
        let value = rc_borrowed.0.get_mut(key);
        value.map(|v| Guarded::new(guard, v))
    }

    pub fn get_mut<'a>(&'a mut self, key: &u64) -> Option<Guarded<'a, u64>> {
        self.get(key)
    }

    pub fn insert(&mut self, key: u64, value: u64) -> Option<()>{
        let guard = self.acquire_guard()?;
        let mut guard_mut = guard.borrow_mut();
        guard_mut.0.insert(key, value);
        Some(())
    }
}

pub struct Guarded<'a, T: 'a> {
    value: *mut T,
    _guard: RcGuardInner,
    _marker: PhantomData<&'a ()>,
}

impl<'a, T> Guarded<'a, T> {
    fn new<'b>(guard: RcGuardInner, value: &mut T) -> Guarded<'b, T> {
        Guarded {
            value,
            _guard: guard,
            _marker: PhantomData,
        }
    }
}

impl<'a, T> Deref for Guarded<'a, T> {
    type Target = T;

    fn deref(&self) -> &'a Self::Target {
        // Safety
        // The lifetime bound 'a is from &'a Transaction. Rust will guarantee that &'a Transaction will
        // not drop before Guarded<'a, T> which indirectly enforces that the self.value will be valid
        // for the lifetime of 'a
        unsafe { &*self.value }
    }
}

impl<'a, T> DerefMut for Guarded<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety
        // The lifetime bound 'a is from &'a Transaction. Rust will guarantee that &'a Transaction will
        // not drop before Guarded<'a, T> which indirectly enforces that the self.value will be valid
        // for the lifetime of 'a
        unsafe { &mut *self.value }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locking() {
        let mut tx = Transaction::default();
        tx.insert(1, 100);
        tx.insert(2, 200);
        tx.insert(3, 300);
        tx.insert(4, 400);
        let another_tx = tx.clone();
        {
            let value1 = tx.get(&1).unwrap();
            let value2 = tx.get(&2).unwrap();
            assert!(Rc::ptr_eq(&value1._guard, &value2._guard));
            assert_eq!(*value1, 100);
            assert_eq!(*value2, 200);
            let mut value3 = tx.get_mut(&3).unwrap();
            *value3 = 24;
            assert_eq!(*value3, 24);

            // Next line will hang out program
            // let value4 = another_tx.get(&4)
        }
        let value4 = another_tx.get(&4).unwrap();
        assert_eq!(*value4, 400);
    }

    fn do_something(tx: &Transaction) -> Guarded<u64>{
        tx.get(&1).unwrap()
    }

    #[test]
    fn test_borrow_checker() {
        let mut tx = Transaction::default();
        tx.insert(1, 100);
        tx.insert(2, 200);
        tx.insert(3, 300);
        tx.insert(4, 400);
        let value1 = tx.get(&1).unwrap();
        let value2 = tx.get(&2).unwrap();
        assert!(Rc::ptr_eq(&value1._guard, &value2._guard));
        assert_eq!(*value1, 100);
        assert_eq!(*value2, 200);
        let mut value3 = tx.get_mut(&3).unwrap();
        *value3 = 24;
        assert_eq!(*value3, 24);

        do_something(&tx);

        // Code snippets below will not compile because of Rust borrow checker rules

        // let value4 = tx.get_mut(&4).unwrap();
        // *value3 = 240
        //
        // error[E0499]: cannot borrow `tx` as mutable more than once at a time
        // --> crates/jstz_arc_test/src/lib.rs:123:22
        //     |
        // 118 |         let mut value3 = tx.get_mut(&3).unwrap();
        //     |                          -- first mutable borrow occurs here
        // ...
        // 123 |         let value4 = tx.get_mut(&4).unwrap();
        //     |                      ^^ second mutable borrow occurs here
        // 124 |         *value3 = 240
        //     |          ------ first borrow later used here

        // let value4 = tx.get(&4).unwrap();
        // *value3 = 240
        //
        // error[E0502]: cannot borrow `tx` as immutable because it is also borrowed as mutable
        // --> crates/jstz_arc_test/src/lib.rs:138:22
        //     |
        // 118 |         let mut value3 = tx.get_mut(&3).unwrap();
        //     |                          -- mutable borrow occurs here
        // ...
        // 138 |         let value4 = tx.get(&4).unwrap();
        //     |                      ^^ immutable borrow occurs here
        // 139 |         *value3 = 240
        //     |          ------ mutable borrow later used here

        // drop(tx);
        // *value3 = 240;

        // error[E0505]: cannot move out of `tx` because it is borrowed
        // --> crates/jstz_arc_test/src/lib.rs:155:14
        //     |
        // 108 |         let mut tx = Transaction::default();
        //     |             ------ binding `tx` declared here
        // ...
        // 121 |         let mut value3 = tx.get_mut(&3).unwrap();
        //     |                          -- borrow of `tx` occurs here
        // ...
        // 155 |         drop(tx);
        //     |              ^^ move out of `tx` occurs here
        // 156 |         *value3 = 240;
        //     |          ------ borrow later used here
    }

    // #[test]
    // fn cache_and_relock_behaviour() {
    //     let tx = Transaction::default();

    //     let g1 = tx.inc();
    //     let g2 = tx.counter();
    //     assert!(Rc::ptr_eq(&g1._guard, &g2._guard));
    //     drop(g1);
    //     drop(g2);

    //     let g3 = tx.counter();
    //     assert_eq!(*g3, 1);
    // }

    // #[test]
    // fn cache_and_relock_behaviour2() {
    //     let tx = Transaction::default();

    //     {
    //         let mut a = tx.inc();
    //         *a += 1;
    //         let b = tx.counter();
    //         assert!(Rc::ptr_eq(&a._guard, &b._guard));
    //     }

    //     let c = tx.counter();
    //     assert_eq!(*c, 2);
    // }
}
