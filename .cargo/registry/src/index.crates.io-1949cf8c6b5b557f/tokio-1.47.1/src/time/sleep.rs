use crate::runtime::time::TimerEntry;
use crate::time::{error::Error, Duration, Instant};
use crate::util::trace;

use pin_project_lite::pin_project;
use std::future::Future;
use std::panic::Location;
use std::pin::Pin;
use std::task::{self, ready, Poll};

/// Waits until `deadline` is reached.
///
/// No work is performed while awaiting on the sleep future to complete. `Sleep`
/// operates at millisecond granularity and should not be used for tasks that
/// require high-resolution timers.
///
/// To run something regularly on a schedule, see [`interval`].
///
/// # Cancellation
///
/// Canceling a sleep instance is done by dropping the returned future. No additional
/// cleanup work is required.
///
/// # Examples
///
/// Wait 100ms and print "100 ms have elapsed".
///
/// ```
/// use tokio::time::{sleep_until, Instant, Duration};
///
/// #[tokio::main]
/// async fn main() {
///     sleep_until(Instant::now() + Duration::from_millis(100)).await;
///     println!("100 ms have elapsed");
/// }
/// ```
///
/// See the documentation for the [`Sleep`] type for more examples.
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
/// [`Sleep`]: struct@crate::time::Sleep
/// [`interval`]: crate::time::interval()
/// [`Builder::enable_time`]: crate::runtime::Builder::enable_time
/// [`Builder::enable_all`]: crate::runtime::Builder::enable_all
// Alias for old name in 0.x
#[cfg_attr(docsrs, doc(alias = "delay_until"))]
#[track_caller]
pub fn sleep_until(deadline: Instant) -> Sleep {
    Sleep::new_timeout(deadline, trace::caller_location())
}

/// Waits until `duration` has elapsed.
///
/// Equivalent to `sleep_until(Instant::now() + duration)`. An asynchronous
/// analog to `std::thread::sleep`.
///
/// No work is performed while awaiting on the sleep future to complete. `Sleep`
/// operates at millisecond granularity and should not be used for tasks that
/// require high-resolution timers. The implementation is platform specific,
/// and some platforms (specifically Windows) will provide timers with a
/// larger resolution than 1 ms.
///
/// To run something regularly on a schedule, see [`interval`].
///
/// # Cancellation
///
/// Canceling a sleep instance is done by dropping the returned future. No additional
/// cleanup work is required.
///
/// # Examples
///
/// Wait 100ms and print "100 ms have elapsed".
///
/// ```
/// use tokio::time::{sleep, Duration};
///
/// #[tokio::main]
/// async fn main() {
///     sleep(Duration::from_millis(100)).await;
///     println!("100 ms have elapsed");
/// }
/// ```
///
/// See the documentation for the [`Sleep`] type for more examples.
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
/// [`Sleep`]: struct@crate::time::Sleep
/// [`interval`]: crate::time::interval()
/// [`Builder::enable_time`]: crate::runtime::Builder::enable_time
/// [`Builder::enable_all`]: crate::runtime::Builder::enable_all
// Alias for old name in 0.x
#[cfg_attr(docsrs, doc(alias = "delay_for"))]
#[cfg_attr(docsrs, doc(alias = "wait"))]
#[track_caller]
pub fn sleep(duration: Duration) -> Sleep {
    let location = trace::caller_location();

    match Instant::now().checked_add(duration) {
        Some(deadline) => Sleep::new_timeout(deadline, location),
        None => Sleep::new_timeout(Instant::far_future(), location),
    }
}

pin_project! {
    /// Future returned by [`sleep`](sleep) and [`sleep_until`](sleep_until).
    ///
    /// This type does not implement the `Unpin` trait, which means that if you
    /// use it with [`select!`] or by calling `poll`, you have to pin it first.
    /// If you use it with `.await`, this does not apply.
    ///
    /// # Examples
    ///
    /// Wait 100ms and print "100 ms have elapsed".
    ///
    /// ```
    /// use tokio::time::{sleep, Duration};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     sleep(Duration::from_millis(100)).await;
    ///     println!("100 ms have elapsed");
    /// }
    /// ```
    ///
    /// Use with [`select!`]. Pinning the `Sleep` with [`tokio::pin!`] is
    /// necessary when the same `Sleep` is selected on multiple times.
    /// ```no_run
    /// use tokio::time::{self, Duration, Instant};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let sleep = time::sleep(Duration::from_millis(10));
    ///     tokio::pin!(sleep);
    ///
    ///     loop {
    ///         tokio::select! {
    ///             () = &mut sleep => {
    ///                 println!("timer elapsed");
    ///                 sleep.as_mut().reset(Instant::now() + Duration::from_millis(50));
    ///             },
    ///         }
    ///     }
    /// }
    /// ```
    /// Use in a struct with boxing. By pinning the `Sleep` with a `Box`, the
    /// `HasSleep` struct implements `Unpin`, even though `Sleep` does not.
    /// ```
    /// use std::future::Future;
    /// use std::pin::Pin;
    /// use std::task::{Context, Poll};
    /// use tokio::time::Sleep;
    ///
    /// struct HasSleep {
    ///     sleep: Pin<Box<Sleep>>,
    /// }
    ///
    /// impl Future for HasSleep {
    ///     type Output = ();
    ///
    ///     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    ///         self.sleep.as_mut().poll(cx)
    ///     }
    /// }
    /// ```
    /// Use in a struct with pin projection. This method avoids the `Box`, but
    /// the `HasSleep` struct will not be `Unpin` as a consequence.
    /// ```
    /// use std::future::Future;
    /// use std::pin::Pin;
    /// use std::task::{Context, Poll};
    /// use tokio::time::Sleep;
    /// use pin_project_lite::pin_project;
    ///
    /// pin_project! {
    ///     struct HasSleep {
    ///         #[pin]
    ///         sleep: Sleep,
    ///     }
    /// }
    ///
    /// impl Future for HasSleep {
    ///     type Output = ();
    ///
    ///     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    ///         self.project().sleep.poll(cx)
    ///     }
    /// }
    /// ```
    ///
    /// [`select!`]: ../macro.select.html
    /// [`tokio::pin!`]: ../macro.pin.html
    #[project(!Unpin)]
    // Alias for old name in 0.2
    #[cfg_attr(docsrs, doc(alias = "Delay"))]
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Sleep {
        inner: Inner,

        // The link between the `Sleep` instance and the timer that drives it.
        #[pin]
        entry: TimerEntry,
    }
}

cfg_trace! {
    #[derive(Debug)]
    struct Inner {
        ctx: trace::AsyncOpTracingCtx,
    }
}

cfg_not_trace! {
    #[derive(Debug)]
    struct Inner {
    }
}

impl Sleep {
    #[cfg_attr(not(all(tokio_unstable, feature = "tracing")), allow(unused_variables))]
    #[track_caller]
    pub(crate) fn new_timeout(
        deadline: Instant,
        location: Option<&'static Location<'static>>,
    ) -> Sleep {
        use crate::runtime::scheduler;
        let handle = scheduler::Handle::current();
        let entry = TimerEntry::new(handle, deadline);
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let inner = {
            let handle = scheduler::Handle::current();
            let clock = handle.driver().clock();
            let handle = &handle.driver().time();
            let time_source = handle.time_source();
            let deadline_tick = time_source.deadline_to_tick(deadline);
            let duration = deadline_tick.saturating_sub(time_source.now(clock));

            let location = location.expect("should have location if tracing");
            let resource_span = tracing::trace_span!(
                parent: None,
                "runtime.resource",
                concrete_type = "Sleep",
                kind = "timer",
                loc.file = location.file(),
                loc.line = location.line(),
                loc.col = location.column(),
            );

            let async_op_span = resource_span.in_scope(|| {
                tracing::trace!(
                    target: "runtime::resource::state_update",
                    duration = duration,
                    duration.unit = "ms",
                    duration.op = "override",
                );

                tracing::trace_span!("runtime.resource.async_op", source = "Sleep::new_timeout")
            });

            let async_op_poll_span =
                async_op_span.in_scope(|| tracing::trace_span!("runtime.resource.async_op.poll"));

            let ctx = trace::AsyncOpTracingCtx {
                async_op_span,
                async_op_poll_span,
                resource_span,
            };

            Inner { ctx }
        };

        #[cfg(not(all(tokio_unstable, feature = "tracing")))]
        let inner = Inner {};

        Sleep { inner, entry }
    }

    pub(crate) fn far_future(location: Option<&'static Location<'static>>) -> Sleep {
        Self::new_timeout(Instant::far_future(), location)
    }

    /// Returns the instant at which the future will complete.
    pub fn deadline(&self) -> Instant {
        self.entry.deadline()
    }

    /// Returns `true` if `Sleep` has elapsed.
    ///
    /// A `Sleep` instance is elapsed when the requested duration has elapsed.
    pub fn is_elapsed(&self) -> bool {
        self.entry.is_elapsed()
    }

    /// Resets the `Sleep` instance to a new deadline.
    ///
    /// Calling this function allows changing the instant at which the `Sleep`
    /// future completes without having to create new associated state.
    ///
    /// This function can be called both before and after the future has
    /// completed.
    ///
    /// To call this method, you will usually combine the call with
    /// [`Pin::as_mut`], which lets you call the method without consuming the
    /// `Sleep` itself.
    ///
    /// # Example
    ///
    /// ```
    /// use tokio::time::{Duration, Instant};
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// let sleep = tokio::time::sleep(Duration::from_millis(10));
    /// tokio::pin!(sleep);
    ///
    /// sleep.as_mut().reset(Instant::now() + Duration::from_millis(20));
    /// # }
    /// ```
    ///
    /// See also the top-level examples.
    ///
    /// [`Pin::as_mut`]: fn@std::pin::Pin::as_mut
    pub fn reset(self: Pin<&mut Self>, deadline: Instant) {
        self.reset_inner(deadline);
    }

    /// Resets the `Sleep` instance to a new deadline without reregistering it
    /// to be woken up.
    ///
    /// Calling this function allows changing the instant at which the `Sleep`
    /// future completes without having to create new associated state and
    /// without having it registered. This is required in e.g. the
    /// [`crate::time::Interval`] where we want to reset the internal [Sleep]
    /// without having it wake up the last task that polled it.
    pub(crate) fn reset_without_reregister(self: Pin<&mut Self>, deadline: Instant) {
        let mut me = self.project();
        me.entry.as_mut().reset(deadline, false);
    }

    fn reset_inner(self: Pin<&mut Self>, deadline: Instant) {
        let mut me = self.project();
        me.entry.as_mut().reset(deadline, true);

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        {
            let _resource_enter = me.inner.ctx.resource_span.enter();
            me.inner.ctx.async_op_span =
                tracing::trace_span!("runtime.resource.async_op", source = "Sleep::reset");
            let _async_op_enter = me.inner.ctx.async_op_span.enter();

            me.inner.ctx.async_op_poll_span =
                tracing::trace_span!("runtime.resource.async_op.poll");

            let duration = {
                let clock = me.entry.clock();
                let time_source = me.entry.driver().time_source();
                let now = time_source.now(clock);
                let deadline_tick = time_source.deadline_to_tick(deadline);
                deadline_tick.saturating_sub(now)
            };

            tracing::trace!(
                target: "runtime::resource::state_update",
                duration = duration,
                duration.unit = "ms",
                duration.op = "override",
            );
        }
    }

    fn poll_elapsed(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Result<(), Error>> {
        let me = self.project();

        ready!(crate::trace::trace_leaf(cx));

        // Keep track of task budget
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let coop = ready!(trace_poll_op!(
            "poll_elapsed",
            crate::task::coop::poll_proceed(cx),
        ));

        #[cfg(any(not(tokio_unstable), not(feature = "tracing")))]
        let coop = ready!(crate::task::coop::poll_proceed(cx));

        let result = me.entry.poll_elapsed(cx).map(move |r| {
            coop.made_progress();
            r
        });

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        return trace_poll_op!("poll_elapsed", result);

        #[cfg(any(not(tokio_unstable), not(feature = "tracing")))]
        return result;
    }
}

impl Future for Sleep {
    type Output = ();

    // `poll_elapsed` can return an error in two cases:
    //
    // - AtCapacity: this is a pathological case where far too many
    //   sleep instances have been scheduled.
    // - Shutdown: No timer has been setup, which is a mis-use error.
    //
    // Both cases are extremely rare, and pretty accurately fit into
    // "logic errors", so we just panic in this case. A user couldn't
    // really do much better if we passed the error onwards.
    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let _res_span = self.inner.ctx.resource_span.clone().entered();
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let _ao_span = self.inner.ctx.async_op_span.clone().entered();
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let _ao_poll_span = self.inner.ctx.async_op_poll_span.clone().entered();
        match ready!(self.as_mut().poll_elapsed(cx)) {
            Ok(()) => Poll::Ready(()),
            Err(e) => panic!("timer error: {e}"),
        }
    }
}
