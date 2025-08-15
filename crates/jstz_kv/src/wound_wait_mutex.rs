//! A wound-wait mutex implementation.
//!
//! This module provides a specialized mutex that implements the wound-wait deadlock prevention
//! algorithm used in transactional systems:
//!
//! - Wound: When an older transaction (lower ID) requests a resource held by a younger
//!   transaction (higher ID), the younger transaction is "wounded" (aborted) and the older
//!   transaction gets the resource.
//!
//! - Wait: When a younger transaction requests a resource held by an older transaction,
//!   the younger transaction waits.
//!
//! This approach ensures that deadlocks cannot occur because there's always a consistent ordering
//! based on transaction timestamps/IDs.

use std::{
    cell::UnsafeCell,
    collections::BinaryHeap,
    fmt::Debug,
    future::Future,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use pin_project::pin_project;
use tokio_util::sync::CancellationToken;

/// A min-priority queue based on BinaryHeap (max-heap), where the priority is the transaction ID.
/// Lower IDs have higher priority (older transactions). This ensures older transactions
/// are processed first when multiple waiters exist.
type MinPriorityQueue<T> = BinaryHeap<std::cmp::Reverse<T>>;

/// Inner state of a raw wound-wait mutex containing the wait queue and current holder.
/// This struct is protected by a standard mutex to ensure thread-safe access.
#[derive(Debug)]
struct RawWoundWaitMutexInner<Id: Ord> {
    /// Queue of waiting operations ordered by transaction ID.
    waiters: MinPriorityQueue<Waiter<Id>>,
    /// Current holder of the mutex, if any.
    holder: Option<ActiveHolder<Id>>,
}

/// A raw wound-wait mutex that provides the core functionality for managing waiters and holders.
/// This is the main synchronization primitive that implements the wound-wait algorithm.
#[derive(Debug)]
struct RawWoundWaitMutex<Id: Ord> {
    /// Inner state of the mutex, including waiters and current holder.
    /// Protected by a standard mutex to ensure atomic state transitions.
    inner: Mutex<RawWoundWaitMutexInner<Id>>,
}

/// Error returned from the [`RawWoundWaitMutex::try_lock`] function.
#[derive(Debug, PartialEq, Eq)]
pub enum TryLockError {
    /// The mutex is currently held by another transaction.
    LockHeld,
    /// The transaction was cancelled before acquiring the lock.
    Cancelled,
}

/// Error returned from the [`RawWoundWaitMutex::lock`] function.
#[derive(Debug, PartialEq, Eq)]
pub enum LockError {
    /// The transaction was cancelled before acquiring the lock.
    Cancelled,
}

/// Represents the current holder of the wound-wait mutex.
///
/// The holder can be "wounded" (cancelled) by an older transaction
/// attempting to acquire the same mutex.
#[derive(Debug)]
struct ActiveHolder<Id: Ord> {
    /// Transaction ID of the current holder
    id: Id,
    /// Cancellation token used to "wound" (abort) this holder
    cancellation_token: CancellationToken,
}

/// An entry in the wait queue representing a transaction waiting to acquire the mutex.
/// Waiters are ordered by transaction ID in a priority queue, with older transactions having higher priority.
#[derive(Debug)]
struct Waiter<Id: Ord> {
    /// Transaction ID of the waiting transaction
    id: Id,

    /// The waker to notify when the mutex becomes available
    waker: Waker,

    /// Cancellation token to signal if the waiter should be aborted
    cancellation_token: CancellationToken,
}

impl<Id: Ord> PartialEq for Waiter<Id> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<Id: Ord> Eq for Waiter<Id> {}

impl<Id: Ord> Ord for Waiter<Id> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}
impl<Id: Ord> PartialOrd for Waiter<Id> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<Id: Ord> Waiter<Id> {
    /// Checks if this waiter has been cancelled (wounded by an older transaction).
    fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Wakes up this waiter, signaling that it should retry acquiring the mutex.
    fn wake(self) {
        self.waker.wake();
    }
}

impl<Id: Ord> RawWoundWaitMutexInner<Id> {
    fn new() -> Self {
        Self {
            waiters: MinPriorityQueue::new(),
            holder: None,
        }
    }

    /// Checks if there are any waiters with an ID lower than the given ID.
    /// Returns true if older transactions are already waiting, meaning this transaction should wait.
    fn has_older_waiters(&self, id: &Id) -> bool {
        // Check if there are any waiters with an id less than the current id
        match self.waiters.peek() {
            None => false,
            Some(std::cmp::Reverse(waiter)) => waiter.id < *id,
        }
    }

    /// Wakes up the next eligible waiter from the wait queue.
    /// Skips cancelled waiters and wakes the oldest (lowest ID) non-cancelled waiter.
    fn wake_next_waiter(&mut self) {
        while let Some(std::cmp::Reverse(waiter)) = self.waiters.pop() {
            if waiter.is_cancelled() {
                // If the waiter is cancelled, we skip it
                continue;
            }

            waiter.wake();
            break;
        }
    }

    /// Registers a waiter in the wait queue if it hasn't been cancelled.
    /// The waiter is inserted into the priority queue ordered by transaction ID.
    fn register_waiter(&mut self, waiter: Waiter<Id>) {
        // If the cancellation token is already cancelled, we can't register the waiter
        if waiter.is_cancelled() {
            return;
        }

        // Add the waiter to the queue (ordered by ID)
        self.waiters.push(std::cmp::Reverse(waiter));
    }

    /// Attempts to acquire the lock without blocking.
    ///
    /// See [`RawWoundWaitMutex::try_lock`] for more details.
    fn try_lock(
        &mut self,
        id: Id,
        cancellation_token: CancellationToken,
    ) -> Result<(), TryLockError>
    where
        Id: Clone,
    {
        // If the cancellation token is already cancelled, we can't lock
        if cancellation_token.is_cancelled() {
            return Err(TryLockError::Cancelled);
        }

        // If there's a current holder with a higher ID (younger), wound it and take the lock
        if let Some(ref holder) = self.holder {
            if id < holder.id {
                // Wound the current holder
                holder.cancellation_token.cancel();
            }
            return Err(TryLockError::LockHeld);
        }

        // Check if there are any older waiters
        if self.has_older_waiters(&id) {
            return Err(TryLockError::LockHeld);
        }

        // At this point, we can take the lock
        self.holder = Some(ActiveHolder {
            id: id.clone(),
            cancellation_token: cancellation_token.clone(),
        });

        Ok(())
    }

    /// Unlocks the mutex and wakes up the next waiting transaction.
    ///
    /// See [`RawWoundWaitMutex::unlock`] for more details.
    unsafe fn unlock(&mut self) {
        self.holder = None;
        self.wake_next_waiter();
    }
}

impl<Id: Ord> RawWoundWaitMutex<Id> {
    fn new() -> Self {
        Self {
            inner: Mutex::new(RawWoundWaitMutexInner::new()),
        }
    }

    /// Attempts to acquire the lock without blocking.
    ///
    /// If successful, the calling transaction becomes the holder. If the lock is currently
    /// held by a younger transaction, that transaction is wounded (cancelled) and this
    /// transaction takes the lock. If held by an older transaction or older waiters exist,
    /// returns an error indicating the lock is held.
    fn try_lock(
        &self,
        id: Id,
        cancellation_token: CancellationToken,
    ) -> Result<(), TryLockError>
    where
        Id: Clone,
    {
        // SAFETY: We have exclusive access to the mutex, so we know it is not poisoned.
        let mut this = self.inner.lock().expect("Mutex cannot be poisoned");
        this.try_lock(id, cancellation_token)
    }

    /// Unlocks the mutex and wakes up the next waiting transaction.
    ///
    /// After releasing the lock, the oldest waiting transaction (if any) is notified
    /// to retry acquiring the mutex.
    ///
    /// # Safety
    ///
    /// This function must only be called by the current holder of the mutex.
    unsafe fn unlock(&self) {
        // SAFETY: We have exclusive access to the mutex, so we know it is not poisoned.
        let mut this = self.inner.lock().expect("Mutex cannot be poisoned");
        this.unlock();
    }

    /// Creates a future that will resolve when the lock is acquired.
    /// The future implements the wound-wait logic, including waiting for older transactions
    /// and being wounded by them if necessary.
    fn lock(
        &self,
        id: Id,
        cancellation_token: CancellationToken,
    ) -> RawLockFuture<'_, Id> {
        RawLockFuture::new(self, id, cancellation_token)
    }
}

/// A future that resolves when a lock on the `RawWoundWaitMutex` is acquired.
///
/// This future first attempts to acquire the lock immediately. If that fails,
/// it registers itself as a waiter and will be woken when the lock becomes available
/// or when the transaction is cancelled (wounded).
#[derive(Debug)]
#[pin_project]
struct RawLockFuture<'a, Id: Ord> {
    raw_mutex: &'a RawWoundWaitMutex<Id>,
    id: Id,
    cancellation_token: CancellationToken,
    waker_registered: bool,
}

impl<'a, Id: Ord> RawLockFuture<'a, Id> {
    fn new(
        raw_mutex: &'a RawWoundWaitMutex<Id>,
        id: Id,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            raw_mutex,
            id,
            cancellation_token,
            waker_registered: false,
        }
    }
}

impl<Id: Ord + Clone> Future for RawLockFuture<'_, Id> {
    type Output = Result<(), LockError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.cancellation_token.is_cancelled() {
            // If the cancellation token is cancelled, we can't lock
            return Poll::Ready(Err(LockError::Cancelled));
        }

        // SAFETY: We have exclusive access to the mutex, so we know it is not poisoned.
        let mut raw_mutex = self
            .raw_mutex
            .inner
            .lock()
            .expect("Mutex cannot be poisoned");

        match raw_mutex.try_lock(self.id.clone(), self.cancellation_token.clone()) {
            Ok(()) => {
                // If we successfully acquired the lock, we can return Ok
                return Poll::Ready(Ok(()));
            }
            Err(TryLockError::Cancelled) => {
                return Poll::Ready(Err(LockError::Cancelled))
            }
            Err(TryLockError::LockHeld) => (),
        }

        // If we can't acquire the lock, we need to register the waiter (if not already registered)
        if !self.waker_registered {
            raw_mutex.register_waiter(Waiter {
                waker: cx.waker().clone(),
                id: self.id.clone(),
                cancellation_token: self.cancellation_token.clone(),
            });

            // Register the waker
            self.waker_registered = true;
        }

        Poll::Pending
    }
}

/// An asynchronous wound-wait mutex primitive that protects shared data.
///
/// This mutex implements the wound-wait deadlock prevention algorithm, where older
/// transactions (lower IDs) can "wound" younger transactions holding locks they need,
/// while younger transactions must wait for older ones.
#[derive(Debug)]
pub struct WoundWaitMutex<Id: Ord, T> {
    /// The data being protected by the mutex.
    /// UnsafeCell to allow interior mutability. Safety is ensured by mutual exclusion.
    data: UnsafeCell<T>,
    /// Raw wound-wait mutex managing the state of the mutex.
    raw: RawWoundWaitMutex<Id>,
}

unsafe impl<Id: Ord, T: Send> Send for WoundWaitMutex<Id, T> {}
unsafe impl<Id: Ord, T: Sync> Sync for WoundWaitMutex<Id, T> {}

impl<Id: Ord, T> WoundWaitMutex<Id, T> {
    /// Creates a new unlocked wound-wait mutex protecting the given data.
    /// The mutex starts with no holder and an empty wait queue.
    pub fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            raw: RawWoundWaitMutex::new(),
        }
    }

    /// Consumes this mutex, returning the underlying data.
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<Id: Ord, T: Default> Default for WoundWaitMutex<Id, T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<Id: Ord, T> From<T> for WoundWaitMutex<Id, T> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

/// A RAII implementation of a "scoped lock" of a mutex. When this structure is dropped,
/// the lock will be released.
///
/// The data protected by the mutex can be accessed through dereferencing this guard.
#[derive(Debug)]
pub struct WoundWaitMutexGuard<'a, Id: Ord, T> {
    mutex: &'a WoundWaitMutex<Id, T>,
    _marker: PhantomData<&'a mut T>,
}

unsafe impl<'a, Id: Ord + Sync + 'a, T: Sync + 'a> Sync
    for WoundWaitMutexGuard<'a, Id, T>
{
}

impl<'a, Id: Ord + 'a, T: 'a> WoundWaitMutexGuard<'a, Id, T> {
    /// Returns a reference to the underlying mutex that this guard is protecting.
    pub fn mutex(&self) -> &'a WoundWaitMutex<Id, T> {
        self.mutex
    }
}

impl<'a, Id: Ord + 'a, T: 'a> Deref for WoundWaitMutexGuard<'a, Id, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, Id: Ord + 'a, T: 'a> DerefMut for WoundWaitMutexGuard<'a, Id, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, Id: Ord + 'a, T: 'a> Drop for WoundWaitMutexGuard<'a, Id, T> {
    fn drop(&mut self) {
        // SAFETY: A WoundWaitMutexGuard has exclusive access to the mutex.
        unsafe {
            self.mutex.raw.unlock();
        }
    }
}

/// An RAII mutex guard returned by the `lock_arc` method on [`WoundWaitMutex`].
///
/// This is similar to `WoundWaitMutexGuard`, but it holds an `Arc` to the mutex, allowing
/// it to be shared across multiple threads and has a static lifetime.
#[derive(Debug)]
pub struct ArcWoundWaitMutexGuard<Id: Ord, T> {
    mutex: Arc<WoundWaitMutex<Id, T>>,
    _marker: PhantomData<*const ()>,
}

unsafe impl<Id: Ord + Sync, T: Sync> Sync for ArcWoundWaitMutexGuard<Id, T> {}
unsafe impl<Id: Ord + Send, T: Send> Send for ArcWoundWaitMutexGuard<Id, T> {}

impl<Id: Ord, T> ArcWoundWaitMutexGuard<Id, T> {
    /// Returns a reference to the underlying mutex, contained in its `Arc`.
    /// This allows access to the mutex even after the guard is moved or cloned.
    pub fn mutex(&self) -> &Arc<WoundWaitMutex<Id, T>> {
        &self.mutex
    }
}

impl<Id: Ord, T> Deref for ArcWoundWaitMutexGuard<Id, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<Id: Ord, T> DerefMut for ArcWoundWaitMutexGuard<Id, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<Id: Ord, T> Drop for ArcWoundWaitMutexGuard<Id, T> {
    fn drop(&mut self) {
        // SAFETY: A ArcWoundWaitMutexGuard has exlusive access to the mutex.
        unsafe {
            self.mutex.raw.unlock();
        }
    }
}

impl<Id: Ord, T> WoundWaitMutex<Id, T> {
    /// Attempts to acquire the lock without blocking.
    /// If successful, returns a guard that provides access to the protected data.
    pub fn try_lock(
        &self,
        id: Id,
        cancellation_token: CancellationToken,
    ) -> Result<WoundWaitMutexGuard<'_, Id, T>, TryLockError>
    where
        Id: Clone,
    {
        // Attempt to acquire the lock without blocking
        self.raw.try_lock(id, cancellation_token)?;

        // If successful, return a guard that provides access to the protected data
        Ok(WoundWaitMutexGuard {
            mutex: self,
            _marker: PhantomData,
        })
    }

    /// Locks the mutex with the given transaction ID and cancellation token.
    ///
    /// Returns a guard that provides access to the protected data. The guard will
    /// automatically unlock the mutex when dropped. If the transaction is cancelled
    /// (wounded), returns a `LockError::Cancelled`.
    pub async fn lock(
        &self,
        id: Id,
        cancellation_token: CancellationToken,
    ) -> Result<WoundWaitMutexGuard<'_, Id, T>, LockError>
    where
        Id: Clone,
    {
        self.raw.lock(id, cancellation_token).await?;

        Ok(WoundWaitMutexGuard {
            mutex: self,
            _marker: PhantomData,
        })
    }

    /// Locks the mutex from an `Arc` reference, returning an owned guard.
    ///
    /// This is useful when the mutex needs to be locked from within an `Arc` and
    /// the guard needs to have a static lifetime. The guard holds the `Arc` reference,
    /// keeping the mutex alive even if other references are dropped.
    pub async fn lock_arc(
        self: &Arc<Self>,
        id: Id,
        cancellation_token: CancellationToken,
    ) -> Result<ArcWoundWaitMutexGuard<Id, T>, LockError>
    where
        Id: Clone,
    {
        self.raw.lock(id, cancellation_token).await?;

        Ok(ArcWoundWaitMutexGuard {
            mutex: self.clone(),
            _marker: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::future::poll_fn;
    use tokio::{pin, select, time};

    use super::*;

    fn spawn_waiter<T>(
        mutex: &Arc<WoundWaitMutex<i32, T>>,
        id: i32,
        cancellation_token: &CancellationToken,
    ) -> tokio::task::JoinHandle<Result<ArcWoundWaitMutexGuard<i32, T>, LockError>>
    where
        T: Send + Sync + 'static,
    {
        let mutex_clone = mutex.clone();
        let cancellation_token_clone = cancellation_token.clone();
        tokio::spawn(
            async move { mutex_clone.lock_arc(id, cancellation_token_clone).await },
        )
    }

    async fn poll_once<F: Future>(mut fut: Pin<&mut F>) -> Poll<F::Output> {
        // `poll_fn` runs inside the current Tokio task context.
        // The inner poll is wrapped in `Poll::Ready` so that `poll_fn` completes immediately after a single poll.
        poll_fn(|cx| Poll::Ready(fut.as_mut().poll(cx))).await
    }

    #[tokio::test]
    async fn test_basic_mutex() {
        let mutex = WoundWaitMutex::new(42);

        let cancel0 = CancellationToken::new();
        let id0 = 0;

        let mut guard = mutex.lock(id0, cancel0.clone()).await.unwrap();
        assert_eq!(*guard, 42);
        *guard = 100;
        drop(guard);

        let guard = mutex.lock(id0, cancel0.clone()).await.unwrap();
        assert_eq!(*guard, 100);
    }

    #[tokio::test]
    async fn test_wound_wait_semantics() {
        let mutex = WoundWaitMutex::new(42);

        let cancel0 = CancellationToken::new();
        let id0 = 0;
        let cancel1 = CancellationToken::new();
        let id1 = 1;

        // First lock
        let mut young_guard = mutex.lock(id1, cancel1.clone()).await.unwrap();
        assert!(!cancel1.is_cancelled());

        *young_guard = 200;

        // Second lock should wound the first
        let old_guard_fut = mutex.lock(id0, cancel0.clone());

        // Poll the future once to register the waker and check if it is pending
        pin!(old_guard_fut);
        assert!(matches!(
            poll_once(old_guard_fut.as_mut()).await,
            Poll::Pending
        ));

        assert!(cancel1.is_cancelled());
        assert!(!cancel0.is_cancelled());

        // The cancellation token is signalled, we must now drop the young guard
        drop(young_guard);

        // We can now await the old guard future
        let old_guard = old_guard_fut.await.unwrap();
        assert_eq!(*old_guard, 200);
    }

    #[tokio::test]
    async fn test_young_waits_for_old() {
        let mutex = Arc::new(WoundWaitMutex::new(42));

        let cancel0 = CancellationToken::new();
        let id0 = 0;
        let cancel1 = CancellationToken::new();
        let id1 = 1;

        let old_guard = mutex.lock(id0, cancel0.clone()).await.unwrap();
        let handle = spawn_waiter(&mutex, id1, &cancel1);

        // Give some time for the young waiter to acquire the lock
        time::sleep(Duration::from_millis(500)).await;
        assert!(!handle.is_finished());

        // Drop the old guard, waking the young waiter
        drop(old_guard);

        let young_guard = handle.await.unwrap().unwrap();
        assert!(!cancel1.is_cancelled());
        assert_eq!(*young_guard, 42);
    }

    #[tokio::test]
    async fn test_multiple_waiters_priority_order() {
        let mutex = Arc::new(WoundWaitMutex::new(100));

        let cancel0 = CancellationToken::new();
        let id0 = 0;
        let cancel1 = CancellationToken::new();
        let id1 = 1;
        let cancel2 = CancellationToken::new();
        let id2 = 2;
        let cancel3 = CancellationToken::new();
        let id3 = 3;

        let holder_guard = mutex.lock_arc(id0, cancel0.clone()).await.unwrap();

        // Spawn waiters in reverse order (highest ID first)
        let handle3 = spawn_waiter(&mutex, id3, &cancel3);
        let handle2 = spawn_waiter(&mutex, id2, &cancel2);
        let handle1 = spawn_waiter(&mutex, id1, &cancel1);

        time::sleep(Duration::from_millis(100)).await;

        // All waiters should be waiting
        assert!(!handle1.is_finished());
        assert!(!handle2.is_finished());
        assert!(!handle3.is_finished());

        // Release the lock
        drop(holder_guard);

        // Waiter 1 (lowest ID) should get the lock first
        let guard1 = handle1.await.unwrap().unwrap();
        assert_eq!(*guard1, 100);
        assert!(!cancel1.is_cancelled());

        // Others should still be waiting
        time::sleep(Duration::from_millis(50)).await;
        assert!(!handle2.is_finished());
        assert!(!handle3.is_finished());

        drop(guard1);

        // Waiter 2 should get it next
        let guard2 = handle2.await.unwrap().unwrap();
        assert_eq!(*guard2, 100);
        assert!(!cancel2.is_cancelled());

        drop(guard2);

        // Finally waiter 3
        let guard3 = handle3.await.unwrap().unwrap();
        assert_eq!(*guard3, 100);
        assert!(!cancel3.is_cancelled());
    }

    #[tokio::test]
    async fn test_cancelled_token_before_lock() {
        let mutex = WoundWaitMutex::new(25);

        let cancel0 = CancellationToken::new();
        let id0 = 0;

        // Cancel the token before trying to lock
        cancel0.cancel();

        let result = mutex.lock(id0, cancel0).await;
        assert_eq!(result.unwrap_err(), LockError::Cancelled);
    }

    #[tokio::test]
    async fn test_concurrent_same_priority_transactions() {
        let mutex = Arc::new(WoundWaitMutex::new(75));

        fn spawn_handle(
            mutex: &Arc<WoundWaitMutex<i32, i32>>,
            amount: i32,
        ) -> tokio::task::JoinHandle<i32> {
            let mutex_clone = mutex.clone();
            tokio::spawn(async move {
                let guard = mutex_clone
                    .lock_arc(0, CancellationToken::new())
                    .await
                    .unwrap();
                time::sleep(Duration::from_millis(100)).await;
                *guard + amount
            })
        }

        let handle1 = spawn_handle(&mutex, 10);
        let handle2 = spawn_handle(&mutex, 20);

        let results = tokio::join!(handle1, handle2);

        // Both should succeed, but we can't predict the order
        let result1 = results.0.unwrap();
        let result2 = results.1.unwrap();

        // One should see 75, the other should see the modified value
        let mut values = vec![result1, result2];
        values.sort();
        assert!(values == vec![85, 95]);
    }

    #[tokio::test]
    async fn test_mutex_data_consistency() {
        let mutex = Arc::new(WoundWaitMutex::new(0));
        let num_tasks = 50;

        let mut handles = Vec::new();

        for id in 0..num_tasks {
            let cancel = CancellationToken::new();
            let cancel_clone = cancel.clone();
            let mutex_clone = mutex.clone();
            let handle = tokio::spawn(async move {
                select! {
                    _ = cancel_clone.cancelled() => { },
                    result = mutex_clone.lock_arc(id, cancel.clone()) => {
                        if let Ok(mut guard) = result {
                            let current = *guard;
                            time::sleep(Duration::from_millis(20)).await; // Simulate work
                            *guard = current + 1;
                        }
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Final value should be consistent (some operations might have been cancelled)
        let final_guard = mutex.lock_arc(999, CancellationToken::new()).await.unwrap();
        let final_value = *final_guard;

        assert!(final_value <= num_tasks);
        // Should be greater than 0 (at least some operations succeeded)
        assert!(final_value > 0);
    }

    #[tokio::test]
    async fn test_wound_during_wait() {
        let mutex = Arc::new(WoundWaitMutex::new(30));

        // Transaction 2 holds the lock
        let cancel2 = CancellationToken::new();
        let guard2 = mutex.lock_arc(2, cancel2.clone()).await.unwrap();

        // Transaction 3 starts waiting
        let cancel3 = CancellationToken::new();

        let handle3 = spawn_waiter(&mutex, 3, &cancel3);

        time::sleep(Duration::from_millis(50)).await;
        assert!(!handle3.is_finished());

        // Transaction 1 (older) comes and should wound transaction 2 and take the lock
        let cancel1 = CancellationToken::new();
        let guard1_fut = mutex.lock_arc(1, cancel1.clone());

        // Poll the future to register the waker and check if it is pending
        pin!(guard1_fut);
        assert!(matches!(
            poll_once(guard1_fut.as_mut()).await,
            Poll::Pending
        ));

        // Transaction 2 should be wounded (and its guard is poisoned)
        assert!(cancel2.is_cancelled());
        drop(guard2);
        drop(cancel2);

        let mut guard1 = mutex.lock_arc(1, cancel1.clone()).await.unwrap();

        // Transaction 1 should have the lock
        *guard1 = 40;
        assert_eq!(*guard1, 40);
        assert!(!cancel1.is_cancelled());

        // Transaction 3 should still be waiting (older transaction has priority)
        time::sleep(Duration::from_millis(50)).await;
        assert!(!cancel3.is_cancelled());

        drop(guard1);

        // Now transaction 3 should get the lock
        let guard3 = handle3.await.unwrap().unwrap();
        assert_eq!(*guard3, 40);
        assert!(!cancel3.is_cancelled());
    }
}
