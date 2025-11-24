#![cfg_attr(not(feature = "sync"), allow(unreachable_pub, dead_code))]
//! # Implementation Details.
//!
//! The semaphore is implemented using an intrusive linked list of waiters. An
//! atomic counter tracks the number of available permits. If the semaphore does
//! not contain the required number of permits, the task attempting to acquire
//! permits places its waker at the end of a queue. When new permits are made
//! available (such as by releasing an initial acquisition), they are assigned
//! to the task at the front of the queue, waking that task if its requested
//! number of permits is met.
//!
//! Because waiters are enqueued at the back of the linked list and dequeued
//! from the front, the semaphore is fair. Tasks trying to acquire large numbers
//! of permits at a time will always be woken eventually, even if many other
//! tasks are acquiring smaller numbers of permits. This means that in a
//! use-case like tokio's read-write lock, writers will not be starved by
//! readers.
use crate::loom::cell::UnsafeCell;
use crate::loom::sync::atomic::AtomicUsize;
use crate::loom::sync::{Mutex, MutexGuard};
use crate::util::linked_list::{self, LinkedList};
#[cfg(all(tokio_unstable, feature = "tracing"))]
use crate::util::trace;
use crate::util::WakeList;

use std::future::Future;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::Ordering::*;
use std::task::{ready, Context, Poll, Waker};
use std::{cmp, fmt};

/// An asynchronous counting semaphore which permits waiting on multiple permits at once.
pub(crate) struct Semaphore {
    waiters: Mutex<Waitlist>,
    /// The current number of available permits in the semaphore.
    permits: AtomicUsize,
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
}

struct Waitlist {
    queue: LinkedList<Waiter, <Waiter as linked_list::Link>::Target>,
    closed: bool,
}

/// Error returned from the [`Semaphore::try_acquire`] function.
///
/// [`Semaphore::try_acquire`]: crate::sync::Semaphore::try_acquire
#[derive(Debug, PartialEq, Eq)]
pub enum TryAcquireError {
    /// The semaphore has been [closed] and cannot issue new permits.
    ///
    /// [closed]: crate::sync::Semaphore::close
    Closed,

    /// The semaphore has no available permits.
    NoPermits,
}
/// Error returned from the [`Semaphore::acquire`] function.
///
/// An `acquire` operation can only fail if the semaphore has been
/// [closed].
///
/// [closed]: crate::sync::Semaphore::close
/// [`Semaphore::acquire`]: crate::sync::Semaphore::acquire
#[derive(Debug)]
pub struct AcquireError(());

pub(crate) struct Acquire<'a> {
    node: Waiter,
    semaphore: &'a Semaphore,
    num_permits: usize,
    queued: bool,
}

/// An entry in the wait queue.
struct Waiter {
    /// The current state of the waiter.
    ///
    /// This is either the number of remaining permits required by
    /// the waiter, or a flag indicating that the waiter is not yet queued.
    state: AtomicUsize,

    /// The waker to notify the task awaiting permits.
    ///
    /// # Safety
    ///
    /// This may only be accessed while the wait queue is locked.
    waker: UnsafeCell<Option<Waker>>,

    /// Intrusive linked-list pointers.
    ///
    /// # Safety
    ///
    /// This may only be accessed while the wait queue is locked.
    ///
    /// TODO: Ideally, we would be able to use loom to enforce that
    /// this isn't accessed concurrently. However, it is difficult to
    /// use a `UnsafeCell` here, since the `Link` trait requires _returning_
    /// references to `Pointers`, and `UnsafeCell` requires that checked access
    /// take place inside a closure. We should consider changing `Pointers` to
    /// use `UnsafeCell` internally.
    pointers: linked_list::Pointers<Waiter>,

    #[cfg(all(tokio_unstable, feature = "tracing"))]
    ctx: trace::AsyncOpTracingCtx,

    /// Should not be `Unpin`.
    _p: PhantomPinned,
}

generate_addr_of_methods! {
    impl<> Waiter {
        unsafe fn addr_of_pointers(self: NonNull<Self>) -> NonNull<linked_list::Pointers<Waiter>> {
            &self.pointers
        }
    }
}

impl Semaphore {
    /// The maximum number of permits which a semaphore can hold.
    ///
    /// Note that this reserves three bits of flags in the permit counter, but
    /// we only actually use one of them. However, the previous semaphore
    /// implementation used three bits, so we will continue to reserve them to
    /// avoid a breaking change if additional flags need to be added in the
    /// future.
    pub(crate) const MAX_PERMITS: usize = usize::MAX >> 3;
    const CLOSED: usize = 1;
    // The least-significant bit in the number of permits is reserved to use
    // as a flag indicating that the semaphore has been closed. Consequently
    // PERMIT_SHIFT is used to leave that bit for that purpose.
    const PERMIT_SHIFT: usize = 1;

    /// Creates a new semaphore with the initial number of permits
    ///
    /// Maximum number of permits on 32-bit platforms is `1<<29`.
    pub(crate) fn new(permits: usize) -> Self {
        assert!(
            permits <= Self::MAX_PERMITS,
            "a semaphore may not have more than MAX_PERMITS permits ({})",
            Self::MAX_PERMITS
        );

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let resource_span = {
            let resource_span = tracing::trace_span!(
                parent: None,
                "runtime.resource",
                concrete_type = "Semaphore",
                kind = "Sync",
                is_internal = true
            );

            resource_span.in_scope(|| {
                tracing::trace!(
                    target: "runtime::resource::state_update",
                    permits = permits,
                    permits.op = "override",
                )
            });
            resource_span
        };

        Self {
            permits: AtomicUsize::new(permits << Self::PERMIT_SHIFT),
            waiters: Mutex::new(Waitlist {
                queue: LinkedList::new(),
                closed: false,
            }),
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span,
        }
    }

    /// Creates a new semaphore with the initial number of permits.
    ///
    /// Maximum number of permits on 32-bit platforms is `1<<29`.
    #[cfg(not(all(loom, test)))]
    pub(crate) const fn const_new(permits: usize) -> Self {
        assert!(permits <= Self::MAX_PERMITS);

        Self {
            permits: AtomicUsize::new(permits << Self::PERMIT_SHIFT),
            waiters: Mutex::const_new(Waitlist {
                queue: LinkedList::new(),
                closed: false,
            }),
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: tracing::Span::none(),
        }
    }

    /// Creates a new closed semaphore with 0 permits.
    pub(crate) fn new_closed() -> Self {
        Self {
            permits: AtomicUsize::new(Self::CLOSED),
            waiters: Mutex::new(Waitlist {
                queue: LinkedList::new(),
                closed: true,
            }),
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: tracing::Span::none(),
        }
    }

    /// Creates a new closed semaphore with 0 permits.
    #[cfg(not(all(loom, test)))]
    pub(crate) const fn const_new_closed() -> Self {
        Self {
            permits: AtomicUsize::new(Self::CLOSED),
            waiters: Mutex::const_new(Waitlist {
                queue: LinkedList::new(),
                closed: true,
            }),
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: tracing::Span::none(),
        }
    }

    /// Returns the current number of available permits.
    pub(crate) fn available_permits(&self) -> usize {
        self.permits.load(Acquire) >> Self::PERMIT_SHIFT
    }

    /// Adds `added` new permits to the semaphore.
    ///
    /// The maximum number of permits is `usize::MAX >> 3`, and this function will panic if the limit is exceeded.
    pub(crate) fn release(&self, added: usize) {
        if added == 0 {
            return;
        }

        // Assign permits to the wait queue
        self.add_permits_locked(added, self.waiters.lock());
    }

    /// Closes the semaphore. This prevents the semaphore from issuing new
    /// permits and notifies all pending waiters.
    pub(crate) fn close(&self) {
        let mut waiters = self.waiters.lock();
        // If the semaphore's permits counter has enough permits for an
        // unqueued waiter to acquire all the permits it needs immediately,
        // it won't touch the wait list. Therefore, we have to set a bit on
        // the permit counter as well. However, we must do this while
        // holding the lock --- otherwise, if we set the bit and then wait
        // to acquire the lock we'll enter an inconsistent state where the
        // permit counter is closed, but the wait list is not.
        self.permits.fetch_or(Self::CLOSED, Release);
        waiters.closed = true;
        while let Some(mut waiter) = waiters.queue.pop_back() {
            let waker = unsafe { waiter.as_mut().waker.with_mut(|waker| (*waker).take()) };
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }

    /// Returns true if the semaphore is closed.
    pub(crate) fn is_closed(&self) -> bool {
        self.permits.load(Acquire) & Self::CLOSED == Self::CLOSED
    }

    pub(crate) fn try_acquire(&self, num_permits: usize) -> Result<(), TryAcquireError> {
        assert!(
            num_permits <= Self::MAX_PERMITS,
            "a semaphore may not have more than MAX_PERMITS permits ({})",
            Self::MAX_PERMITS
        );
        let num_permits = num_permits << Self::PERMIT_SHIFT;
        let mut curr = self.permits.load(Acquire);
        loop {
            // Has the semaphore closed?
            if curr & Self::CLOSED == Self::CLOSED {
                return Err(TryAcquireError::Closed);
            }

            // Are there enough permits remaining?
            if curr < num_permits {
                return Err(TryAcquireError::NoPermits);
            }

            let next = curr - num_permits;

            match self.permits.compare_exchange(curr, next, AcqRel, Acquire) {
                Ok(_) => {
                    // TODO: Instrument once issue has been solved
                    return Ok(());
                }
                Err(actual) => curr = actual,
            }
        }
    }

    pub(crate) fn acquire(&self, num_permits: usize) -> Acquire<'_> {
        Acquire::new(self, num_permits)
    }

    /// Release `rem` permits to the semaphore's wait list, starting from the
    /// end of the queue.
    ///
    /// If `rem` exceeds the number of permits needed by the wait list, the
    /// remainder are assigned back to the semaphore.
    fn add_permits_locked(&self, mut rem: usize, waiters: MutexGuard<'_, Waitlist>) {
        let mut wakers = WakeList::new();
        let mut lock = Some(waiters);
        let mut is_empty = false;
        while rem > 0 {
            let mut waiters = lock.take().unwrap_or_else(|| self.waiters.lock());
            'inner: while wakers.can_push() {
                // Was the waiter assigned enough permits to wake it?
                match waiters.queue.last() {
                    Some(waiter) => {
                        if !waiter.assign_permits(&mut rem) {
                            break 'inner;
                        }
                    }
                    None => {
                        is_empty = true;
                        // If we assigned permits to all the waiters in the queue, and there are
                        // still permits left over, assign them back to the semaphore.
                        break 'inner;
                    }
                };
                let mut waiter = waiters.queue.pop_back().unwrap();
                if let Some(waker) =
                    unsafe { waiter.as_mut().waker.with_mut(|waker| (*waker).take()) }
                {
                    wakers.push(waker);
                }
            }

            if rem > 0 && is_empty {
                let permits = rem;
                assert!(
                    permits <= Self::MAX_PERMITS,
                    "cannot add more than MAX_PERMITS permits ({})",
                    Self::MAX_PERMITS
                );
                let prev = self.permits.fetch_add(rem << Self::PERMIT_SHIFT, Release);
                let prev = prev >> Self::PERMIT_SHIFT;
                assert!(
                    prev + permits <= Self::MAX_PERMITS,
                    "number of added permits ({}) would overflow MAX_PERMITS ({})",
                    rem,
                    Self::MAX_PERMITS
                );

                // add remaining permits back
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                self.resource_span.in_scope(|| {
                    tracing::trace!(
                    target: "runtime::resource::state_update",
                    permits = rem,
                    permits.op = "add",
                    )
                });

                rem = 0;
            }

            drop(waiters); // release the lock

            wakers.wake_all();
        }

        assert_eq!(rem, 0);
    }

    /// Decrease a semaphore's permits by a maximum of `n`.
    ///
    /// If there are insufficient permits and it's not possible to reduce by `n`,
    /// return the number of permits that were actually reduced.
    pub(crate) fn forget_permits(&self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }

        let mut curr_bits = self.permits.load(Acquire);
        loop {
            let curr = curr_bits >> Self::PERMIT_SHIFT;
            let new = curr.saturating_sub(n);
            match self.permits.compare_exchange_weak(
                curr_bits,
                new << Self::PERMIT_SHIFT,
                AcqRel,
                Acquire,
            ) {
                Ok(_) => return std::cmp::min(curr, n),
                Err(actual) => curr_bits = actual,
            };
        }
    }

    fn poll_acquire(
        &self,
        cx: &mut Context<'_>,
        num_permits: usize,
        node: Pin<&mut Waiter>,
        queued: bool,
    ) -> Poll<Result<(), AcquireError>> {
        let mut acquired = 0;

        let needed = if queued {
            node.state.load(Acquire) << Self::PERMIT_SHIFT
        } else {
            num_permits << Self::PERMIT_SHIFT
        };

        let mut lock = None;
        // First, try to take the requested number of permits from the
        // semaphore.
        let mut curr = self.permits.load(Acquire);
        let mut waiters = loop {
            // Has the semaphore closed?
            if curr & Self::CLOSED > 0 {
                return Poll::Ready(Err(AcquireError::closed()));
            }

            let mut remaining = 0;
            let total = curr
                .checked_add(acquired)
                .expect("number of permits must not overflow");
            let (next, acq) = if total >= needed {
                let next = curr - (needed - acquired);
                (next, needed >> Self::PERMIT_SHIFT)
            } else {
                remaining = (needed - acquired) - curr;
                (0, curr >> Self::PERMIT_SHIFT)
            };

            if remaining > 0 && lock.is_none() {
                // No permits were immediately available, so this permit will
                // (probably) need to wait. We'll need to acquire a lock on the
                // wait queue before continuing. We need to do this _before_ the
                // CAS that sets the new value of the semaphore's `permits`
                // counter. Otherwise, if we subtract the permits and then
                // acquire the lock, we might miss additional permits being
                // added while waiting for the lock.
                lock = Some(self.waiters.lock());
            }

            match self.permits.compare_exchange(curr, next, AcqRel, Acquire) {
                Ok(_) => {
                    acquired += acq;
                    if remaining == 0 {
                        if !queued {
                            #[cfg(all(tokio_unstable, feature = "tracing"))]
                            self.resource_span.in_scope(|| {
                                tracing::trace!(
                                    target: "runtime::resource::state_update",
                                    permits = acquired,
                                    permits.op = "sub",
                                );
                                tracing::trace!(
                                    target: "runtime::resource::async_op::state_update",
                                    permits_obtained = acquired,
                                    permits.op = "add",
                                )
                            });

                            return Poll::Ready(Ok(()));
                        } else if lock.is_none() {
                            break self.waiters.lock();
                        }
                    }
                    break lock.expect("lock must be acquired before waiting");
                }
                Err(actual) => curr = actual,
            }
        };

        if waiters.closed {
            return Poll::Ready(Err(AcquireError::closed()));
        }

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        self.resource_span.in_scope(|| {
            tracing::trace!(
                target: "runtime::resource::state_update",
                permits = acquired,
                permits.op = "sub",
            )
        });

        if node.assign_permits(&mut acquired) {
            self.add_permits_locked(acquired, waiters);
            return Poll::Ready(Ok(()));
        }

        assert_eq!(acquired, 0);
        let mut old_waker = None;

        // Otherwise, register the waker & enqueue the node.
        node.waker.with_mut(|waker| {
            // Safety: the wait list is locked, so we may modify the waker.
            let waker = unsafe { &mut *waker };
            // Do we need to register the new waker?
            if waker
                .as_ref()
                .map_or(true, |waker| !waker.will_wake(cx.waker()))
            {
                old_waker = waker.replace(cx.waker().clone());
            }
        });

        // If the waiter is not already in the wait queue, enqueue it.
        if !queued {
            let node = unsafe {
                let node = Pin::into_inner_unchecked(node) as *mut _;
                NonNull::new_unchecked(node)
            };

            waiters.queue.push_front(node);
        }
        drop(waiters);
        drop(old_waker);

        Poll::Pending
    }
}

impl fmt::Debug for Semaphore {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Semaphore")
            .field("permits", &self.available_permits())
            .finish()
    }
}

impl Waiter {
    fn new(
        num_permits: usize,
        #[cfg(all(tokio_unstable, feature = "tracing"))] ctx: trace::AsyncOpTracingCtx,
    ) -> Self {
        Waiter {
            waker: UnsafeCell::new(None),
            state: AtomicUsize::new(num_permits),
            pointers: linked_list::Pointers::new(),
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            ctx,
            _p: PhantomPinned,
        }
    }

    /// Assign permits to the waiter.
    ///
    /// Returns `true` if the waiter should be removed from the queue
    fn assign_permits(&self, n: &mut usize) -> bool {
        let mut curr = self.state.load(Acquire);
        loop {
            let assign = cmp::min(curr, *n);
            let next = curr - assign;
            match self.state.compare_exchange(curr, next, AcqRel, Acquire) {
                Ok(_) => {
                    *n -= assign;
                    #[cfg(all(tokio_unstable, feature = "tracing"))]
                    self.ctx.async_op_span.in_scope(|| {
                        tracing::trace!(
                            target: "runtime::resource::async_op::state_update",
                            permits_obtained = assign,
                            permits.op = "add",
                        );
                    });
                    return next == 0;
                }
                Err(actual) => curr = actual,
            }
        }
    }
}

impl Future for Acquire<'_> {
    type Output = Result<(), AcquireError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        ready!(crate::trace::trace_leaf(cx));

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let _resource_span = self.node.ctx.resource_span.clone().entered();
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let _async_op_span = self.node.ctx.async_op_span.clone().entered();
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let _async_op_poll_span = self.node.ctx.async_op_poll_span.clone().entered();

        let (node, semaphore, needed, queued) = self.project();

        // First, ensure the current task has enough budget to proceed.
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let coop = ready!(trace_poll_op!(
            "poll_acquire",
            crate::task::coop::poll_proceed(cx),
        ));

        #[cfg(not(all(tokio_unstable, feature = "tracing")))]
        let coop = ready!(crate::task::coop::poll_proceed(cx));

        let result = match semaphore.poll_acquire(cx, needed, node, *queued) {
            Poll::Pending => {
                *queued = true;
                Poll::Pending
            }
            Poll::Ready(r) => {
                coop.made_progress();
                r?;
                *queued = false;
                Poll::Ready(Ok(()))
            }
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        return trace_poll_op!("poll_acquire", result);

        #[cfg(not(all(tokio_unstable, feature = "tracing")))]
        return result;
    }
}

impl<'a> Acquire<'a> {
    fn new(semaphore: &'a Semaphore, num_permits: usize) -> Self {
        #[cfg(any(not(tokio_unstable), not(feature = "tracing")))]
        return Self {
            node: Waiter::new(num_permits),
            semaphore,
            num_permits,
            queued: false,
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        return semaphore.resource_span.in_scope(|| {
            let async_op_span =
                tracing::trace_span!("runtime.resource.async_op", source = "Acquire::new");
            let async_op_poll_span = async_op_span.in_scope(|| {
                tracing::trace!(
                    target: "runtime::resource::async_op::state_update",
                    permits_requested = num_permits,
                    permits.op = "override",
                );

                tracing::trace!(
                    target: "runtime::resource::async_op::state_update",
                    permits_obtained = 0usize,
                    permits.op = "override",
                );

                tracing::trace_span!("runtime.resource.async_op.poll")
            });

            let ctx = trace::AsyncOpTracingCtx {
                async_op_span,
                async_op_poll_span,
                resource_span: semaphore.resource_span.clone(),
            };

            Self {
                node: Waiter::new(num_permits, ctx),
                semaphore,
                num_permits,
                queued: false,
            }
        });
    }

    fn project(self: Pin<&mut Self>) -> (Pin<&mut Waiter>, &Semaphore, usize, &mut bool) {
        fn is_unpin<T: Unpin>() {}
        unsafe {
            // Safety: all fields other than `node` are `Unpin`

            is_unpin::<&Semaphore>();
            is_unpin::<&mut bool>();
            is_unpin::<usize>();

            let this = self.get_unchecked_mut();
            (
                Pin::new_unchecked(&mut this.node),
                this.semaphore,
                this.num_permits,
                &mut this.queued,
            )
        }
    }
}

impl Drop for Acquire<'_> {
    fn drop(&mut self) {
        // If the future is completed, there is no node in the wait list, so we
        // can skip acquiring the lock.
        if !self.queued {
            return;
        }

        // This is where we ensure safety. The future is being dropped,
        // which means we must ensure that the waiter entry is no longer stored
        // in the linked list.
        let mut waiters = self.semaphore.waiters.lock();

        // remove the entry from the list
        let node = NonNull::from(&mut self.node);
        // Safety: we have locked the wait list.
        unsafe { waiters.queue.remove(node) };

        let acquired_permits = self.num_permits - self.node.state.load(Acquire);
        if acquired_permits > 0 {
            self.semaphore.add_permits_locked(acquired_permits, waiters);
        }
    }
}

// Safety: the `Acquire` future is not `Sync` automatically because it contains
// a `Waiter`, which, in turn, contains an `UnsafeCell`. However, the
// `UnsafeCell` is only accessed when the future is borrowed mutably (either in
// `poll` or in `drop`). Therefore, it is safe (although not particularly
// _useful_) for the future to be borrowed immutably across threads.
unsafe impl Sync for Acquire<'_> {}

// ===== impl AcquireError ====

impl AcquireError {
    fn closed() -> AcquireError {
        AcquireError(())
    }
}

impl fmt::Display for AcquireError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "semaphore closed")
    }
}

impl std::error::Error for AcquireError {}

// ===== impl TryAcquireError =====

impl TryAcquireError {
    /// Returns `true` if the error was caused by a closed semaphore.
    #[allow(dead_code)] // may be used later!
    pub(crate) fn is_closed(&self) -> bool {
        matches!(self, TryAcquireError::Closed)
    }

    /// Returns `true` if the error was caused by calling `try_acquire` on a
    /// semaphore with no available permits.
    #[allow(dead_code)] // may be used later!
    pub(crate) fn is_no_permits(&self) -> bool {
        matches!(self, TryAcquireError::NoPermits)
    }
}

impl fmt::Display for TryAcquireError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TryAcquireError::Closed => write!(fmt, "semaphore closed"),
            TryAcquireError::NoPermits => write!(fmt, "no permits available"),
        }
    }
}

impl std::error::Error for TryAcquireError {}

/// # Safety
///
/// `Waiter` is forced to be !Unpin.
unsafe impl linked_list::Link for Waiter {
    type Handle = NonNull<Waiter>;
    type Target = Waiter;

    fn as_raw(handle: &Self::Handle) -> NonNull<Waiter> {
        *handle
    }

    unsafe fn from_raw(ptr: NonNull<Waiter>) -> NonNull<Waiter> {
        ptr
    }

    unsafe fn pointers(target: NonNull<Waiter>) -> NonNull<linked_list::Pointers<Waiter>> {
        Waiter::addr_of_pointers(target)
    }
}
