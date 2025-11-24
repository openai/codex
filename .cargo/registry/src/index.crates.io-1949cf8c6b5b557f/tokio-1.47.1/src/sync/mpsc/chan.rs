use crate::loom::cell::UnsafeCell;
use crate::loom::future::AtomicWaker;
use crate::loom::sync::atomic::AtomicUsize;
use crate::loom::sync::Arc;
use crate::runtime::park::CachedParkThread;
use crate::sync::mpsc::error::TryRecvError;
use crate::sync::mpsc::{bounded, list, unbounded};
use crate::sync::notify::Notify;
use crate::util::cacheline::CachePadded;

use std::fmt;
use std::panic;
use std::process;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};
use std::task::Poll::{Pending, Ready};
use std::task::{ready, Context, Poll};

/// Channel sender.
pub(crate) struct Tx<T, S> {
    inner: Arc<Chan<T, S>>,
}

impl<T, S: fmt::Debug> fmt::Debug for Tx<T, S> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Tx").field("inner", &self.inner).finish()
    }
}

/// Channel receiver.
pub(crate) struct Rx<T, S: Semaphore> {
    inner: Arc<Chan<T, S>>,
}

impl<T, S: Semaphore + fmt::Debug> fmt::Debug for Rx<T, S> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Rx").field("inner", &self.inner).finish()
    }
}

pub(crate) trait Semaphore {
    fn is_idle(&self) -> bool;

    fn add_permit(&self);

    fn add_permits(&self, n: usize);

    fn close(&self);

    fn is_closed(&self) -> bool;
}

pub(super) struct Chan<T, S> {
    /// Handle to the push half of the lock-free list.
    tx: CachePadded<list::Tx<T>>,

    /// Receiver waker. Notified when a value is pushed into the channel.
    rx_waker: CachePadded<AtomicWaker>,

    /// Notifies all tasks listening for the receiver being dropped.
    notify_rx_closed: Notify,

    /// Coordinates access to channel's capacity.
    semaphore: S,

    /// Tracks the number of outstanding sender handles.
    ///
    /// When this drops to zero, the send half of the channel is closed.
    tx_count: AtomicUsize,

    /// Tracks the number of outstanding weak sender handles.
    tx_weak_count: AtomicUsize,

    /// Only accessed by `Rx` handle.
    rx_fields: UnsafeCell<RxFields<T>>,
}

impl<T, S> fmt::Debug for Chan<T, S>
where
    S: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Chan")
            .field("tx", &*self.tx)
            .field("semaphore", &self.semaphore)
            .field("rx_waker", &*self.rx_waker)
            .field("tx_count", &self.tx_count)
            .field("rx_fields", &"...")
            .finish()
    }
}

/// Fields only accessed by `Rx` handle.
struct RxFields<T> {
    /// Channel receiver. This field is only accessed by the `Receiver` type.
    list: list::Rx<T>,

    /// `true` if `Rx::close` is called.
    rx_closed: bool,
}

impl<T> fmt::Debug for RxFields<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("RxFields")
            .field("list", &self.list)
            .field("rx_closed", &self.rx_closed)
            .finish()
    }
}

unsafe impl<T: Send, S: Send> Send for Chan<T, S> {}
unsafe impl<T: Send, S: Sync> Sync for Chan<T, S> {}
impl<T, S> panic::RefUnwindSafe for Chan<T, S> {}
impl<T, S> panic::UnwindSafe for Chan<T, S> {}

pub(crate) fn channel<T, S: Semaphore>(semaphore: S) -> (Tx<T, S>, Rx<T, S>) {
    let (tx, rx) = list::channel();

    let chan = Arc::new(Chan {
        notify_rx_closed: Notify::new(),
        tx: CachePadded::new(tx),
        semaphore,
        rx_waker: CachePadded::new(AtomicWaker::new()),
        tx_count: AtomicUsize::new(1),
        tx_weak_count: AtomicUsize::new(0),
        rx_fields: UnsafeCell::new(RxFields {
            list: rx,
            rx_closed: false,
        }),
    });

    (Tx::new(chan.clone()), Rx::new(chan))
}

// ===== impl Tx =====

impl<T, S> Tx<T, S> {
    fn new(chan: Arc<Chan<T, S>>) -> Tx<T, S> {
        Tx { inner: chan }
    }

    pub(super) fn strong_count(&self) -> usize {
        self.inner.tx_count.load(Acquire)
    }

    pub(super) fn weak_count(&self) -> usize {
        self.inner.tx_weak_count.load(Relaxed)
    }

    pub(super) fn downgrade(&self) -> Arc<Chan<T, S>> {
        self.inner.increment_weak_count();

        self.inner.clone()
    }

    // Returns the upgraded channel or None if the upgrade failed.
    pub(super) fn upgrade(chan: Arc<Chan<T, S>>) -> Option<Self> {
        let mut tx_count = chan.tx_count.load(Acquire);

        loop {
            if tx_count == 0 {
                // channel is closed
                return None;
            }

            match chan
                .tx_count
                .compare_exchange_weak(tx_count, tx_count + 1, AcqRel, Acquire)
            {
                Ok(_) => return Some(Tx { inner: chan }),
                Err(prev_count) => tx_count = prev_count,
            }
        }
    }

    pub(super) fn semaphore(&self) -> &S {
        &self.inner.semaphore
    }

    /// Send a message and notify the receiver.
    pub(crate) fn send(&self, value: T) {
        self.inner.send(value);
    }

    /// Wake the receive half
    pub(crate) fn wake_rx(&self) {
        self.inner.rx_waker.wake();
    }

    /// Returns `true` if senders belong to the same channel.
    pub(crate) fn same_channel(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T, S: Semaphore> Tx<T, S> {
    pub(crate) fn is_closed(&self) -> bool {
        self.inner.semaphore.is_closed()
    }

    pub(crate) async fn closed(&self) {
        // In order to avoid a race condition, we first request a notification,
        // **then** check whether the semaphore is closed. If the semaphore is
        // closed the notification request is dropped.
        let notified = self.inner.notify_rx_closed.notified();

        if self.inner.semaphore.is_closed() {
            return;
        }
        notified.await;
    }
}

impl<T, S> Clone for Tx<T, S> {
    fn clone(&self) -> Tx<T, S> {
        // Using a Relaxed ordering here is sufficient as the caller holds a
        // strong ref to `self`, preventing a concurrent decrement to zero.
        self.inner.tx_count.fetch_add(1, Relaxed);

        Tx {
            inner: self.inner.clone(),
        }
    }
}

impl<T, S> Drop for Tx<T, S> {
    fn drop(&mut self) {
        if self.inner.tx_count.fetch_sub(1, AcqRel) != 1 {
            return;
        }

        // Close the list, which sends a `Close` message
        self.inner.tx.close();

        // Notify the receiver
        self.wake_rx();
    }
}

// ===== impl Rx =====

impl<T, S: Semaphore> Rx<T, S> {
    fn new(chan: Arc<Chan<T, S>>) -> Rx<T, S> {
        Rx { inner: chan }
    }

    pub(crate) fn close(&mut self) {
        self.inner.rx_fields.with_mut(|rx_fields_ptr| {
            let rx_fields = unsafe { &mut *rx_fields_ptr };

            if rx_fields.rx_closed {
                return;
            }

            rx_fields.rx_closed = true;
        });

        self.inner.semaphore.close();
        self.inner.notify_rx_closed.notify_waiters();
    }

    pub(crate) fn is_closed(&self) -> bool {
        // There two internal states that can represent a closed channel
        //
        //  1. When `close` is called.
        //  In this case, the inner semaphore will be closed.
        //
        //  2. When all senders are dropped.
        //  In this case, the semaphore remains unclosed, and the `index` in the list won't
        //  reach the tail position. It is necessary to check the list if the last block is
        //  `closed`.
        self.inner.semaphore.is_closed() || self.inner.tx_count.load(Acquire) == 0
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.rx_fields.with(|rx_fields_ptr| {
            let rx_fields = unsafe { &*rx_fields_ptr };
            rx_fields.list.is_empty(&self.inner.tx)
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.inner.rx_fields.with(|rx_fields_ptr| {
            let rx_fields = unsafe { &*rx_fields_ptr };
            rx_fields.list.len(&self.inner.tx)
        })
    }

    /// Receive the next value
    pub(crate) fn recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<T>> {
        use super::block::Read;

        ready!(crate::trace::trace_leaf(cx));

        // Keep track of task budget
        let coop = ready!(crate::task::coop::poll_proceed(cx));

        self.inner.rx_fields.with_mut(|rx_fields_ptr| {
            let rx_fields = unsafe { &mut *rx_fields_ptr };

            macro_rules! try_recv {
                () => {
                    match rx_fields.list.pop(&self.inner.tx) {
                        Some(Read::Value(value)) => {
                            self.inner.semaphore.add_permit();
                            coop.made_progress();
                            return Ready(Some(value));
                        }
                        Some(Read::Closed) => {
                            // TODO: This check may not be required as it most
                            // likely can only return `true` at this point. A
                            // channel is closed when all tx handles are
                            // dropped. Dropping a tx handle releases memory,
                            // which ensures that if dropping the tx handle is
                            // visible, then all messages sent are also visible.
                            assert!(self.inner.semaphore.is_idle());
                            coop.made_progress();
                            return Ready(None);
                        }
                        None => {} // fall through
                    }
                };
            }

            try_recv!();

            self.inner.rx_waker.register_by_ref(cx.waker());

            // It is possible that a value was pushed between attempting to read
            // and registering the task, so we have to check the channel a
            // second time here.
            try_recv!();

            if rx_fields.rx_closed && self.inner.semaphore.is_idle() {
                coop.made_progress();
                Ready(None)
            } else {
                Pending
            }
        })
    }

    /// Receives up to `limit` values into `buffer`
    ///
    /// For `limit > 0`, receives up to limit values into `buffer`.
    /// For `limit == 0`, immediately returns Ready(0).
    pub(crate) fn recv_many(
        &mut self,
        cx: &mut Context<'_>,
        buffer: &mut Vec<T>,
        limit: usize,
    ) -> Poll<usize> {
        use super::block::Read;

        ready!(crate::trace::trace_leaf(cx));

        // Keep track of task budget
        let coop = ready!(crate::task::coop::poll_proceed(cx));

        if limit == 0 {
            coop.made_progress();
            return Ready(0usize);
        }

        let mut remaining = limit;
        let initial_length = buffer.len();

        self.inner.rx_fields.with_mut(|rx_fields_ptr| {
            let rx_fields = unsafe { &mut *rx_fields_ptr };
            macro_rules! try_recv {
                () => {
                    while remaining > 0 {
                        match rx_fields.list.pop(&self.inner.tx) {
                            Some(Read::Value(value)) => {
                                remaining -= 1;
                                buffer.push(value);
                            }

                            Some(Read::Closed) => {
                                let number_added = buffer.len() - initial_length;
                                if number_added > 0 {
                                    self.inner.semaphore.add_permits(number_added);
                                }
                                // TODO: This check may not be required as it most
                                // likely can only return `true` at this point. A
                                // channel is closed when all tx handles are
                                // dropped. Dropping a tx handle releases memory,
                                // which ensures that if dropping the tx handle is
                                // visible, then all messages sent are also visible.
                                assert!(self.inner.semaphore.is_idle());
                                coop.made_progress();
                                return Ready(number_added);
                            }

                            None => {
                                break; // fall through
                            }
                        }
                    }
                    let number_added = buffer.len() - initial_length;
                    if number_added > 0 {
                        self.inner.semaphore.add_permits(number_added);
                        coop.made_progress();
                        return Ready(number_added);
                    }
                };
            }

            try_recv!();

            self.inner.rx_waker.register_by_ref(cx.waker());

            // It is possible that a value was pushed between attempting to read
            // and registering the task, so we have to check the channel a
            // second time here.
            try_recv!();

            if rx_fields.rx_closed && self.inner.semaphore.is_idle() {
                assert!(buffer.is_empty());
                coop.made_progress();
                Ready(0usize)
            } else {
                Pending
            }
        })
    }

    /// Try to receive the next value.
    pub(crate) fn try_recv(&mut self) -> Result<T, TryRecvError> {
        use super::list::TryPopResult;

        self.inner.rx_fields.with_mut(|rx_fields_ptr| {
            let rx_fields = unsafe { &mut *rx_fields_ptr };

            macro_rules! try_recv {
                () => {
                    match rx_fields.list.try_pop(&self.inner.tx) {
                        TryPopResult::Ok(value) => {
                            self.inner.semaphore.add_permit();
                            return Ok(value);
                        }
                        TryPopResult::Closed => return Err(TryRecvError::Disconnected),
                        TryPopResult::Empty => return Err(TryRecvError::Empty),
                        TryPopResult::Busy => {} // fall through
                    }
                };
            }

            try_recv!();

            // If a previous `poll_recv` call has set a waker, we wake it here.
            // This allows us to put our own CachedParkThread waker in the
            // AtomicWaker slot instead.
            //
            // This is not a spurious wakeup to `poll_recv` since we just got a
            // Busy from `try_pop`, which only happens if there are messages in
            // the queue.
            self.inner.rx_waker.wake();

            // Park the thread until the problematic send has completed.
            let mut park = CachedParkThread::new();
            let waker = park.waker().unwrap();
            loop {
                self.inner.rx_waker.register_by_ref(&waker);
                // It is possible that the problematic send has now completed,
                // so we have to check for messages again.
                try_recv!();
                park.park();
            }
        })
    }

    pub(super) fn semaphore(&self) -> &S {
        &self.inner.semaphore
    }

    pub(super) fn sender_strong_count(&self) -> usize {
        self.inner.tx_count.load(Acquire)
    }

    pub(super) fn sender_weak_count(&self) -> usize {
        self.inner.tx_weak_count.load(Relaxed)
    }
}

impl<T, S: Semaphore> Drop for Rx<T, S> {
    fn drop(&mut self) {
        use super::block::Read::Value;

        self.close();

        self.inner.rx_fields.with_mut(|rx_fields_ptr| {
            let rx_fields = unsafe { &mut *rx_fields_ptr };
            struct Guard<'a, T, S: Semaphore> {
                list: &'a mut list::Rx<T>,
                tx: &'a list::Tx<T>,
                sem: &'a S,
            }

            impl<'a, T, S: Semaphore> Guard<'a, T, S> {
                fn drain(&mut self) {
                    // call T's destructor.
                    while let Some(Value(_)) = self.list.pop(self.tx) {
                        self.sem.add_permit();
                    }
                }
            }

            impl<'a, T, S: Semaphore> Drop for Guard<'a, T, S> {
                fn drop(&mut self) {
                    self.drain();
                }
            }

            let mut guard = Guard {
                list: &mut rx_fields.list,
                tx: &self.inner.tx,
                sem: &self.inner.semaphore,
            };

            guard.drain();
        });
    }
}

// ===== impl Chan =====

impl<T, S> Chan<T, S> {
    fn send(&self, value: T) {
        // Push the value
        self.tx.push(value);

        // Notify the rx task
        self.rx_waker.wake();
    }

    pub(super) fn decrement_weak_count(&self) {
        self.tx_weak_count.fetch_sub(1, Relaxed);
    }

    pub(super) fn increment_weak_count(&self) {
        self.tx_weak_count.fetch_add(1, Relaxed);
    }

    pub(super) fn strong_count(&self) -> usize {
        self.tx_count.load(Acquire)
    }

    pub(super) fn weak_count(&self) -> usize {
        self.tx_weak_count.load(Relaxed)
    }
}

impl<T, S> Drop for Chan<T, S> {
    fn drop(&mut self) {
        use super::block::Read::Value;

        // Safety: the only owner of the rx fields is Chan, and being
        // inside its own Drop means we're the last ones to touch it.
        self.rx_fields.with_mut(|rx_fields_ptr| {
            let rx_fields = unsafe { &mut *rx_fields_ptr };

            while let Some(Value(_)) = rx_fields.list.pop(&self.tx) {}
            unsafe { rx_fields.list.free_blocks() };
        });
    }
}

// ===== impl Semaphore for (::Semaphore, capacity) =====

impl Semaphore for bounded::Semaphore {
    fn add_permit(&self) {
        self.semaphore.release(1);
    }

    fn add_permits(&self, n: usize) {
        self.semaphore.release(n)
    }

    fn is_idle(&self) -> bool {
        self.semaphore.available_permits() == self.bound
    }

    fn close(&self) {
        self.semaphore.close();
    }

    fn is_closed(&self) -> bool {
        self.semaphore.is_closed()
    }
}

// ===== impl Semaphore for AtomicUsize =====

impl Semaphore for unbounded::Semaphore {
    fn add_permit(&self) {
        let prev = self.0.fetch_sub(2, Release);

        if prev >> 1 == 0 {
            // Something went wrong
            process::abort();
        }
    }

    fn add_permits(&self, n: usize) {
        let prev = self.0.fetch_sub(n << 1, Release);

        if (prev >> 1) < n {
            // Something went wrong
            process::abort();
        }
    }

    fn is_idle(&self) -> bool {
        self.0.load(Acquire) >> 1 == 0
    }

    fn close(&self) {
        self.0.fetch_or(1, Release);
    }

    fn is_closed(&self) -> bool {
        self.0.load(Acquire) & 1 == 1
    }
}
