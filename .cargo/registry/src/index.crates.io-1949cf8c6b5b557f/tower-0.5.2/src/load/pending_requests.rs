//! A [`Load`] implementation that measures load using the number of in-flight requests.

#[cfg(feature = "discover")]
use crate::discover::{Change, Discover};
#[cfg(feature = "discover")]
use futures_core::{ready, Stream};
#[cfg(feature = "discover")]
use pin_project_lite::pin_project;
#[cfg(feature = "discover")]
use std::pin::Pin;

use super::completion::{CompleteOnResponse, TrackCompletion, TrackCompletionFuture};
use super::Load;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower_service::Service;

/// Measures the load of the underlying service using the number of currently-pending requests.
#[derive(Debug)]
pub struct PendingRequests<S, C = CompleteOnResponse> {
    service: S,
    ref_count: RefCount,
    completion: C,
}

/// Shared between instances of [`PendingRequests`] and [`Handle`] to track active references.
#[derive(Clone, Debug, Default)]
struct RefCount(Arc<()>);

#[cfg(feature = "discover")]
pin_project! {
    /// Wraps a `D`-typed stream of discovered services with [`PendingRequests`].
    #[cfg_attr(docsrs, doc(cfg(feature = "discover")))]
    #[derive(Debug)]
    pub struct PendingRequestsDiscover<D, C = CompleteOnResponse> {
        #[pin]
        discover: D,
        completion: C,
    }
}

/// Represents the number of currently-pending requests to a given service.
#[derive(Clone, Copy, Debug, Default, PartialOrd, PartialEq, Ord, Eq)]
pub struct Count(usize);

/// Tracks an in-flight request by reference count.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Handle(RefCount);

// ===== impl PendingRequests =====

impl<S, C> PendingRequests<S, C> {
    /// Wraps an `S`-typed service so that its load is tracked by the number of pending requests.
    pub fn new(service: S, completion: C) -> Self {
        Self {
            service,
            completion,
            ref_count: RefCount::default(),
        }
    }

    fn handle(&self) -> Handle {
        Handle(self.ref_count.clone())
    }
}

impl<S, C> Load for PendingRequests<S, C> {
    type Metric = Count;

    fn load(&self) -> Count {
        // Count the number of references that aren't `self`.
        Count(self.ref_count.ref_count() - 1)
    }
}

impl<S, C, Request> Service<Request> for PendingRequests<S, C>
where
    S: Service<Request>,
    C: TrackCompletion<Handle, S::Response>,
{
    type Response = C::Output;
    type Error = S::Error;
    type Future = TrackCompletionFuture<S::Future, C, Handle>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        TrackCompletionFuture::new(
            self.completion.clone(),
            self.handle(),
            self.service.call(req),
        )
    }
}

// ===== impl PendingRequestsDiscover =====

#[cfg(feature = "discover")]
impl<D, C> PendingRequestsDiscover<D, C> {
    /// Wraps a [`Discover`], wrapping all of its services with [`PendingRequests`].
    pub const fn new<Request>(discover: D, completion: C) -> Self
    where
        D: Discover,
        D::Service: Service<Request>,
        C: TrackCompletion<Handle, <D::Service as Service<Request>>::Response>,
    {
        Self {
            discover,
            completion,
        }
    }
}

#[cfg(feature = "discover")]
impl<D, C> Stream for PendingRequestsDiscover<D, C>
where
    D: Discover,
    C: Clone,
{
    type Item = Result<Change<D::Key, PendingRequests<D::Service, C>>, D::Error>;

    /// Yields the next discovery change set.
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use self::Change::*;

        let this = self.project();
        let change = match ready!(this.discover.poll_discover(cx)).transpose()? {
            None => return Poll::Ready(None),
            Some(Insert(k, svc)) => Insert(k, PendingRequests::new(svc, this.completion.clone())),
            Some(Remove(k)) => Remove(k),
        };

        Poll::Ready(Some(Ok(change)))
    }
}

// ==== RefCount ====

impl RefCount {
    pub(crate) fn ref_count(&self) -> usize {
        Arc::strong_count(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::future;
    use std::task::{Context, Poll};

    struct Svc;
    impl Service<()> for Svc {
        type Response = ();
        type Error = ();
        type Future = future::Ready<Result<(), ()>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), ()>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, (): ()) -> Self::Future {
            future::ok(())
        }
    }

    #[test]
    fn default() {
        let mut svc = PendingRequests::new(Svc, CompleteOnResponse);
        assert_eq!(svc.load(), Count(0));

        let rsp0 = svc.call(());
        assert_eq!(svc.load(), Count(1));

        let rsp1 = svc.call(());
        assert_eq!(svc.load(), Count(2));

        let () = tokio_test::block_on(rsp0).unwrap();
        assert_eq!(svc.load(), Count(1));

        let () = tokio_test::block_on(rsp1).unwrap();
        assert_eq!(svc.load(), Count(0));
    }

    #[test]
    fn with_completion() {
        #[derive(Clone)]
        struct IntoHandle;
        impl TrackCompletion<Handle, ()> for IntoHandle {
            type Output = Handle;
            fn track_completion(&self, i: Handle, (): ()) -> Handle {
                i
            }
        }

        let mut svc = PendingRequests::new(Svc, IntoHandle);
        assert_eq!(svc.load(), Count(0));

        let rsp = svc.call(());
        assert_eq!(svc.load(), Count(1));
        let i0 = tokio_test::block_on(rsp).unwrap();
        assert_eq!(svc.load(), Count(1));

        let rsp = svc.call(());
        assert_eq!(svc.load(), Count(2));
        let i1 = tokio_test::block_on(rsp).unwrap();
        assert_eq!(svc.load(), Count(2));

        drop(i1);
        assert_eq!(svc.load(), Count(1));

        drop(i0);
        assert_eq!(svc.load(), Count(0));
    }
}
