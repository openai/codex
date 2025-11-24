#![cfg_attr(not(feature = "full"), allow(dead_code))]
#![cfg_attr(not(feature = "rt"), allow(unreachable_pub))]

//! Utilities for improved cooperative scheduling.
//!
//! ### Cooperative scheduling
//!
//! A single call to [`poll`] on a top-level task may potentially do a lot of
//! work before it returns `Poll::Pending`. If a task runs for a long period of
//! time without yielding back to the executor, it can starve other tasks
//! waiting on that executor to execute them, or drive underlying resources.
//! Since Rust does not have a runtime, it is difficult to forcibly preempt a
//! long-running task. Instead, this module provides an opt-in mechanism for
//! futures to collaborate with the executor to avoid starvation.
//!
//! Consider a future like this one:
//!
//! ```
//! # use tokio_stream::{Stream, StreamExt};
//! async fn drop_all<I: Stream + Unpin>(mut input: I) {
//!     while let Some(_) = input.next().await {}
//! }
//! ```
//!
//! It may look harmless, but consider what happens under heavy load if the
//! input stream is _always_ ready. If we spawn `drop_all`, the task will never
//! yield, and will starve other tasks and resources on the same executor.
//!
//! To account for this, Tokio has explicit yield points in a number of library
//! functions, which force tasks to return to the executor periodically.
//!
//!
//! #### unconstrained
//!
//! If necessary, [`task::unconstrained`] lets you opt a future out of Tokio's cooperative
//! scheduling. When a future is wrapped with `unconstrained`, it will never be forced to yield to
//! Tokio. For example:
//!
//! ```
//! # #[tokio::main]
//! # async fn main() {
//! use tokio::{task, sync::mpsc};
//!
//! let fut = async {
//!     let (tx, mut rx) = mpsc::unbounded_channel();
//!
//!     for i in 0..1000 {
//!         let _ = tx.send(());
//!         // This will always be ready. If coop was in effect, this code would be forced to yield
//!         // periodically. However, if left unconstrained, then this code will never yield.
//!         rx.recv().await;
//!     }
//! };
//!
//! task::coop::unconstrained(fut).await;
//! # }
//! ```
//! [`poll`]: method@std::future::Future::poll
//! [`task::unconstrained`]: crate::task::unconstrained()

cfg_rt! {
    mod consume_budget;
    pub use consume_budget::consume_budget;

    mod unconstrained;
    pub use unconstrained::{unconstrained, Unconstrained};
}

// ```ignore
// # use tokio_stream::{Stream, StreamExt};
// async fn drop_all<I: Stream + Unpin>(mut input: I) {
//     while let Some(_) = input.next().await {
//         tokio::coop::proceed().await;
//     }
// }
// ```
//
// The `proceed` future will coordinate with the executor to make sure that
// every so often control is yielded back to the executor so it can run other
// tasks.
//
// # Placing yield points
//
// Voluntary yield points should be placed _after_ at least some work has been
// done. If they are not, a future sufficiently deep in the task hierarchy may
// end up _never_ getting to run because of the number of yield points that
// inevitably appear before it is reached. In general, you will want yield
// points to only appear in "leaf" futures -- those that do not themselves poll
// other futures. By doing this, you avoid double-counting each iteration of
// the outer future against the cooperating budget.

use crate::runtime::context;

/// Opaque type tracking the amount of "work" a task may still do before
/// yielding back to the scheduler.
#[derive(Debug, Copy, Clone)]
pub(crate) struct Budget(Option<u8>);

pub(crate) struct BudgetDecrement {
    success: bool,
    hit_zero: bool,
}

impl Budget {
    /// Budget assigned to a task on each poll.
    ///
    /// The value itself is chosen somewhat arbitrarily. It needs to be high
    /// enough to amortize wakeup and scheduling costs, but low enough that we
    /// do not starve other tasks for too long. The value also needs to be high
    /// enough that particularly deep tasks are able to do at least some useful
    /// work at all.
    ///
    /// Note that as more yield points are added in the ecosystem, this value
    /// will probably also have to be raised.
    const fn initial() -> Budget {
        Budget(Some(128))
    }

    /// Returns an unconstrained budget. Operations will not be limited.
    pub(crate) const fn unconstrained() -> Budget {
        Budget(None)
    }

    fn has_remaining(self) -> bool {
        self.0.map_or(true, |budget| budget > 0)
    }
}

/// Runs the given closure with a cooperative task budget. When the function
/// returns, the budget is reset to the value prior to calling the function.
#[inline(always)]
pub(crate) fn budget<R>(f: impl FnOnce() -> R) -> R {
    with_budget(Budget::initial(), f)
}

/// Runs the given closure with an unconstrained task budget. When the function returns, the budget
/// is reset to the value prior to calling the function.
#[inline(always)]
pub(crate) fn with_unconstrained<R>(f: impl FnOnce() -> R) -> R {
    with_budget(Budget::unconstrained(), f)
}

#[inline(always)]
fn with_budget<R>(budget: Budget, f: impl FnOnce() -> R) -> R {
    struct ResetGuard {
        prev: Budget,
    }

    impl Drop for ResetGuard {
        fn drop(&mut self) {
            let _ = context::budget(|cell| {
                cell.set(self.prev);
            });
        }
    }

    #[allow(unused_variables)]
    let maybe_guard = context::budget(|cell| {
        let prev = cell.get();
        cell.set(budget);

        ResetGuard { prev }
    });

    // The function is called regardless even if the budget is not successfully
    // set due to the thread-local being destroyed.
    f()
}

/// Returns `true` if there is still budget left on the task.
///
/// # Examples
///
/// This example defines a `Timeout` future that requires a given `future` to complete before the
/// specified duration elapses. If it does, its result is returned; otherwise, an error is returned
/// and the future is canceled.
///
/// Note that the future could exhaust the budget before we evaluate the timeout. Using `has_budget_remaining`,
/// we can detect this scenario and ensure the timeout is always checked.
///
/// ```
/// # use std::future::Future;
/// # use std::pin::{pin, Pin};
/// # use std::task::{ready, Context, Poll};
/// # use tokio::task::coop;
/// # use tokio::time::Sleep;
/// pub struct Timeout<T> {
///     future: T,
///     delay: Pin<Box<Sleep>>,
/// }
///
/// impl<T> Future for Timeout<T>
/// where
///     T: Future + Unpin,
/// {
///     type Output = Result<T::Output, ()>;
///
///     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
///         let this = Pin::into_inner(self);
///         let future = Pin::new(&mut this.future);
///         let delay = Pin::new(&mut this.delay);
///
///         // check if the future is ready
///         let had_budget_before = coop::has_budget_remaining();
///         if let Poll::Ready(v) = future.poll(cx) {
///             return Poll::Ready(Ok(v));
///         }
///         let has_budget_now = coop::has_budget_remaining();
///
///         // evaluate the timeout
///         if let (true, false) = (had_budget_before, has_budget_now) {
///             // it is the underlying future that exhausted the budget
///             ready!(pin!(coop::unconstrained(delay)).poll(cx));
///         } else {
///             ready!(delay.poll(cx));
///         }
///         return Poll::Ready(Err(()));
///     }
/// }
///```
#[inline(always)]
#[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
pub fn has_budget_remaining() -> bool {
    // If the current budget cannot be accessed due to the thread-local being
    // shutdown, then we assume there is budget remaining.
    context::budget(|cell| cell.get().has_remaining()).unwrap_or(true)
}

cfg_rt_multi_thread! {
    /// Sets the current task's budget.
    pub(crate) fn set(budget: Budget) {
        let _ = context::budget(|cell| cell.set(budget));
    }
}

cfg_rt! {
    /// Forcibly removes the budgeting constraints early.
    ///
    /// Returns the remaining budget
    pub(crate) fn stop() -> Budget {
        context::budget(|cell| {
            let prev = cell.get();
            cell.set(Budget::unconstrained());
            prev
        }).unwrap_or(Budget::unconstrained())
    }
}

cfg_coop! {
    use pin_project_lite::pin_project;
    use std::cell::Cell;
    use std::future::Future;
    use std::marker::PhantomData;
    use std::pin::Pin;
    use std::task::{ready, Context, Poll};

    /// Value returned by the [`poll_proceed`] method.
    #[derive(Debug)]
    #[must_use]
    pub struct RestoreOnPending(Cell<Budget>, PhantomData<*mut ()>);

    impl RestoreOnPending {
        fn new(budget: Budget) -> Self {
            RestoreOnPending(
                Cell::new(budget),
                PhantomData,
            )
        }

        /// Signals that the task that obtained this `RestoreOnPending` was able to make
        /// progress. This prevents the task budget from being restored to the value
        /// it had prior to obtaining this instance when it is dropped.
        pub fn made_progress(&self) {
            self.0.set(Budget::unconstrained());
        }
    }

    impl Drop for RestoreOnPending {
        fn drop(&mut self) {
            // Don't reset if budget was unconstrained or if we made progress.
            // They are both represented as the remembered budget being unconstrained.
            let budget = self.0.get();
            if !budget.is_unconstrained() {
                let _ = context::budget(|cell| {
                    cell.set(budget);
                });
            }
        }
    }

    /// Decrements the task budget and returns [`Poll::Pending`] if the budget is depleted.
    /// This indicates that the task should yield to the scheduler. Otherwise, returns
    /// [`RestoreOnPending`] which can be used to commit the budget consumption.
    ///
    /// The returned [`RestoreOnPending`] will revert the budget to its former
    /// value when dropped unless [`RestoreOnPending::made_progress`]
    /// is called. It is the caller's responsibility to do so when it _was_ able to
    /// make progress after the call to [`poll_proceed`].
    /// Restoring the budget automatically ensures the task can try to make progress in some other
    /// way.
    ///
    /// Note that [`RestoreOnPending`] restores the budget **as it was before [`poll_proceed`]**.
    /// Therefore, if the budget is _further_ adjusted between when [`poll_proceed`] returns and
    /// [`RestoreOnPending`] is dropped, those adjustments are erased unless the caller indicates
    /// that progress was made.
    ///
    /// # Examples
    ///
    /// This example shows a simple countdown latch that uses [`poll_proceed`] to participate in
    /// cooperative scheduling.
    ///
    /// ```
    /// use std::future::{Future};
    /// use std::pin::Pin;
    /// use std::task::{ready, Context, Poll, Waker};
    /// use tokio::task::coop;
    ///
    /// struct CountdownLatch<T> {
    ///     counter: usize,
    ///     value: Option<T>,
    ///     waker: Option<Waker>
    /// }
    ///
    /// impl<T> CountdownLatch<T> {
    ///     fn new(value: T, count: usize) -> Self {
    ///         CountdownLatch {
    ///             counter: count,
    ///             value: Some(value),
    ///             waker: None
    ///         }
    ///     }
    ///     fn count_down(&mut self) {
    ///         if self.counter <= 0 {
    ///             return;
    ///         }
    ///
    ///         self.counter -= 1;
    ///         if self.counter == 0 {
    ///             if let Some(w) = self.waker.take() {
    ///                 w.wake();
    ///             }
    ///         }
    ///     }
    /// }
    ///
    /// impl<T> Future for CountdownLatch<T> {
    ///     type Output = T;
    ///
    ///     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    ///         // `poll_proceed` checks with the runtime if this task is still allowed to proceed
    ///         // with performing work.
    ///         // If not, `Pending` is returned and `ready!` ensures this function returns.
    ///         // If we are allowed to proceed, coop now represents the budget consumption
    ///         let coop = ready!(coop::poll_proceed(cx));
    ///
    ///         // Get a mutable reference to the CountdownLatch
    ///         let this = Pin::get_mut(self);
    ///
    ///         // Next we check if the latch is ready to release its value
    ///         if this.counter == 0 {
    ///             let t = this.value.take();
    ///             // The latch made progress so call `made_progress` to ensure the budget
    ///             // is not reverted.
    ///             coop.made_progress();
    ///             Poll::Ready(t.unwrap())
    ///         } else {
    ///             // If the latch is not ready so return pending and simply drop `coop`.
    ///             // This will restore the budget making it available again to perform any
    ///             // other work.
    ///             this.waker = Some(cx.waker().clone());
    ///             Poll::Pending
    ///         }
    ///     }
    /// }
    ///
    /// impl<T> Unpin for CountdownLatch<T> {}
    /// ```
    #[inline]
    pub fn poll_proceed(cx: &mut Context<'_>) -> Poll<RestoreOnPending> {
        context::budget(|cell| {
            let mut budget = cell.get();

            let decrement = budget.decrement();

            if decrement.success {
                let restore = RestoreOnPending::new(cell.get());
                cell.set(budget);

                // avoid double counting
                if decrement.hit_zero {
                    inc_budget_forced_yield_count();
                }

                Poll::Ready(restore)
            } else {
                register_waker(cx);
                Poll::Pending
            }
        }).unwrap_or(Poll::Ready(RestoreOnPending::new(Budget::unconstrained())))
    }

    /// Returns `Poll::Ready` if the current task has budget to consume, and `Poll::Pending` otherwise.
    ///
    /// Note that in contrast to `poll_proceed`, this method does not consume any budget and is used when
    /// polling for budget availability.
    #[inline]
    pub(crate) fn poll_budget_available(cx: &mut Context<'_>) -> Poll<()> {
        if has_budget_remaining() {
            Poll::Ready(())
        } else {
            register_waker(cx);

            Poll::Pending
        }
    }

    cfg_rt! {
        cfg_unstable_metrics! {
            #[inline(always)]
            fn inc_budget_forced_yield_count() {
                let _ = context::with_current(|handle| {
                    handle.scheduler_metrics().inc_budget_forced_yield_count();
                });
            }
        }

        cfg_not_unstable_metrics! {
            #[inline(always)]
            fn inc_budget_forced_yield_count() {}
        }

        fn register_waker(cx: &mut Context<'_>) {
            context::defer(cx.waker());
        }
    }

    cfg_not_rt! {
        #[inline(always)]
        fn inc_budget_forced_yield_count() {}

        fn register_waker(cx: &mut Context<'_>) {
            cx.waker().wake_by_ref()
        }
    }

    impl Budget {
        /// Decrements the budget. Returns `true` if successful. Decrementing fails
        /// when there is not enough remaining budget.
        fn decrement(&mut self) -> BudgetDecrement {
            if let Some(num) = &mut self.0 {
                if *num > 0 {
                    *num -= 1;

                    let hit_zero = *num == 0;

                    BudgetDecrement { success: true, hit_zero }
                } else {
                    BudgetDecrement { success: false, hit_zero: false }
                }
            } else {
                BudgetDecrement { success: true, hit_zero: false }
            }
        }

        fn is_unconstrained(self) -> bool {
            self.0.is_none()
        }
    }

    pin_project! {
        /// Future wrapper to ensure cooperative scheduling created by [`cooperative`].
        #[must_use = "futures do nothing unless polled"]
        pub struct Coop<F: Future> {
            #[pin]
            pub(crate) fut: F,
        }
    }

    impl<F: Future> Future for Coop<F> {
        type Output = F::Output;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let coop = ready!(poll_proceed(cx));
            let me = self.project();
            if let Poll::Ready(ret) = me.fut.poll(cx) {
                coop.made_progress();
                Poll::Ready(ret)
            } else {
                Poll::Pending
            }
        }
    }

    /// Creates a wrapper future that makes the inner future cooperate with the Tokio scheduler.
    ///
    /// When polled, the wrapper will first call [`poll_proceed`] to consume task budget, and
    /// immediately yield if the budget has been depleted. If budget was available, the inner future
    /// is polled. The budget consumption will be made final using [`RestoreOnPending::made_progress`]
    /// if the inner future resolves to its final value.
    ///
    /// # Examples
    ///
    /// When you call `recv` on the `Receiver` of a [`tokio::sync::mpsc`](crate::sync::mpsc)
    /// channel, task budget will automatically be consumed when the next value is returned.
    /// This makes tasks that use Tokio mpsc channels automatically cooperative.
    ///
    /// If you're using [`futures::channel::mpsc`](https://docs.rs/futures/latest/futures/channel/mpsc/index.html)
    /// instead, automatic task budget consumption will not happen. This example shows how can use
    /// `cooperative` to make `futures::channel::mpsc` channels cooperate with the scheduler in the
    /// same way Tokio channels do.
    ///
    /// ```
    /// use tokio::task::coop::cooperative;
    /// use futures::channel::mpsc::Receiver;
    /// use futures::stream::StreamExt;
    ///
    /// async fn receive_next<T>(receiver: &mut Receiver<T>) -> Option<T> {
    ///     // Use `StreamExt::next` to obtain a `Future` that resolves to the next value
    ///     let recv_future = receiver.next();
    ///     // Wrap it a cooperative wrapper
    ///     let coop_future = cooperative(recv_future);
    ///     // And await
    ///     coop_future.await
    /// }
    #[inline]
    pub fn cooperative<F: Future>(fut: F) -> Coop<F> {
        Coop { fut }
    }
}

#[cfg(all(test, not(loom)))]
mod test {
    use super::*;

    #[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn get() -> Budget {
        context::budget(|cell| cell.get()).unwrap_or(Budget::unconstrained())
    }

    #[test]
    fn budgeting() {
        use std::future::poll_fn;
        use tokio_test::*;

        assert!(get().0.is_none());

        let coop = assert_ready!(task::spawn(()).enter(|cx, _| poll_proceed(cx)));

        assert!(get().0.is_none());
        drop(coop);
        assert!(get().0.is_none());

        budget(|| {
            assert_eq!(get().0, Budget::initial().0);

            let coop = assert_ready!(task::spawn(()).enter(|cx, _| poll_proceed(cx)));
            assert_eq!(get().0.unwrap(), Budget::initial().0.unwrap() - 1);
            drop(coop);
            // we didn't make progress
            assert_eq!(get().0, Budget::initial().0);

            let coop = assert_ready!(task::spawn(()).enter(|cx, _| poll_proceed(cx)));
            assert_eq!(get().0.unwrap(), Budget::initial().0.unwrap() - 1);
            coop.made_progress();
            drop(coop);
            // we _did_ make progress
            assert_eq!(get().0.unwrap(), Budget::initial().0.unwrap() - 1);

            let coop = assert_ready!(task::spawn(()).enter(|cx, _| poll_proceed(cx)));
            assert_eq!(get().0.unwrap(), Budget::initial().0.unwrap() - 2);
            coop.made_progress();
            drop(coop);
            assert_eq!(get().0.unwrap(), Budget::initial().0.unwrap() - 2);

            budget(|| {
                assert_eq!(get().0, Budget::initial().0);

                let coop = assert_ready!(task::spawn(()).enter(|cx, _| poll_proceed(cx)));
                assert_eq!(get().0.unwrap(), Budget::initial().0.unwrap() - 1);
                coop.made_progress();
                drop(coop);
                assert_eq!(get().0.unwrap(), Budget::initial().0.unwrap() - 1);
            });

            assert_eq!(get().0.unwrap(), Budget::initial().0.unwrap() - 2);
        });

        assert!(get().0.is_none());

        budget(|| {
            let n = get().0.unwrap();

            for _ in 0..n {
                let coop = assert_ready!(task::spawn(()).enter(|cx, _| poll_proceed(cx)));
                coop.made_progress();
            }

            let mut task = task::spawn(poll_fn(|cx| {
                let coop = std::task::ready!(poll_proceed(cx));
                coop.made_progress();
                Poll::Ready(())
            }));

            assert_pending!(task.poll());
        });
    }
}
