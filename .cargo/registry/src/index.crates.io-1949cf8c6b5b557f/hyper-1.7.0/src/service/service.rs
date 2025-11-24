use std::future::Future;

/// An asynchronous function from a `Request` to a `Response`.
///
/// The `Service` trait is a simplified interface making it easy to write
/// network applications in a modular and reusable way, decoupled from the
/// underlying protocol.
///
/// # Functional
///
/// A `Service` is a function of a `Request`. It immediately returns a
/// [`Future`] representing the eventual completion of processing the
/// request. The actual request processing may happen at any time in the
/// future, on any thread or executor. The processing may depend on calling
/// other services. At some point in the future, the processing will complete,
/// and the [`Future`] will resolve to a response or an error.
///
/// At a high level, the `Service::call` function represents an RPC request. The
/// `Service` value can be a server or a client.
///
/// # Utilities
///
/// The [`hyper-util`][util] crate provides facilities to bridge this trait to
/// other libraries, such as [`tower`][tower], which might provide their
/// own `Service` variants.
///
/// See [`hyper_util::service`][util-service] for more information.
///
/// [tower]: https://docs.rs/tower
/// [util]: https://docs.rs/hyper-util
/// [util-service]: https://docs.rs/hyper-util/latest/hyper_util/service/index.html
pub trait Service<Request> {
    /// Responses given by the service.
    type Response;

    /// Errors produced by the service.
    ///
    /// Note: Returning an `Error` to a hyper server, the behavior depends on the
    /// protocol. In most cases, hyper will cause the connection to be abruptly aborted.
    /// It will abort the request however the protocol allows, either with some sort of RST_STREAM,
    /// or killing the connection if that doesn't exist.
    type Error;

    /// The future response value.
    type Future: Future<Output = Result<Self::Response, Self::Error>>;

    /// Process the request and return the response asynchronously.
    /// `call` takes `&self` instead of `mut &self` because:
    /// - It prepares the way for async fn,
    ///   since then the future only borrows `&self`, and thus a Service can concurrently handle
    ///   multiple outstanding requests at once.
    /// - It's clearer that Services can likely be cloned.
    /// - To share state across clones, you generally need `Arc<Mutex<_>>`
    ///   That means you're not really using the `&mut self` and could do with a `&self`.
    ///   The discussion on this is here: <https://github.com/hyperium/hyper/issues/3040>
    fn call(&self, req: Request) -> Self::Future;
}

impl<Request, S: Service<Request> + ?Sized> Service<Request> for &'_ S {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn call(&self, req: Request) -> Self::Future {
        (**self).call(req)
    }
}

impl<Request, S: Service<Request> + ?Sized> Service<Request> for &'_ mut S {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn call(&self, req: Request) -> Self::Future {
        (**self).call(req)
    }
}

impl<Request, S: Service<Request> + ?Sized> Service<Request> for Box<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn call(&self, req: Request) -> Self::Future {
        (**self).call(req)
    }
}

impl<Request, S: Service<Request> + ?Sized> Service<Request> for std::rc::Rc<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn call(&self, req: Request) -> Self::Future {
        (**self).call(req)
    }
}

impl<Request, S: Service<Request> + ?Sized> Service<Request> for std::sync::Arc<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn call(&self, req: Request) -> Self::Future {
        (**self).call(req)
    }
}
