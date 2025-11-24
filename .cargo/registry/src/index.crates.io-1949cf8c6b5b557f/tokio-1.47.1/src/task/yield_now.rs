use crate::runtime::context;

use std::future::Future;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

/// Yields execution back to the Tokio runtime.
///
/// A task yields by awaiting on `yield_now()`, and may resume when that future
/// completes (with no output.) The current task will be re-added as a pending
/// task at the _back_ of the pending queue. Any other pending tasks will be
/// scheduled. No other waking is required for the task to continue.
///
/// See also the usage example in the [task module](index.html#yield_now).
///
/// ## Non-guarantees
///
/// This function may not yield all the way up to the executor if there are any
/// special combinators above it in the call stack. For example, if a
/// [`tokio::select!`] has another branch complete during the same poll as the
/// `yield_now()`, then the yield is not propagated all the way up to the
/// runtime.
///
/// It is generally not guaranteed that the runtime behaves like you expect it
/// to when deciding which task to schedule next after a call to `yield_now()`.
/// In particular, the runtime may choose to poll the task that just ran
/// `yield_now()` again immediately without polling any other tasks first. For
/// example, the runtime will not drive the IO driver between every poll of a
/// task, and this could result in the runtime polling the current task again
/// immediately even if there is another task that could make progress if that
/// other task is waiting for a notification from the IO driver.
///
/// In general, changes to the order in which the runtime polls tasks is not
/// considered a breaking change, and your program should be correct no matter
/// which order the runtime polls your tasks in.
///
/// [`tokio::select!`]: macro@crate::select
#[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
pub async fn yield_now() {
    /// Yield implementation
    struct YieldNow {
        yielded: bool,
    }

    impl Future for YieldNow {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            ready!(crate::trace::trace_leaf(cx));

            if self.yielded {
                return Poll::Ready(());
            }

            self.yielded = true;

            context::defer(cx.waker());

            Poll::Pending
        }
    }

    YieldNow { yielded: false }.await;
}
