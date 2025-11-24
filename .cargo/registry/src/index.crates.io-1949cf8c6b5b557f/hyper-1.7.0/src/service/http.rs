use std::error::Error as StdError;
use std::future::Future;

use crate::body::Body;
use crate::service::service::Service;
use crate::{Request, Response};

/// An asynchronous function from [`Request`] to [`Response`].
///
/// This is a *sealed* trait, meaning that it can not be implemented directly. Rather, it is an
/// alias for [`Service`]s that accept a [`Request`] and return a [`Future`] that resolves to a
/// [`Response`]. External callers should implement [`Service`] instead.
///
/// Rather than being generic over the request and response, this trait is generic across the
/// request [`Body`] and response [`Body`].
///
/// See the crate-level [`service`][crate::service] documentation for more information.
///
/// See [`Service`] for more information.
pub trait HttpService<ReqBody>: sealed::Sealed<ReqBody> {
    /// The [`Body`] body of the [`Response`].
    type ResBody: Body;

    /// The error type that can occur within this [`Service`].
    ///
    /// Note: Returning an `Error` to a hyper server, the behavior depends on the protocol. In
    /// most cases, hyper will cause the connection to be abruptly aborted. In most cases, it is
    /// better to return a `Response` with a 4xx or 5xx status code.
    ///
    /// See [`Service::Error`] for more information.
    type Error: Into<Box<dyn StdError + Send + Sync>>;

    /// The [`Future`] returned by this [`Service`].
    type Future: Future<Output = Result<Response<Self::ResBody>, Self::Error>>;

    #[doc(hidden)]
    fn call(&mut self, req: Request<ReqBody>) -> Self::Future;
}

impl<T, B1, B2> HttpService<B1> for T
where
    T: Service<Request<B1>, Response = Response<B2>>,
    B2: Body,
    T::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type ResBody = B2;

    type Error = T::Error;
    type Future = T::Future;

    fn call(&mut self, req: Request<B1>) -> Self::Future {
        Service::call(self, req)
    }
}

impl<T, B1, B2> sealed::Sealed<B1> for T
where
    T: Service<Request<B1>, Response = Response<B2>>,
    B2: Body,
{
}

mod sealed {
    pub trait Sealed<T> {}
}
