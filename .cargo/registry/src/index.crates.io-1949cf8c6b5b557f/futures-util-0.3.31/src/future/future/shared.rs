use crate::task::{waker_ref, ArcWake};
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll, Waker};
use slab::Slab;
use std::cell::UnsafeCell;
use std::fmt;
use std::hash::Hasher;
use std::pin::Pin;
use std::ptr;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{Acquire, SeqCst};
use std::sync::{Arc, Mutex, Weak};

/// Future for the [`shared`](super::FutureExt::shared) method.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Shared<Fut: Future> {
    inner: Option<Arc<Inner<Fut>>>,
    waker_key: usize,
}

struct Inner<Fut: Future> {
    future_or_output: UnsafeCell<FutureOrOutput<Fut>>,
    notifier: Arc<Notifier>,
}

struct Notifier {
    state: AtomicUsize,
    wakers: Mutex<Option<Slab<Option<Waker>>>>,
}

/// A weak reference to a [`Shared`] that can be upgraded much like an `Arc`.
pub struct WeakShared<Fut: Future>(Weak<Inner<Fut>>);

impl<Fut: Future> Clone for WeakShared<Fut> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Fut: Future> fmt::Debug for Shared<Fut> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shared")
            .field("inner", &self.inner)
            .field("waker_key", &self.waker_key)
            .finish()
    }
}

impl<Fut: Future> fmt::Debug for Inner<Fut> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inner").finish()
    }
}

impl<Fut: Future> fmt::Debug for WeakShared<Fut> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WeakShared").finish()
    }
}

enum FutureOrOutput<Fut: Future> {
    Future(Fut),
    Output(Fut::Output),
}

unsafe impl<Fut> Send for Inner<Fut>
where
    Fut: Future + Send,
    Fut::Output: Send + Sync,
{
}

unsafe impl<Fut> Sync for Inner<Fut>
where
    Fut: Future + Send,
    Fut::Output: Send + Sync,
{
}

const IDLE: usize = 0;
const POLLING: usize = 1;
const COMPLETE: usize = 2;
const POISONED: usize = 3;

const NULL_WAKER_KEY: usize = usize::MAX;

impl<Fut: Future> Shared<Fut> {
    pub(super) fn new(future: Fut) -> Self {
        let inner = Inner {
            future_or_output: UnsafeCell::new(FutureOrOutput::Future(future)),
            notifier: Arc::new(Notifier {
                state: AtomicUsize::new(IDLE),
                wakers: Mutex::new(Some(Slab::new())),
            }),
        };

        Self { inner: Some(Arc::new(inner)), waker_key: NULL_WAKER_KEY }
    }
}

impl<Fut> Shared<Fut>
where
    Fut: Future,
{
    /// Returns [`Some`] containing a reference to this [`Shared`]'s output if
    /// it has already been computed by a clone or [`None`] if it hasn't been
    /// computed yet or this [`Shared`] already returned its output from
    /// [`poll`](Future::poll).
    pub fn peek(&self) -> Option<&Fut::Output> {
        if let Some(inner) = self.inner.as_ref() {
            match inner.notifier.state.load(SeqCst) {
                COMPLETE => unsafe { return Some(inner.output()) },
                POISONED => panic!("inner future panicked during poll"),
                _ => {}
            }
        }
        None
    }

    /// Creates a new [`WeakShared`] for this [`Shared`].
    ///
    /// Returns [`None`] if it has already been polled to completion.
    pub fn downgrade(&self) -> Option<WeakShared<Fut>> {
        if let Some(inner) = self.inner.as_ref() {
            return Some(WeakShared(Arc::downgrade(inner)));
        }
        None
    }

    /// Gets the number of strong pointers to this allocation.
    ///
    /// Returns [`None`] if it has already been polled to completion.
    ///
    /// # Safety
    ///
    /// This method by itself is safe, but using it correctly requires extra care. Another thread
    /// can change the strong count at any time, including potentially between calling this method
    /// and acting on the result.
    #[allow(clippy::unnecessary_safety_doc)]
    pub fn strong_count(&self) -> Option<usize> {
        self.inner.as_ref().map(|arc| Arc::strong_count(arc))
    }

    /// Gets the number of weak pointers to this allocation.
    ///
    /// Returns [`None`] if it has already been polled to completion.
    ///
    /// # Safety
    ///
    /// This method by itself is safe, but using it correctly requires extra care. Another thread
    /// can change the weak count at any time, including potentially between calling this method
    /// and acting on the result.
    #[allow(clippy::unnecessary_safety_doc)]
    pub fn weak_count(&self) -> Option<usize> {
        self.inner.as_ref().map(|arc| Arc::weak_count(arc))
    }

    /// Hashes the internal state of this `Shared` in a way that's compatible with `ptr_eq`.
    pub fn ptr_hash<H: Hasher>(&self, state: &mut H) {
        match self.inner.as_ref() {
            Some(arc) => {
                state.write_u8(1);
                ptr::hash(Arc::as_ptr(arc), state);
            }
            None => {
                state.write_u8(0);
            }
        }
    }

    /// Returns `true` if the two `Shared`s point to the same future (in a vein similar to
    /// `Arc::ptr_eq`).
    ///
    /// Returns `false` if either `Shared` has terminated.
    pub fn ptr_eq(&self, rhs: &Self) -> bool {
        let lhs = match self.inner.as_ref() {
            Some(lhs) => lhs,
            None => return false,
        };
        let rhs = match rhs.inner.as_ref() {
            Some(rhs) => rhs,
            None => return false,
        };
        Arc::ptr_eq(lhs, rhs)
    }
}

impl<Fut> Inner<Fut>
where
    Fut: Future,
{
    /// Safety: callers must first ensure that `self.inner.state`
    /// is `COMPLETE`
    unsafe fn output(&self) -> &Fut::Output {
        match unsafe { &*self.future_or_output.get() } {
            FutureOrOutput::Output(item) => item,
            FutureOrOutput::Future(_) => unreachable!(),
        }
    }
}

impl<Fut> Inner<Fut>
where
    Fut: Future,
    Fut::Output: Clone,
{
    /// Registers the current task to receive a wakeup when we are awoken.
    fn record_waker(&self, waker_key: &mut usize, cx: &mut Context<'_>) {
        let mut wakers_guard = self.notifier.wakers.lock().unwrap();

        let wakers = match wakers_guard.as_mut() {
            Some(wakers) => wakers,
            None => return,
        };

        let new_waker = cx.waker();

        if *waker_key == NULL_WAKER_KEY {
            *waker_key = wakers.insert(Some(new_waker.clone()));
        } else {
            match wakers[*waker_key] {
                Some(ref old_waker) if new_waker.will_wake(old_waker) => {}
                // Could use clone_from here, but Waker doesn't specialize it.
                ref mut slot => *slot = Some(new_waker.clone()),
            }
        }
        debug_assert!(*waker_key != NULL_WAKER_KEY);
    }

    /// Safety: callers must first ensure that `inner.state`
    /// is `COMPLETE`
    unsafe fn take_or_clone_output(self: Arc<Self>) -> Fut::Output {
        match Arc::try_unwrap(self) {
            Ok(inner) => match inner.future_or_output.into_inner() {
                FutureOrOutput::Output(item) => item,
                FutureOrOutput::Future(_) => unreachable!(),
            },
            Err(inner) => unsafe { inner.output().clone() },
        }
    }
}

impl<Fut> FusedFuture for Shared<Fut>
where
    Fut: Future,
    Fut::Output: Clone,
{
    fn is_terminated(&self) -> bool {
        self.inner.is_none()
    }
}

impl<Fut> Future for Shared<Fut>
where
    Fut: Future,
    Fut::Output: Clone,
{
    type Output = Fut::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;

        let inner = this.inner.take().expect("Shared future polled again after completion");

        // Fast path for when the wrapped future has already completed
        if inner.notifier.state.load(Acquire) == COMPLETE {
            // Safety: We're in the COMPLETE state
            return unsafe { Poll::Ready(inner.take_or_clone_output()) };
        }

        inner.record_waker(&mut this.waker_key, cx);

        match inner
            .notifier
            .state
            .compare_exchange(IDLE, POLLING, SeqCst, SeqCst)
            .unwrap_or_else(|x| x)
        {
            IDLE => {
                // Lock acquired, fall through
            }
            POLLING => {
                // Another task is currently polling, at this point we just want
                // to ensure that the waker for this task is registered
                this.inner = Some(inner);
                return Poll::Pending;
            }
            COMPLETE => {
                // Safety: We're in the COMPLETE state
                return unsafe { Poll::Ready(inner.take_or_clone_output()) };
            }
            POISONED => panic!("inner future panicked during poll"),
            _ => unreachable!(),
        }

        let waker = waker_ref(&inner.notifier);
        let mut cx = Context::from_waker(&waker);

        struct Reset<'a> {
            state: &'a AtomicUsize,
            did_not_panic: bool,
        }

        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                if !self.did_not_panic {
                    self.state.store(POISONED, SeqCst);
                }
            }
        }

        let mut reset = Reset { state: &inner.notifier.state, did_not_panic: false };

        let output = {
            let future = unsafe {
                match &mut *inner.future_or_output.get() {
                    FutureOrOutput::Future(fut) => Pin::new_unchecked(fut),
                    _ => unreachable!(),
                }
            };

            let poll_result = future.poll(&mut cx);
            reset.did_not_panic = true;

            match poll_result {
                Poll::Pending => {
                    if inner.notifier.state.compare_exchange(POLLING, IDLE, SeqCst, SeqCst).is_ok()
                    {
                        // Success
                        drop(reset);
                        this.inner = Some(inner);
                        return Poll::Pending;
                    } else {
                        unreachable!()
                    }
                }
                Poll::Ready(output) => output,
            }
        };

        unsafe {
            *inner.future_or_output.get() = FutureOrOutput::Output(output);
        }

        inner.notifier.state.store(COMPLETE, SeqCst);

        // Wake all tasks and drop the slab
        let mut wakers_guard = inner.notifier.wakers.lock().unwrap();
        let mut wakers = wakers_guard.take().unwrap();
        for waker in wakers.drain().flatten() {
            waker.wake();
        }

        drop(reset); // Make borrow checker happy
        drop(wakers_guard);

        // Safety: We're in the COMPLETE state
        unsafe { Poll::Ready(inner.take_or_clone_output()) }
    }
}

impl<Fut> Clone for Shared<Fut>
where
    Fut: Future,
{
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone(), waker_key: NULL_WAKER_KEY }
    }
}

impl<Fut> Drop for Shared<Fut>
where
    Fut: Future,
{
    fn drop(&mut self) {
        if self.waker_key != NULL_WAKER_KEY {
            if let Some(ref inner) = self.inner {
                if let Ok(mut wakers) = inner.notifier.wakers.lock() {
                    if let Some(wakers) = wakers.as_mut() {
                        wakers.remove(self.waker_key);
                    }
                }
            }
        }
    }
}

impl ArcWake for Notifier {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let wakers = &mut *arc_self.wakers.lock().unwrap();
        if let Some(wakers) = wakers.as_mut() {
            for (_key, opt_waker) in wakers {
                if let Some(waker) = opt_waker.take() {
                    waker.wake();
                }
            }
        }
    }
}

impl<Fut: Future> WeakShared<Fut> {
    /// Attempts to upgrade this [`WeakShared`] into a [`Shared`].
    ///
    /// Returns [`None`] if all clones of the [`Shared`] have been dropped or polled
    /// to completion.
    pub fn upgrade(&self) -> Option<Shared<Fut>> {
        Some(Shared { inner: Some(self.0.upgrade()?), waker_key: NULL_WAKER_KEY })
    }
}
