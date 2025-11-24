//! Raw, unsafe reader-writer locking implementation,
//! doesn't depend on the data protected by the lock.
//! [`RwLock`](super::RwLock) is implemented in terms of this.
//!
//! Splitting the implementation this way allows instantiating
//! the locking code only once, and also lets us make
//! [`RwLockReadGuard`](super::RwLockReadGuard) covariant in `T`.

use core::marker::PhantomPinned;
use core::mem::forget;
use core::pin::Pin;
use core::task::Poll;

use crate::sync::atomic::{AtomicUsize, Ordering};

use event_listener::{Event, EventListener};
use event_listener_strategy::{EventListenerFuture, Strategy};

use crate::futures::Lock;
use crate::Mutex;

const WRITER_BIT: usize = 1;
const ONE_READER: usize = 2;

/// A "raw" RwLock that doesn't hold any data.
pub(super) struct RawRwLock {
    /// Acquired by the writer.
    mutex: Mutex<()>,

    /// Event triggered when the last reader is dropped.
    no_readers: Event,

    /// Event triggered when the writer is dropped.
    no_writer: Event,

    /// Current state of the lock.
    ///
    /// The least significant bit (`WRITER_BIT`) is set to 1 when a writer is holding the lock or
    /// trying to acquire it.
    ///
    /// The upper bits contain the number of currently active readers. Each active reader
    /// increments the state by `ONE_READER`.
    state: AtomicUsize,
}

impl RawRwLock {
    const_fn! {
        const_if: #[cfg(not(loom))];
        #[inline]
        pub(super) const fn new() -> Self {
            RawRwLock {
                mutex: Mutex::new(()),
                no_readers: Event::new(),
                no_writer: Event::new(),
                state: AtomicUsize::new(0),
            }
        }
    }

    /// Returns `true` iff a read lock was successfully acquired.
    pub(super) fn try_read(&self) -> bool {
        let mut state = self.state.load(Ordering::Acquire);

        loop {
            // If there's a writer holding the lock or attempting to acquire it, we cannot acquire
            // a read lock here.
            if state & WRITER_BIT != 0 {
                return false;
            }

            // Make sure the number of readers doesn't overflow.
            if state > isize::MAX as usize {
                crate::abort();
            }

            // Increment the number of readers.
            match self.state.compare_exchange(
                state,
                state + ONE_READER,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(s) => state = s,
            }
        }
    }

    #[inline]
    pub(super) fn read(&self) -> RawRead<'_> {
        RawRead {
            lock: self,
            state: self.state.load(Ordering::Acquire),
            listener: None,
            _pin: PhantomPinned,
        }
    }

    /// Returns `true` iff an upgradable read lock was successfully acquired.
    pub(super) fn try_upgradable_read(&self) -> bool {
        // First try grabbing the mutex.
        let lock = if let Some(lock) = self.mutex.try_lock() {
            lock
        } else {
            return false;
        };

        forget(lock);

        let mut state = self.state.load(Ordering::Acquire);

        // Make sure the number of readers doesn't overflow.
        if state > isize::MAX as usize {
            crate::abort();
        }

        // Increment the number of readers.
        loop {
            match self.state.compare_exchange(
                state,
                state + ONE_READER,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(s) => state = s,
            }
        }
    }

    #[inline]
    pub(super) fn upgradable_read(&self) -> RawUpgradableRead<'_> {
        RawUpgradableRead {
            lock: self,
            acquire: self.mutex.lock(),
        }
    }

    /// Returns `true` iff a write lock was successfully acquired.
    pub(super) fn try_write(&self) -> bool {
        // First try grabbing the mutex.
        let lock = if let Some(lock) = self.mutex.try_lock() {
            lock
        } else {
            return false;
        };

        // If there are no readers, grab the write lock.
        if self
            .state
            .compare_exchange(0, WRITER_BIT, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            forget(lock);
            true
        } else {
            drop(lock);
            false
        }
    }

    #[inline]
    pub(super) fn write(&self) -> RawWrite<'_> {
        RawWrite {
            lock: self,
            no_readers: None,
            state: WriteState::Acquiring {
                lock: self.mutex.lock(),
            },
        }
    }

    /// Returns `true` iff a the upgradable read lock was successfully upgraded to a write lock.
    ///
    /// # Safety
    ///
    /// Caller must hold an upgradable read lock.
    /// This will attempt to upgrade it to a write lock.
    pub(super) unsafe fn try_upgrade(&self) -> bool {
        self.state
            .compare_exchange(ONE_READER, WRITER_BIT, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// # Safety
    ///
    /// Caller must hold an upgradable read lock.
    /// This will upgrade it to a write lock.
    pub(super) unsafe fn upgrade(&self) -> RawUpgrade<'_> {
        // Set `WRITER_BIT` and decrement the number of readers at the same time.
        self.state
            .fetch_sub(ONE_READER - WRITER_BIT, Ordering::SeqCst);

        RawUpgrade {
            lock: Some(self),
            listener: None,
            _pin: PhantomPinned,
        }
    }

    /// # Safety
    ///
    /// Caller must hold an upgradable read lock.
    /// This will downgrade it to a standard read lock.
    #[inline]
    pub(super) unsafe fn downgrade_upgradable_read(&self) {
        self.mutex.unlock_unchecked();
    }

    /// # Safety
    ///
    /// Caller must hold a write lock.
    /// This will downgrade it to a read lock.
    pub(super) unsafe fn downgrade_write(&self) {
        // Atomically downgrade state.
        self.state
            .fetch_add(ONE_READER - WRITER_BIT, Ordering::SeqCst);

        // Release the writer mutex.
        self.mutex.unlock_unchecked();

        // Trigger the "no writer" event.
        self.no_writer.notify(1);
    }

    /// # Safety
    ///
    /// Caller must hold a write lock.
    /// This will downgrade it to an upgradable read lock.
    pub(super) unsafe fn downgrade_to_upgradable(&self) {
        // Atomically downgrade state.
        self.state
            .fetch_add(ONE_READER - WRITER_BIT, Ordering::SeqCst);
    }

    /// # Safety
    ///
    /// Caller must hold a read lock .
    /// This will unlock that lock.
    pub(super) unsafe fn read_unlock(&self) {
        // Decrement the number of readers.
        if self.state.fetch_sub(ONE_READER, Ordering::SeqCst) & !WRITER_BIT == ONE_READER {
            // If this was the last reader, trigger the "no readers" event.
            self.no_readers.notify(1);
        }
    }

    /// # Safety
    ///
    /// Caller must hold an upgradable read lock.
    /// This will unlock that lock.
    pub(super) unsafe fn upgradable_read_unlock(&self) {
        // Decrement the number of readers.
        if self.state.fetch_sub(ONE_READER, Ordering::SeqCst) & !WRITER_BIT == ONE_READER {
            // If this was the last reader, trigger the "no readers" event.
            self.no_readers.notify(1);
        }

        // SAFETY: upgradable read guards acquire the writer mutex upon creation.
        self.mutex.unlock_unchecked();
    }

    /// # Safety
    ///
    /// Caller must hold a write lock.
    /// This will unlock that lock.
    pub(super) unsafe fn write_unlock(&self) {
        // Unset `WRITER_BIT`.
        self.state.fetch_and(!WRITER_BIT, Ordering::SeqCst);
        // Trigger the "no writer" event.
        self.no_writer.notify(1);

        // Release the writer lock.
        // SAFETY: `RwLockWriteGuard` always holds a lock on writer mutex.
        self.mutex.unlock_unchecked();
    }
}

pin_project_lite::pin_project! {
    /// The future returned by [`RawRwLock::read`].

    pub(super) struct RawRead<'a> {
        // The lock that is being acquired.
        pub(super) lock: &'a RawRwLock,

        // The last-observed state of the lock.
        state: usize,

        // The listener for the "no writers" event.
        listener: Option<EventListener>,

        // Making this type `!Unpin` enables future optimizations.
        #[pin]
        _pin: PhantomPinned
    }
}

impl EventListenerFuture for RawRead<'_> {
    type Output = ();

    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<()> {
        let this = self.project();

        loop {
            if *this.state & WRITER_BIT == 0 {
                // Make sure the number of readers doesn't overflow.
                if *this.state > isize::MAX as usize {
                    crate::abort();
                }

                // If nobody is holding a write lock or attempting to acquire it, increment the
                // number of readers.
                match this.lock.state.compare_exchange(
                    *this.state,
                    *this.state + ONE_READER,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => return Poll::Ready(()),
                    Err(s) => *this.state = s,
                }
            } else {
                // Start listening for "no writer" events.
                let load_ordering = if this.listener.is_none() {
                    *this.listener = Some(this.lock.no_writer.listen());

                    // Make sure there really is no writer.
                    Ordering::SeqCst
                } else {
                    // Wait for the writer to finish.
                    ready!(strategy.poll(this.listener, cx));

                    // Notify the next reader waiting in list.
                    this.lock.no_writer.notify(1);

                    // Check the state again.
                    Ordering::Acquire
                };

                // Reload the state.
                *this.state = this.lock.state.load(load_ordering);
            }
        }
    }
}

pin_project_lite::pin_project! {
    /// The future returned by [`RawRwLock::upgradable_read`].
    pub(super) struct RawUpgradableRead<'a> {
        // The lock that is being acquired.
        pub(super) lock: &'a RawRwLock,

        // The mutex we are trying to acquire.
        #[pin]
        acquire: Lock<'a, ()>,
    }
}

impl EventListenerFuture for RawUpgradableRead<'_> {
    type Output = ();

    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<()> {
        let this = self.project();

        // Acquire the mutex.
        let mutex_guard = ready!(this.acquire.poll_with_strategy(strategy, cx));
        forget(mutex_guard);

        // Load the current state.
        let mut state = this.lock.state.load(Ordering::Acquire);

        // Make sure the number of readers doesn't overflow.
        if state > isize::MAX as usize {
            crate::abort();
        }

        // Increment the number of readers.
        loop {
            match this.lock.state.compare_exchange(
                state,
                state + ONE_READER,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Poll::Ready(());
                }
                Err(s) => state = s,
            }
        }
    }
}

pin_project_lite::pin_project! {
    /// The future returned by [`RawRwLock::write`].

    pub(super) struct RawWrite<'a> {
        // The lock that is being acquired.
        pub(super) lock: &'a RawRwLock,

        // Our listener for the "no readers" event.
        no_readers: Option<EventListener>,

        // Current state of this future.
        #[pin]
        state: WriteState<'a>,
    }

    impl PinnedDrop for RawWrite<'_> {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();

            if matches!(this.state.project(), WriteStateProj::WaitingReaders) {
                // Safety: we hold a write lock, more or less.
                unsafe {
                    this.lock.write_unlock();
                }
            }
        }
    }
}

pin_project_lite::pin_project! {
    #[project = WriteStateProj]
    #[project_replace = WriteStateProjReplace]
    enum WriteState<'a> {
        // We are currently acquiring the inner mutex.
        Acquiring { #[pin] lock: Lock<'a, ()> },

        // We are currently waiting for readers to finish.
        WaitingReaders,

        // The future has completed.
        Acquired,
    }
}

impl EventListenerFuture for RawWrite<'_> {
    type Output = ();

    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<()> {
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                WriteStateProj::Acquiring { lock } => {
                    // First grab the mutex.
                    let mutex_guard = ready!(lock.poll_with_strategy(strategy, cx));
                    forget(mutex_guard);

                    // Set `WRITER_BIT` and create a guard that unsets it in case this future is canceled.
                    let new_state = this.lock.state.fetch_or(WRITER_BIT, Ordering::SeqCst);

                    // If we just acquired the lock, return.
                    if new_state == WRITER_BIT {
                        this.state.as_mut().set(WriteState::Acquired);
                        return Poll::Ready(());
                    }

                    // Start waiting for the readers to finish.
                    *this.no_readers = Some(this.lock.no_readers.listen());
                    this.state.as_mut().set(WriteState::WaitingReaders);
                }

                WriteStateProj::WaitingReaders => {
                    let load_ordering = if this.no_readers.is_some() {
                        Ordering::Acquire
                    } else {
                        Ordering::SeqCst
                    };

                    // Check the state again.
                    if this.lock.state.load(load_ordering) == WRITER_BIT {
                        // We are the only ones holding the lock, return `Ready`.
                        this.state.as_mut().set(WriteState::Acquired);
                        return Poll::Ready(());
                    }

                    // Wait for the readers to finish.
                    if this.no_readers.is_none() {
                        // Register a listener.
                        *this.no_readers = Some(this.lock.no_readers.listen());
                    } else {
                        // Wait for the readers to finish.
                        ready!(strategy.poll(this.no_readers, cx));
                    };
                }
                WriteStateProj::Acquired => panic!("Write lock already acquired"),
            }
        }
    }
}

pin_project_lite::pin_project! {
    /// The future returned by [`RawRwLock::upgrade`].

    pub(super) struct RawUpgrade<'a> {
        lock: Option<&'a RawRwLock>,

        // The event listener we are waiting on.
        listener: Option<EventListener>,

        // Keeping this future `!Unpin` enables future optimizations.
        #[pin]
        _pin: PhantomPinned
    }

    impl PinnedDrop for RawUpgrade<'_> {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            if let Some(lock) = this.lock {
                // SAFETY: we are dropping the future that would give us a write lock,
                // so we don't need said lock anymore.
                unsafe {
                    lock.write_unlock();
                }
            }
        }
    }
}

impl<'a> EventListenerFuture for RawUpgrade<'a> {
    type Output = &'a RawRwLock;

    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<&'a RawRwLock> {
        let this = self.project();
        let lock = this.lock.expect("cannot poll future after completion");

        // If there are readers, we need to wait for them to finish.
        loop {
            let load_ordering = if this.listener.is_some() {
                Ordering::Acquire
            } else {
                Ordering::SeqCst
            };

            // See if the number of readers is zero.
            let state = lock.state.load(load_ordering);
            if state == WRITER_BIT {
                break;
            }

            // If there are readers, wait for them to finish.
            if this.listener.is_none() {
                // Start listening for "no readers" events.
                *this.listener = Some(lock.no_readers.listen());
            } else {
                // Wait for the readers to finish.
                ready!(strategy.poll(this.listener, cx));
            };
        }

        // We are done.
        Poll::Ready(this.lock.take().unwrap())
    }
}

impl RawUpgrade<'_> {
    /// Whether the future returned `Poll::Ready(..)` at some point.
    #[inline]
    pub(super) fn is_ready(&self) -> bool {
        self.lock.is_none()
    }
}
