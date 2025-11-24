use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::Hub;

/// A future that binds a `Hub` to its execution.
///
/// This activates the given hub for the duration of the inner futures `poll`
/// method. Users usually do not need to construct this type manually, but
/// rather use the [`FutureExt::bind_hub`] method instead.
///
/// [`FutureExt::bind_hub`]: trait.FutureExt.html#method.bind_hub
#[derive(Debug)]
pub struct SentryFuture<F> {
    hub: Arc<Hub>,
    future: F,
}

impl<F> SentryFuture<F> {
    /// Creates a new bound future with a `Hub`.
    pub fn new(hub: Arc<Hub>, future: F) -> Self {
        Self { hub, future }
    }
}

impl<F> Future for SentryFuture<F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let hub = self.hub.clone();
        // https://doc.rust-lang.org/std/pin/index.html#pinning-is-structural-for-field
        let future = unsafe { self.map_unchecked_mut(|s| &mut s.future) };
        #[cfg(feature = "client")]
        {
            let _guard = crate::hub_impl::SwitchGuard::new(hub);
            future.poll(cx)
        }
        #[cfg(not(feature = "client"))]
        {
            let _ = hub;
            future.poll(cx)
        }
    }
}

/// Future extensions for Sentry.
pub trait SentryFutureExt: Sized {
    /// Binds a hub to the execution of this future.
    ///
    /// This ensures that the future is polled within the given hub.
    fn bind_hub<H>(self, hub: H) -> SentryFuture<Self>
    where
        H: Into<Arc<Hub>>,
    {
        SentryFuture {
            future: self,
            hub: hub.into(),
        }
    }
}

impl<F> SentryFutureExt for F where F: Future {}

#[cfg(all(test, feature = "test"))]
mod tests {
    use crate::test::with_captured_events;
    use crate::{capture_message, configure_scope, Hub, Level, SentryFutureExt};
    use tokio::runtime::Runtime;

    #[test]
    fn test_futures() {
        let mut events = with_captured_events(|| {
            let runtime = Runtime::new().unwrap();

            // spawn two separate tasks, and await them in the end.
            runtime.block_on(async {
                let task1 = async {
                    configure_scope(|scope| scope.set_transaction(Some("transaction1")));
                    capture_message("oh hai from 1", Level::Info);
                }
                .bind_hub(Hub::new_from_top(Hub::current()));
                let task1 = tokio::task::spawn(task1);

                let task2 = async {
                    configure_scope(|scope| scope.set_transaction(Some("transaction2")));
                    capture_message("oh hai from 2", Level::Info);
                }
                .bind_hub(Hub::new_from_top(Hub::current()));
                let task2 = tokio::task::spawn(task2);

                task1.await.unwrap();
                task2.await.unwrap();
            });

            capture_message("oh hai from outside", Level::Info);
        });

        events.sort_by(|a, b| a.transaction.cmp(&b.transaction));
        assert_eq!(events.len(), 3);
        assert_eq!(events[1].transaction, Some("transaction1".into()));
        assert_eq!(events[2].transaction, Some("transaction2".into()));
    }
}
