use super::service::Buffer;
use std::{fmt, marker::PhantomData};
use tower_layer::Layer;
use tower_service::Service;

/// Adds an mpsc buffer in front of an inner service.
///
/// The default Tokio executor is used to run the given service,
/// which means that this layer can only be used on the Tokio runtime.
///
/// See the module documentation for more details.
pub struct BufferLayer<Request> {
    bound: usize,
    _p: PhantomData<fn(Request)>,
}

impl<Request> BufferLayer<Request> {
    /// Creates a new [`BufferLayer`] with the provided `bound`.
    ///
    /// `bound` gives the maximal number of requests that can be queued for the service before
    /// backpressure is applied to callers.
    ///
    /// # A note on choosing a `bound`
    ///
    /// When [`Buffer`]'s implementation of [`poll_ready`] returns [`Poll::Ready`], it reserves a
    /// slot in the channel for the forthcoming [`call`]. However, if this call doesn't arrive,
    /// this reserved slot may be held up for a long time. As a result, it's advisable to set
    /// `bound` to be at least the maximum number of concurrent requests the [`Buffer`] will see.
    /// If you do not, all the slots in the buffer may be held up by futures that have just called
    /// [`poll_ready`] but will not issue a [`call`], which prevents other senders from issuing new
    /// requests.
    ///
    /// [`Poll::Ready`]: std::task::Poll::Ready
    /// [`call`]: crate::Service::call
    /// [`poll_ready`]: crate::Service::poll_ready
    pub const fn new(bound: usize) -> Self {
        BufferLayer {
            bound,
            _p: PhantomData,
        }
    }
}

impl<S, Request> Layer<S> for BufferLayer<Request>
where
    S: Service<Request> + Send + 'static,
    S::Future: Send,
    S::Error: Into<crate::BoxError> + Send + Sync,
    Request: Send + 'static,
{
    type Service = Buffer<Request, S::Future>;

    fn layer(&self, service: S) -> Self::Service {
        Buffer::new(service, self.bound)
    }
}

impl<Request> fmt::Debug for BufferLayer<Request> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BufferLayer")
            .field("bound", &self.bound)
            .finish()
    }
}

impl<Request> Clone for BufferLayer<Request> {
    fn clone(&self) -> Self {
        Self {
            bound: self.bound,
            _p: PhantomData,
        }
    }
}

impl<Request> Copy for BufferLayer<Request> {}
