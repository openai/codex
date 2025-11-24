//! Allows a future to execute for a maximum amount of time.
//!
//! See [`Timeout`] documentation for more details.
//!
//! [`Timeout`]: struct@Timeout

use crate::{
    task::coop,
    time::{error::Elapsed, sleep_until, Duration, Instant, Sleep},
    util::trace,
};

use pin_project_lite::pin_project;
use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::task::{self, Poll};

/// Requires a `Future` to complete before the specified duration has elapsed.
///
/// If the future completes before the duration has elapsed, then the completed
/// value is returned. Otherwise, an error is returned and the future is
/// canceled.
///
/// Note that the timeout is checked before polling the future, so if the future
/// does not yield during execution then it is possible for the future to complete
/// and exceed the timeout _without_ returning an error.
///
/// This function returns a future whose return type is [`Result`]`<T,`[`Elapsed`]`>`, where `T` is the
/// return type of the provided future.
///
/// If the provided future completes immediately, then the future returned from
/// this function is guaranteed to complete immediately with an [`Ok`] variant
/// no matter the provided duration.
///
/// [`Ok`]: std::result::Result::Ok
/// [`Result`]: std::result::Result
/// [`Elapsed`]: crate::time::error::Elapsed
///
/// # Cancellation
///
/// Cancelling a timeout is done by dropping the future. No additional cleanup
/// or other work is required.
///
/// The original future may be obtained by calling [`Timeout::into_inner`]. This
/// consumes the `Timeout`.
///
/// # Examples
///
/// Create a new `Timeout` set to expire in 10 milliseconds.
///
/// ```rust
/// use tokio::time::timeout;
/// use tokio::sync::oneshot;
///
/// use std::time::Duration;
///
/// # async fn dox() {
/// let (tx, rx) = oneshot::channel();
/// # tx.send(()).unwrap();
///
/// // Wrap the future with a `Timeout` set to expire in 10 milliseconds.
/// if let Err(_) = timeout(Duration::from_millis(10), rx).await {
///     println!("did not receive value within 10 ms");
/// }
/// # }
/// ```
///
/// # Panics
///
/// This function panics if there is no current timer set.
///
/// It can be triggered when [`Builder::enable_time`] or
/// [`Builder::enable_all`] are not included in the builder.
///
/// It can also panic whenever a timer is created outside of a
/// Tokio runtime. That is why `rt.block_on(sleep(...))` will panic,
/// since the function is executed outside of the runtime.
/// Whereas `rt.block_on(async {sleep(...).await})` doesn't panic.
/// And this is because wrapping the function on an async makes it lazy,
/// and so gets executed inside the runtime successfully without
/// panicking.
///
/// [`Builder::enable_time`]: crate::runtime::Builder::enable_time
/// [`Builder::enable_all`]: crate::runtime::Builder::enable_all
#[track_caller]
pub fn timeout<F>(duration: Duration, future: F) -> Timeout<F::IntoFuture>
where
    F: IntoFuture,
{
    let location = trace::caller_location();

    let deadline = Instant::now().checked_add(duration);
    let delay = match deadline {
        Some(deadline) => Sleep::new_timeout(deadline, location),
        None => Sleep::far_future(location),
    };
    Timeout::new_with_delay(future.into_future(), delay)
}

/// Requires a `Future` to complete before the specified instant in time.
///
/// If the future completes before the instant is reached, then the completed
/// value is returned. Otherwise, an error is returned.
///
/// This function returns a future whose return type is [`Result`]`<T,`[`Elapsed`]`>`, where `T` is the
/// return type of the provided future.
///
/// If the provided future completes immediately, then the future returned from
/// this function is guaranteed to complete immediately with an [`Ok`] variant
/// no matter the provided deadline.
///
/// [`Ok`]: std::result::Result::Ok
/// [`Result`]: std::result::Result
/// [`Elapsed`]: crate::time::error::Elapsed
///
/// # Cancellation
///
/// Cancelling a timeout is done by dropping the future. No additional cleanup
/// or other work is required.
///
/// The original future may be obtained by calling [`Timeout::into_inner`]. This
/// consumes the `Timeout`.
///
/// # Examples
///
/// Create a new `Timeout` set to expire in 10 milliseconds.
///
/// ```rust
/// use tokio::time::{Instant, timeout_at};
/// use tokio::sync::oneshot;
///
/// use std::time::Duration;
///
/// # async fn dox() {
/// let (tx, rx) = oneshot::channel();
/// # tx.send(()).unwrap();
///
/// // Wrap the future with a `Timeout` set to expire 10 milliseconds into the
/// // future.
/// if let Err(_) = timeout_at(Instant::now() + Duration::from_millis(10), rx).await {
///     println!("did not receive value within 10 ms");
/// }
/// # }
/// ```
pub fn timeout_at<F>(deadline: Instant, future: F) -> Timeout<F::IntoFuture>
where
    F: IntoFuture,
{
    let delay = sleep_until(deadline);

    Timeout {
        value: future.into_future(),
        delay,
    }
}

pin_project! {
    /// Future returned by [`timeout`](timeout) and [`timeout_at`](timeout_at).
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    #[derive(Debug)]
    pub struct Timeout<T> {
        #[pin]
        value: T,
        #[pin]
        delay: Sleep,
    }
}

impl<T> Timeout<T> {
    pub(crate) fn new_with_delay(value: T, delay: Sleep) -> Timeout<T> {
        Timeout { value, delay }
    }

    /// Gets a reference to the underlying value in this timeout.
    pub fn get_ref(&self) -> &T {
        &self.value
    }

    /// Gets a mutable reference to the underlying value in this timeout.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }

    /// Consumes this timeout, returning the underlying value.
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Future for Timeout<T>
where
    T: Future,
{
    type Output = Result<T::Output, Elapsed>;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let me = self.project();

        let had_budget_before = coop::has_budget_remaining();

        // First, try polling the future
        if let Poll::Ready(v) = me.value.poll(cx) {
            return Poll::Ready(Ok(v));
        }

        let has_budget_now = coop::has_budget_remaining();

        let delay = me.delay;

        let poll_delay = || -> Poll<Self::Output> {
            match delay.poll(cx) {
                Poll::Ready(()) => Poll::Ready(Err(Elapsed::new())),
                Poll::Pending => Poll::Pending,
            }
        };

        if let (true, false) = (had_budget_before, has_budget_now) {
            // if it is the underlying future that exhausted the budget, we poll
            // the `delay` with an unconstrained one. This prevents pathological
            // cases where the underlying future always exhausts the budget and
            // we never get a chance to evaluate whether the timeout was hit or
            // not.
            coop::with_unconstrained(poll_delay)
        } else {
            poll_delay()
        }
    }
}
