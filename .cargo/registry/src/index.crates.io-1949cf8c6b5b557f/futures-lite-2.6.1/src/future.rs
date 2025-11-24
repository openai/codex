//! Combinators for the [`Future`] trait.
//!
//! # Examples
//!
//! ```
//! use futures_lite::future;
//!
//! # spin_on::spin_on(async {
//! for step in 0..3 {
//!     println!("step {}", step);
//!
//!     // Give other tasks a chance to run.
//!     future::yield_now().await;
//! }
//! # });
//! ```

#[doc(no_inline)]
pub use core::future::{pending, ready, Future, Pending, Ready};

use core::fmt;
use core::pin::Pin;
use core::task::{Context, Poll};

#[cfg(feature = "alloc")]
use alloc::boxed::Box;

#[cfg(feature = "std")]
use std::{
    any::Any,
    panic::{catch_unwind, AssertUnwindSafe, UnwindSafe},
    thread_local,
};

#[cfg(feature = "race")]
use fastrand::Rng;
use pin_project_lite::pin_project;

/// Blocks the current thread on a future.
///
/// # Examples
///
/// ```
/// use futures_lite::future;
///
/// let val = future::block_on(async {
///     1 + 2
/// });
///
/// assert_eq!(val, 3);
/// ```
#[cfg(feature = "std")]
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    use core::cell::RefCell;
    use core::task::Waker;

    use parking::Parker;

    // Pin the future on the stack.
    crate::pin!(future);

    // Creates a parker and an associated waker that unparks it.
    fn parker_and_waker() -> (Parker, Waker) {
        let parker = Parker::new();
        let unparker = parker.unparker();
        let waker = Waker::from(unparker);
        (parker, waker)
    }

    thread_local! {
        // Cached parker and waker for efficiency.
        static CACHE: RefCell<(Parker, Waker)> = RefCell::new(parker_and_waker());
    }

    CACHE.with(|cache| {
        // Try grabbing the cached parker and waker.
        let tmp_cached;
        let tmp_fresh;
        let (parker, waker) = match cache.try_borrow_mut() {
            Ok(cache) => {
                // Use the cached parker and waker.
                tmp_cached = cache;
                &*tmp_cached
            }
            Err(_) => {
                // Looks like this is a recursive `block_on()` call.
                // Create a fresh parker and waker.
                tmp_fresh = parker_and_waker();
                &tmp_fresh
            }
        };

        let cx = &mut Context::from_waker(waker);
        // Keep polling until the future is ready.
        loop {
            match future.as_mut().poll(cx) {
                Poll::Ready(output) => return output,
                Poll::Pending => parker.park(),
            }
        }
    })
}

/// Polls a future just once and returns an [`Option`] with the result.
///
/// # Examples
///
/// ```
/// use futures_lite::future;
///
/// # spin_on::spin_on(async {
/// assert_eq!(future::poll_once(future::pending::<()>()).await, None);
/// assert_eq!(future::poll_once(future::ready(42)).await, Some(42));
/// # })
/// ```
pub fn poll_once<T, F>(f: F) -> PollOnce<F>
where
    F: Future<Output = T>,
{
    PollOnce { f }
}

pin_project! {
    /// Future for the [`poll_once()`] function.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct PollOnce<F> {
        #[pin]
        f: F,
    }
}

impl<F> fmt::Debug for PollOnce<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollOnce").finish()
    }
}

impl<T, F> Future for PollOnce<F>
where
    F: Future<Output = T>,
{
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().f.poll(cx) {
            Poll::Ready(t) => Poll::Ready(Some(t)),
            Poll::Pending => Poll::Ready(None),
        }
    }
}

/// Creates a future from a function returning [`Poll`].
///
/// # Examples
///
/// ```
/// use futures_lite::future;
/// use std::task::{Context, Poll};
///
/// # spin_on::spin_on(async {
/// fn f(_: &mut Context<'_>) -> Poll<i32> {
///     Poll::Ready(7)
/// }
///
/// assert_eq!(future::poll_fn(f).await, 7);
/// # })
/// ```
pub fn poll_fn<T, F>(f: F) -> PollFn<F>
where
    F: FnMut(&mut Context<'_>) -> Poll<T>,
{
    PollFn { f }
}

pin_project! {
    /// Future for the [`poll_fn()`] function.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct PollFn<F> {
        f: F,
    }
}

impl<F> fmt::Debug for PollFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollFn").finish()
    }
}

impl<T, F> Future for PollFn<F>
where
    F: FnMut(&mut Context<'_>) -> Poll<T>,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let this = self.project();
        (this.f)(cx)
    }
}

/// Wakes the current task and returns [`Poll::Pending`] once.
///
/// This function is useful when we want to cooperatively give time to the task scheduler. It is
/// generally a good idea to yield inside loops because that way we make sure long-running tasks
/// don't prevent other tasks from running.
///
/// # Examples
///
/// ```
/// use futures_lite::future;
///
/// # spin_on::spin_on(async {
/// future::yield_now().await;
/// # })
/// ```
pub fn yield_now() -> YieldNow {
    YieldNow(false)
}

/// Future for the [`yield_now()`] function.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct YieldNow(bool);

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.0 {
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

/// Joins two futures, waiting for both to complete.
///
/// # Examples
///
/// ```
/// use futures_lite::future;
///
/// # spin_on::spin_on(async {
/// let a = async { 1 };
/// let b = async { 2 };
///
/// assert_eq!(future::zip(a, b).await, (1, 2));
/// # })
/// ```
pub fn zip<F1, F2>(future1: F1, future2: F2) -> Zip<F1, F2>
where
    F1: Future,
    F2: Future,
{
    Zip {
        future1: Some(future1),
        future2: Some(future2),
        output1: None,
        output2: None,
    }
}

pin_project! {
    /// Future for the [`zip()`] function.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Zip<F1, F2>
    where
        F1: Future,
        F2: Future,
    {
        #[pin]
        future1: Option<F1>,
        output1: Option<F1::Output>,
        #[pin]
        future2: Option<F2>,
        output2: Option<F2::Output>,
    }
}

/// Extracts the contents of two options and zips them, handling `(Some(_), None)` cases
fn take_zip_from_parts<T1, T2>(o1: &mut Option<T1>, o2: &mut Option<T2>) -> Poll<(T1, T2)> {
    match (o1.take(), o2.take()) {
        (Some(t1), Some(t2)) => Poll::Ready((t1, t2)),
        (o1x, o2x) => {
            *o1 = o1x;
            *o2 = o2x;
            Poll::Pending
        }
    }
}

impl<F1, F2> Future for Zip<F1, F2>
where
    F1: Future,
    F2: Future,
{
    type Output = (F1::Output, F2::Output);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if let Some(future) = this.future1.as_mut().as_pin_mut() {
            if let Poll::Ready(out) = future.poll(cx) {
                *this.output1 = Some(out);
                this.future1.set(None);
            }
        }

        if let Some(future) = this.future2.as_mut().as_pin_mut() {
            if let Poll::Ready(out) = future.poll(cx) {
                *this.output2 = Some(out);
                this.future2.set(None);
            }
        }

        take_zip_from_parts(this.output1, this.output2)
    }
}

/// Joins two fallible futures, waiting for both to complete or one of them to error.
///
/// # Examples
///
/// ```
/// use futures_lite::future;
///
/// # spin_on::spin_on(async {
/// let a = async { Ok::<i32, i32>(1) };
/// let b = async { Err::<i32, i32>(2) };
///
/// assert_eq!(future::try_zip(a, b).await, Err(2));
/// # })
/// ```
pub fn try_zip<T1, T2, E, F1, F2>(future1: F1, future2: F2) -> TryZip<F1, T1, F2, T2>
where
    F1: Future<Output = Result<T1, E>>,
    F2: Future<Output = Result<T2, E>>,
{
    TryZip {
        future1: Some(future1),
        future2: Some(future2),
        output1: None,
        output2: None,
    }
}

pin_project! {
    /// Future for the [`try_zip()`] function.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TryZip<F1, T1, F2, T2> {
        #[pin]
        future1: Option<F1>,
        output1: Option<T1>,
        #[pin]
        future2: Option<F2>,
        output2: Option<T2>,
    }
}

impl<T1, T2, E, F1, F2> Future for TryZip<F1, T1, F2, T2>
where
    F1: Future<Output = Result<T1, E>>,
    F2: Future<Output = Result<T2, E>>,
{
    type Output = Result<(T1, T2), E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if let Some(future) = this.future1.as_mut().as_pin_mut() {
            if let Poll::Ready(out) = future.poll(cx) {
                match out {
                    Ok(t) => {
                        *this.output1 = Some(t);
                        this.future1.set(None);
                    }
                    Err(err) => return Poll::Ready(Err(err)),
                }
            }
        }

        if let Some(future) = this.future2.as_mut().as_pin_mut() {
            if let Poll::Ready(out) = future.poll(cx) {
                match out {
                    Ok(t) => {
                        *this.output2 = Some(t);
                        this.future2.set(None);
                    }
                    Err(err) => return Poll::Ready(Err(err)),
                }
            }
        }

        take_zip_from_parts(this.output1, this.output2).map(Ok)
    }
}

/// Returns the result of the future that completes first, preferring `future1` if both are ready.
///
/// If you need to treat the two futures fairly without a preference for either, use the [`race()`]
/// function or the [`FutureExt::race()`] method.
///
/// # Examples
///
/// ```
/// use futures_lite::future::{self, pending, ready};
///
/// # spin_on::spin_on(async {
/// assert_eq!(future::or(ready(1), pending()).await, 1);
/// assert_eq!(future::or(pending(), ready(2)).await, 2);
///
/// // The first future wins.
/// assert_eq!(future::or(ready(1), ready(2)).await, 1);
/// # })
/// ```
pub fn or<T, F1, F2>(future1: F1, future2: F2) -> Or<F1, F2>
where
    F1: Future<Output = T>,
    F2: Future<Output = T>,
{
    Or { future1, future2 }
}

pin_project! {
    /// Future for the [`or()`] function and the [`FutureExt::or()`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Or<F1, F2> {
        #[pin]
        future1: F1,
        #[pin]
        future2: F2,
    }
}

impl<T, F1, F2> Future for Or<F1, F2>
where
    F1: Future<Output = T>,
    F2: Future<Output = T>,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        if let Poll::Ready(t) = this.future1.poll(cx) {
            return Poll::Ready(t);
        }
        if let Poll::Ready(t) = this.future2.poll(cx) {
            return Poll::Ready(t);
        }
        Poll::Pending
    }
}

/// Fuse a future such that `poll` will never again be called once it has
/// completed. This method can be used to turn any `Future` into a
/// `FusedFuture`.
///
/// Normally, once a future has returned `Poll::Ready` from `poll`,
/// any further calls could exhibit bad behavior such as blocking
/// forever, panicking, never returning, etc. If it is known that `poll`
/// may be called too often then this method can be used to ensure that it
/// has defined semantics.
///
/// If a `fuse`d future is `poll`ed after having returned `Poll::Ready`
/// previously, it will return `Poll::Pending`, from `poll` again (and will
/// continue to do so for all future calls to `poll`).
///
/// This combinator will drop the underlying future as soon as it has been
/// completed to ensure resources are reclaimed as soon as possible.
pub fn fuse<F>(future: F) -> Fuse<F>
where
    F: Future + Sized,
{
    Fuse::new(future)
}

pin_project! {
    /// [`Future`] for the [`fuse`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Fuse<Fut> {
        #[pin]
        inner: Option<Fut>,
    }
}

impl<Fut> Fuse<Fut> {
    fn new(f: Fut) -> Self {
        Self { inner: Some(f) }
    }
}

impl<Fut: Future> Future for Fuse<Fut> {
    type Output = Fut::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Fut::Output> {
        match self
            .as_mut()
            .project()
            .inner
            .as_pin_mut()
            .map(|f| f.poll(cx))
        {
            Some(Poll::Ready(output)) => {
                self.project().inner.set(None);
                Poll::Ready(output)
            }

            Some(Poll::Pending) | None => Poll::Pending,
        }
    }
}

/// Returns the result of the future that completes first, with no preference if both are ready.
///
/// Each time [`Race`] is polled, the two inner futures are polled in random order. Therefore, no
/// future takes precedence over the other if both can complete at the same time.
///
/// If you have preference for one of the futures, use the [`or()`] function or the
/// [`FutureExt::or()`] method.
///
/// # Examples
///
/// ```
/// use futures_lite::future::{self, pending, ready};
///
/// # spin_on::spin_on(async {
/// assert_eq!(future::race(ready(1), pending()).await, 1);
/// assert_eq!(future::race(pending(), ready(2)).await, 2);
///
/// // One of the two futures is randomly chosen as the winner.
/// let res = future::race(ready(1), ready(2)).await;
/// # })
/// ```
#[cfg(all(feature = "race", feature = "std"))]
pub fn race<T, F1, F2>(future1: F1, future2: F2) -> Race<F1, F2>
where
    F1: Future<Output = T>,
    F2: Future<Output = T>,
{
    Race {
        future1,
        future2,
        rng: Rng::new(),
    }
}

/// Race two futures but with a predefined random seed.
///
/// This function is identical to [`race`], but instead of using a random seed from a thread-local
/// RNG, it allows the user to provide a seed. It is useful for when you already have a source of
/// randomness available, or if you want to use a fixed seed.
///
/// See documentation of the [`race`] function for features and caveats.
///
/// # Examples
///
/// ```
/// use futures_lite::future::{self, pending, ready};
///
/// // A fixed seed is used, so the result is deterministic.
/// const SEED: u64 = 0x42;
///
/// # spin_on::spin_on(async {
/// assert_eq!(future::race_with_seed(ready(1), pending(), SEED).await, 1);
/// assert_eq!(future::race_with_seed(pending(), ready(2), SEED).await, 2);
///
/// // One of the two futures is randomly chosen as the winner.
/// let res = future::race_with_seed(ready(1), ready(2), SEED).await;
/// # })
/// ```
#[cfg(feature = "race")]
pub fn race_with_seed<T, F1, F2>(future1: F1, future2: F2, seed: u64) -> Race<F1, F2>
where
    F1: Future<Output = T>,
    F2: Future<Output = T>,
{
    Race {
        future1,
        future2,
        rng: Rng::with_seed(seed),
    }
}

#[cfg(feature = "race")]
pin_project! {
    /// Future for the [`race()`] function and the [`FutureExt::race()`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Race<F1, F2> {
        #[pin]
        future1: F1,
        #[pin]
        future2: F2,
        rng: Rng,
    }
}

#[cfg(feature = "race")]
impl<T, F1, F2> Future for Race<F1, F2>
where
    F1: Future<Output = T>,
    F2: Future<Output = T>,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        if this.rng.bool() {
            if let Poll::Ready(t) = this.future1.poll(cx) {
                return Poll::Ready(t);
            }
            if let Poll::Ready(t) = this.future2.poll(cx) {
                return Poll::Ready(t);
            }
        } else {
            if let Poll::Ready(t) = this.future2.poll(cx) {
                return Poll::Ready(t);
            }
            if let Poll::Ready(t) = this.future1.poll(cx) {
                return Poll::Ready(t);
            }
        }
        Poll::Pending
    }
}

#[cfg(feature = "std")]
pin_project! {
    /// Future for the [`FutureExt::catch_unwind()`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct CatchUnwind<F> {
        #[pin]
        inner: F,
    }
}

#[cfg(feature = "std")]
impl<F: Future + UnwindSafe> Future for CatchUnwind<F> {
    type Output = Result<F::Output, Box<dyn Any + Send>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        catch_unwind(AssertUnwindSafe(|| this.inner.poll(cx)))?.map(Ok)
    }
}

/// Type alias for `Pin<Box<dyn Future<Output = T> + Send + 'static>>`.
///
/// # Examples
///
/// ```
/// use futures_lite::future::{self, FutureExt};
///
/// // These two lines are equivalent:
/// let f1: future::Boxed<i32> = async { 1 + 2 }.boxed();
/// let f2: future::Boxed<i32> = Box::pin(async { 1 + 2 });
/// ```
#[cfg(feature = "alloc")]
pub type Boxed<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Type alias for `Pin<Box<dyn Future<Output = T> + 'static>>`.
///
/// # Examples
///
/// ```
/// use futures_lite::future::{self, FutureExt};
///
/// // These two lines are equivalent:
/// let f1: future::BoxedLocal<i32> = async { 1 + 2 }.boxed_local();
/// let f2: future::BoxedLocal<i32> = Box::pin(async { 1 + 2 });
/// ```
#[cfg(feature = "alloc")]
pub type BoxedLocal<T> = Pin<Box<dyn Future<Output = T> + 'static>>;

/// Extension trait for [`Future`].
pub trait FutureExt: Future {
    /// A convenience for calling [`Future::poll()`] on `!`[`Unpin`] types.
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Self::Output>
    where
        Self: Unpin,
    {
        Future::poll(Pin::new(self), cx)
    }

    /// Returns the result of `self` or `other` future, preferring `self` if both are ready.
    ///
    /// If you need to treat the two futures fairly without a preference for either, use the
    /// [`race()`] function or the [`FutureExt::race()`] method.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::future::{pending, ready, FutureExt};
    ///
    /// # spin_on::spin_on(async {
    /// assert_eq!(ready(1).or(pending()).await, 1);
    /// assert_eq!(pending().or(ready(2)).await, 2);
    ///
    /// // The first future wins.
    /// assert_eq!(ready(1).or(ready(2)).await, 1);
    /// # })
    /// ```
    fn or<F>(self, other: F) -> Or<Self, F>
    where
        Self: Sized,
        F: Future<Output = Self::Output>,
    {
        Or {
            future1: self,
            future2: other,
        }
    }

    /// Returns the result of `self` or `other` future, with no preference if both are ready.
    ///
    /// Each time [`Race`] is polled, the two inner futures are polled in random order. Therefore,
    /// no future takes precedence over the other if both can complete at the same time.
    ///
    /// If you have preference for one of the futures, use the [`or()`] function or the
    /// [`FutureExt::or()`] method.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::future::{pending, ready, FutureExt};
    ///
    /// # spin_on::spin_on(async {
    /// assert_eq!(ready(1).race(pending()).await, 1);
    /// assert_eq!(pending().race(ready(2)).await, 2);
    ///
    /// // One of the two futures is randomly chosen as the winner.
    /// let res = ready(1).race(ready(2)).await;
    /// # })
    /// ```
    #[cfg(all(feature = "std", feature = "race"))]
    fn race<F>(self, other: F) -> Race<Self, F>
    where
        Self: Sized,
        F: Future<Output = Self::Output>,
    {
        Race {
            future1: self,
            future2: other,
            rng: Rng::new(),
        }
    }

    /// Catches panics while polling the future.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::future::FutureExt;
    ///
    /// # spin_on::spin_on(async {
    /// let fut1 = async {}.catch_unwind();
    /// let fut2 = async { panic!() }.catch_unwind();
    ///
    /// assert!(fut1.await.is_ok());
    /// assert!(fut2.await.is_err());
    /// # })
    /// ```
    #[cfg(feature = "std")]
    fn catch_unwind(self) -> CatchUnwind<Self>
    where
        Self: Sized + UnwindSafe,
    {
        CatchUnwind { inner: self }
    }

    /// Boxes the future and changes its type to `dyn Future + Send + 'a`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::future::{self, FutureExt};
    ///
    /// # spin_on::spin_on(async {
    /// let a = future::ready('a');
    /// let b = future::pending();
    ///
    /// // Futures of different types can be stored in
    /// // the same collection when they are boxed:
    /// let futures = vec![a.boxed(), b.boxed()];
    /// # })
    /// ```
    #[cfg(feature = "alloc")]
    fn boxed<'a>(self) -> Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>
    where
        Self: Sized + Send + 'a,
    {
        Box::pin(self)
    }

    /// Boxes the future and changes its type to `dyn Future + 'a`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::future::{self, FutureExt};
    ///
    /// # spin_on::spin_on(async {
    /// let a = future::ready('a');
    /// let b = future::pending();
    ///
    /// // Futures of different types can be stored in
    /// // the same collection when they are boxed:
    /// let futures = vec![a.boxed_local(), b.boxed_local()];
    /// # })
    /// ```
    #[cfg(feature = "alloc")]
    fn boxed_local<'a>(self) -> Pin<Box<dyn Future<Output = Self::Output> + 'a>>
    where
        Self: Sized + 'a,
    {
        Box::pin(self)
    }
}

impl<F: Future + ?Sized> FutureExt for F {}
