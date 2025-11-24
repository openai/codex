use crate::time::{sleep_until, Duration, Instant, Sleep};
use crate::util::trace;

use std::future::{poll_fn, Future};
use std::panic::Location;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

/// Creates new [`Interval`] that yields with interval of `period`. The first
/// tick completes immediately. The default [`MissedTickBehavior`] is
/// [`Burst`](MissedTickBehavior::Burst), but this can be configured
/// by calling [`set_missed_tick_behavior`](Interval::set_missed_tick_behavior).
///
/// An interval will tick indefinitely. At any time, the [`Interval`] value can
/// be dropped. This cancels the interval.
///
/// This function is equivalent to
/// [`interval_at(Instant::now(), period)`](interval_at).
///
/// # Panics
///
/// This function panics if `period` is zero.
///
/// # Examples
///
/// ```
/// use tokio::time::{self, Duration};
///
/// #[tokio::main]
/// async fn main() {
///     let mut interval = time::interval(Duration::from_millis(10));
///
///     interval.tick().await; // ticks immediately
///     interval.tick().await; // ticks after 10ms
///     interval.tick().await; // ticks after 10ms
///
///     // approximately 20ms have elapsed.
/// }
/// ```
///
/// A simple example using `interval` to execute a task every two seconds.
///
/// The difference between `interval` and [`sleep`] is that an [`Interval`]
/// measures the time since the last tick, which means that [`.tick().await`]
/// may wait for a shorter time than the duration specified for the interval
/// if some time has passed between calls to [`.tick().await`].
///
/// If the tick in the example below was replaced with [`sleep`], the task
/// would only be executed once every three seconds, and not every two
/// seconds.
///
/// ```
/// use tokio::time;
///
/// async fn task_that_takes_a_second() {
///     println!("hello");
///     time::sleep(time::Duration::from_secs(1)).await
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let mut interval = time::interval(time::Duration::from_secs(2));
///     for _i in 0..5 {
///         interval.tick().await;
///         task_that_takes_a_second().await;
///     }
/// }
/// ```
///
/// [`sleep`]: crate::time::sleep()
/// [`.tick().await`]: Interval::tick
#[track_caller]
pub fn interval(period: Duration) -> Interval {
    assert!(period > Duration::new(0, 0), "`period` must be non-zero.");
    internal_interval_at(Instant::now(), period, trace::caller_location())
}

/// Creates new [`Interval`] that yields with interval of `period` with the
/// first tick completing at `start`. The default [`MissedTickBehavior`] is
/// [`Burst`](MissedTickBehavior::Burst), but this can be configured
/// by calling [`set_missed_tick_behavior`](Interval::set_missed_tick_behavior).
///
/// An interval will tick indefinitely. At any time, the [`Interval`] value can
/// be dropped. This cancels the interval.
///
/// # Panics
///
/// This function panics if `period` is zero.
///
/// # Examples
///
/// ```
/// use tokio::time::{interval_at, Duration, Instant};
///
/// #[tokio::main]
/// async fn main() {
///     let start = Instant::now() + Duration::from_millis(50);
///     let mut interval = interval_at(start, Duration::from_millis(10));
///
///     interval.tick().await; // ticks after 50ms
///     interval.tick().await; // ticks after 10ms
///     interval.tick().await; // ticks after 10ms
///
///     // approximately 70ms have elapsed.
/// }
/// ```
#[track_caller]
pub fn interval_at(start: Instant, period: Duration) -> Interval {
    assert!(period > Duration::new(0, 0), "`period` must be non-zero.");
    internal_interval_at(start, period, trace::caller_location())
}

#[cfg_attr(not(all(tokio_unstable, feature = "tracing")), allow(unused_variables))]
fn internal_interval_at(
    start: Instant,
    period: Duration,
    location: Option<&'static Location<'static>>,
) -> Interval {
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    let resource_span = {
        let location = location.expect("should have location if tracing");

        tracing::trace_span!(
            parent: None,
            "runtime.resource",
            concrete_type = "Interval",
            kind = "timer",
            loc.file = location.file(),
            loc.line = location.line(),
            loc.col = location.column(),
        )
    };

    #[cfg(all(tokio_unstable, feature = "tracing"))]
    let delay = resource_span.in_scope(|| Box::pin(sleep_until(start)));

    #[cfg(not(all(tokio_unstable, feature = "tracing")))]
    let delay = Box::pin(sleep_until(start));

    Interval {
        delay,
        period,
        missed_tick_behavior: MissedTickBehavior::default(),
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        resource_span,
    }
}

/// Defines the behavior of an [`Interval`] when it misses a tick.
///
/// Sometimes, an [`Interval`]'s tick is missed. For example, consider the
/// following:
///
/// ```
/// use tokio::time::{self, Duration};
/// # async fn task_that_takes_one_to_three_millis() {}
///
/// #[tokio::main]
/// async fn main() {
///     // ticks every 2 milliseconds
///     let mut interval = time::interval(Duration::from_millis(2));
///     for _ in 0..5 {
///         interval.tick().await;
///         // if this takes more than 2 milliseconds, a tick will be delayed
///         task_that_takes_one_to_three_millis().await;
///     }
/// }
/// ```
///
/// Generally, a tick is missed if too much time is spent without calling
/// [`Interval::tick()`].
///
/// By default, when a tick is missed, [`Interval`] fires ticks as quickly as it
/// can until it is "caught up" in time to where it should be.
/// `MissedTickBehavior` can be used to specify a different behavior for
/// [`Interval`] to exhibit. Each variant represents a different strategy.
///
/// Note that because the executor cannot guarantee exact precision with timers,
/// these strategies will only apply when the delay is greater than 5
/// milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissedTickBehavior {
    /// Ticks as fast as possible until caught up.
    ///
    /// When this strategy is used, [`Interval`] schedules ticks "normally" (the
    /// same as it would have if the ticks hadn't been delayed), which results
    /// in it firing ticks as fast as possible until it is caught up in time to
    /// where it should be. Unlike [`Delay`] and [`Skip`], the ticks yielded
    /// when `Burst` is used (the [`Instant`]s that [`tick`](Interval::tick)
    /// yields) aren't different than they would have been if a tick had not
    /// been missed. Like [`Skip`], and unlike [`Delay`], the ticks may be
    /// shortened.
    ///
    /// This looks something like this:
    /// ```text
    /// Expected ticks: |     1     |     2     |     3     |     4     |     5     |     6     |
    /// Actual ticks:   | work -----|          delay          | work | work | work -| work -----|
    /// ```
    ///
    /// In code:
    ///
    /// ```
    /// use tokio::time::{interval, Duration};
    /// # async fn task_that_takes_200_millis() {}
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// let mut interval = interval(Duration::from_millis(50));
    ///
    /// // First tick resolves immediately after creation
    /// interval.tick().await;
    ///
    /// task_that_takes_200_millis().await;
    /// // The `Interval` has missed a tick
    ///
    /// // Since we have exceeded our timeout, this will resolve immediately
    /// interval.tick().await;
    ///
    /// // Since we are more than 100ms after the start of `interval`, this will
    /// // also resolve immediately.
    /// interval.tick().await;
    ///
    /// // Also resolves immediately, because it was supposed to resolve at
    /// // 150ms after the start of `interval`
    /// interval.tick().await;
    ///
    /// // Resolves immediately
    /// interval.tick().await;
    ///
    /// // Since we have gotten to 200ms after the start of `interval`, this
    /// // will resolve after 50ms
    /// interval.tick().await;
    /// # }
    /// ```
    ///
    /// This is the default behavior when [`Interval`] is created with
    /// [`interval`] and [`interval_at`].
    ///
    /// [`Delay`]: MissedTickBehavior::Delay
    /// [`Skip`]: MissedTickBehavior::Skip
    Burst,

    /// Tick at multiples of `period` from when [`tick`] was called, rather than
    /// from `start`.
    ///
    /// When this strategy is used and [`Interval`] has missed a tick, instead
    /// of scheduling ticks to fire at multiples of `period` from `start` (the
    /// time when the first tick was fired), it schedules all future ticks to
    /// happen at a regular `period` from the point when [`tick`] was called.
    /// Unlike [`Burst`] and [`Skip`], ticks are not shortened, and they aren't
    /// guaranteed to happen at a multiple of `period` from `start` any longer.
    ///
    /// This looks something like this:
    /// ```text
    /// Expected ticks: |     1     |     2     |     3     |     4     |     5     |     6     |
    /// Actual ticks:   | work -----|          delay          | work -----| work -----| work -----|
    /// ```
    ///
    /// In code:
    ///
    /// ```
    /// use tokio::time::{interval, Duration, MissedTickBehavior};
    /// # async fn task_that_takes_more_than_50_millis() {}
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// let mut interval = interval(Duration::from_millis(50));
    /// interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    ///
    /// task_that_takes_more_than_50_millis().await;
    /// // The `Interval` has missed a tick
    ///
    /// // Since we have exceeded our timeout, this will resolve immediately
    /// interval.tick().await;
    ///
    /// // But this one, rather than also resolving immediately, as might happen
    /// // with the `Burst` or `Skip` behaviors, will not resolve until
    /// // 50ms after the call to `tick` up above. That is, in `tick`, when we
    /// // recognize that we missed a tick, we schedule the next tick to happen
    /// // 50ms (or whatever the `period` is) from right then, not from when
    /// // were *supposed* to tick
    /// interval.tick().await;
    /// # }
    /// ```
    ///
    /// [`Burst`]: MissedTickBehavior::Burst
    /// [`Skip`]: MissedTickBehavior::Skip
    /// [`tick`]: Interval::tick
    Delay,

    /// Skips missed ticks and tick on the next multiple of `period` from
    /// `start`.
    ///
    /// When this strategy is used, [`Interval`] schedules the next tick to fire
    /// at the next-closest tick that is a multiple of `period` away from
    /// `start` (the point where [`Interval`] first ticked). Like [`Burst`], all
    /// ticks remain multiples of `period` away from `start`, but unlike
    /// [`Burst`], the ticks may not be *one* multiple of `period` away from the
    /// last tick. Like [`Delay`], the ticks are no longer the same as they
    /// would have been if ticks had not been missed, but unlike [`Delay`], and
    /// like [`Burst`], the ticks may be shortened to be less than one `period`
    /// away from each other.
    ///
    /// This looks something like this:
    /// ```text
    /// Expected ticks: |     1     |     2     |     3     |     4     |     5     |     6     |
    /// Actual ticks:   | work -----|          delay          | work ---| work -----| work -----|
    /// ```
    ///
    /// In code:
    ///
    /// ```
    /// use tokio::time::{interval, Duration, MissedTickBehavior};
    /// # async fn task_that_takes_75_millis() {}
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// let mut interval = interval(Duration::from_millis(50));
    /// interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ///
    /// task_that_takes_75_millis().await;
    /// // The `Interval` has missed a tick
    ///
    /// // Since we have exceeded our timeout, this will resolve immediately
    /// interval.tick().await;
    ///
    /// // This one will resolve after 25ms, 100ms after the start of
    /// // `interval`, which is the closest multiple of `period` from the start
    /// // of `interval` after the call to `tick` up above.
    /// interval.tick().await;
    /// # }
    /// ```
    ///
    /// [`Burst`]: MissedTickBehavior::Burst
    /// [`Delay`]: MissedTickBehavior::Delay
    Skip,
}

impl MissedTickBehavior {
    /// If a tick is missed, this method is called to determine when the next tick should happen.
    fn next_timeout(&self, timeout: Instant, now: Instant, period: Duration) -> Instant {
        match self {
            Self::Burst => timeout + period,
            Self::Delay => now + period,
            Self::Skip => {
                now + period
                    - Duration::from_nanos(
                        ((now - timeout).as_nanos() % period.as_nanos())
                            .try_into()
                            // This operation is practically guaranteed not to
                            // fail, as in order for it to fail, `period` would
                            // have to be longer than `now - timeout`, and both
                            // would have to be longer than 584 years.
                            //
                            // If it did fail, there's not a good way to pass
                            // the error along to the user, so we just panic.
                            .expect(
                                "too much time has elapsed since the interval was supposed to tick",
                            ),
                    )
            }
        }
    }
}

impl Default for MissedTickBehavior {
    /// Returns [`MissedTickBehavior::Burst`].
    ///
    /// For most usecases, the [`Burst`] strategy is what is desired.
    /// Additionally, to preserve backwards compatibility, the [`Burst`]
    /// strategy must be the default. For these reasons,
    /// [`MissedTickBehavior::Burst`] is the default for [`MissedTickBehavior`].
    /// See [`Burst`] for more details.
    ///
    /// [`Burst`]: MissedTickBehavior::Burst
    fn default() -> Self {
        Self::Burst
    }
}

/// Interval returned by [`interval`] and [`interval_at`].
///
/// This type allows you to wait on a sequence of instants with a certain
/// duration between each instant. Unlike calling [`sleep`] in a loop, this lets
/// you count the time spent between the calls to [`sleep`] as well.
///
/// An `Interval` can be turned into a `Stream` with [`IntervalStream`].
///
/// [`IntervalStream`]: https://docs.rs/tokio-stream/latest/tokio_stream/wrappers/struct.IntervalStream.html
/// [`sleep`]: crate::time::sleep()
#[derive(Debug)]
pub struct Interval {
    /// Future that completes the next time the `Interval` yields a value.
    delay: Pin<Box<Sleep>>,

    /// The duration between values yielded by `Interval`.
    period: Duration,

    /// The strategy `Interval` should use when a tick is missed.
    missed_tick_behavior: MissedTickBehavior,

    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
}

impl Interval {
    /// Completes when the next instant in the interval has been reached.
    ///
    /// # Cancel safety
    ///
    /// This method is cancellation safe. If `tick` is used as the branch in a `tokio::select!` and
    /// another branch completes first, then no tick has been consumed.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::time;
    ///
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut interval = time::interval(Duration::from_millis(10));
    ///
    ///     interval.tick().await;
    ///     // approximately 0ms have elapsed. The first tick completes immediately.
    ///     interval.tick().await;
    ///     interval.tick().await;
    ///
    ///     // approximately 20ms have elapsed.
    /// }
    /// ```
    pub async fn tick(&mut self) -> Instant {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let resource_span = self.resource_span.clone();
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let instant = trace::async_op(
            || poll_fn(|cx| self.poll_tick(cx)),
            resource_span,
            "Interval::tick",
            "poll_tick",
            false,
        );
        #[cfg(not(all(tokio_unstable, feature = "tracing")))]
        let instant = poll_fn(|cx| self.poll_tick(cx));

        instant.await
    }

    /// Polls for the next instant in the interval to be reached.
    ///
    /// This method can return the following values:
    ///
    ///  * `Poll::Pending` if the next instant has not yet been reached.
    ///  * `Poll::Ready(instant)` if the next instant has been reached.
    ///
    /// When this method returns `Poll::Pending`, the current task is scheduled
    /// to receive a wakeup when the instant has elapsed. Note that on multiple
    /// calls to `poll_tick`, only the [`Waker`](std::task::Waker) from the
    /// [`Context`] passed to the most recent call is scheduled to receive a
    /// wakeup.
    pub fn poll_tick(&mut self, cx: &mut Context<'_>) -> Poll<Instant> {
        // Wait for the delay to be done
        ready!(Pin::new(&mut self.delay).poll(cx));

        // Get the time when we were scheduled to tick
        let timeout = self.delay.deadline();

        let now = Instant::now();

        // If a tick was not missed, and thus we are being called before the
        // next tick is due, just schedule the next tick normally, one `period`
        // after `timeout`
        //
        // However, if a tick took excessively long and we are now behind,
        // schedule the next tick according to how the user specified with
        // `MissedTickBehavior`
        let next = if now > timeout + Duration::from_millis(5) {
            self.missed_tick_behavior
                .next_timeout(timeout, now, self.period)
        } else {
            timeout
                .checked_add(self.period)
                .unwrap_or_else(Instant::far_future)
        };

        // When we arrive here, the internal delay returned `Poll::Ready`.
        // Reset the delay but do not register it. It should be registered with
        // the next call to [`poll_tick`].
        self.delay.as_mut().reset_without_reregister(next);

        // Return the time when we were scheduled to tick
        Poll::Ready(timeout)
    }

    /// Resets the interval to complete one period after the current time.
    ///
    /// This method ignores [`MissedTickBehavior`] strategy.
    ///
    /// This is equivalent to calling `reset_at(Instant::now() + period)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::time;
    ///
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut interval = time::interval(Duration::from_millis(100));
    ///
    ///     interval.tick().await;
    ///
    ///     time::sleep(Duration::from_millis(50)).await;
    ///     interval.reset();
    ///
    ///     interval.tick().await;
    ///     interval.tick().await;
    ///
    ///     // approximately 250ms have elapsed.
    /// }
    /// ```
    pub fn reset(&mut self) {
        self.delay.as_mut().reset(Instant::now() + self.period);
    }

    /// Resets the interval immediately.
    ///
    /// This method ignores [`MissedTickBehavior`] strategy.
    ///
    /// This is equivalent to calling `reset_at(Instant::now())`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::time;
    ///
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut interval = time::interval(Duration::from_millis(100));
    ///
    ///     interval.tick().await;
    ///
    ///     time::sleep(Duration::from_millis(50)).await;
    ///     interval.reset_immediately();
    ///
    ///     interval.tick().await;
    ///     interval.tick().await;
    ///
    ///     // approximately 150ms have elapsed.
    /// }
    /// ```
    pub fn reset_immediately(&mut self) {
        self.delay.as_mut().reset(Instant::now());
    }

    /// Resets the interval after the specified [`std::time::Duration`].
    ///
    /// This method ignores [`MissedTickBehavior`] strategy.
    ///
    /// This is equivalent to calling `reset_at(Instant::now() + after)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::time;
    ///
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut interval = time::interval(Duration::from_millis(100));
    ///     interval.tick().await;
    ///
    ///     time::sleep(Duration::from_millis(50)).await;
    ///
    ///     let after = Duration::from_millis(20);
    ///     interval.reset_after(after);
    ///
    ///     interval.tick().await;
    ///     interval.tick().await;
    ///
    ///     // approximately 170ms have elapsed.
    /// }
    /// ```
    pub fn reset_after(&mut self, after: Duration) {
        self.delay.as_mut().reset(Instant::now() + after);
    }

    /// Resets the interval to a [`crate::time::Instant`] deadline.
    ///
    /// Sets the next tick to expire at the given instant. If the instant is in
    /// the past, then the [`MissedTickBehavior`] strategy will be used to
    /// catch up. If the instant is in the future, then the next tick will
    /// complete at the given instant, even if that means that it will sleep for
    /// longer than the duration of this [`Interval`]. If the [`Interval`] had
    /// any missed ticks before calling this method, then those are discarded.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::time::{self, Instant};
    ///
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut interval = time::interval(Duration::from_millis(100));
    ///     interval.tick().await;
    ///
    ///     time::sleep(Duration::from_millis(50)).await;
    ///
    ///     let deadline = Instant::now() + Duration::from_millis(30);
    ///     interval.reset_at(deadline);
    ///
    ///     interval.tick().await;
    ///     interval.tick().await;
    ///
    ///     // approximately 180ms have elapsed.
    /// }
    /// ```
    pub fn reset_at(&mut self, deadline: Instant) {
        self.delay.as_mut().reset(deadline);
    }

    /// Returns the [`MissedTickBehavior`] strategy currently being used.
    pub fn missed_tick_behavior(&self) -> MissedTickBehavior {
        self.missed_tick_behavior
    }

    /// Sets the [`MissedTickBehavior`] strategy that should be used.
    pub fn set_missed_tick_behavior(&mut self, behavior: MissedTickBehavior) {
        self.missed_tick_behavior = behavior;
    }

    /// Returns the period of the interval.
    pub fn period(&self) -> Duration {
        self.period
    }
}
