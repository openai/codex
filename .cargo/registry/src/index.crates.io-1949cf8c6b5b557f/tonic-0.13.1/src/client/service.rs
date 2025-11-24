use http_body::Body;
use std::future::Future;
use std::task::{Context, Poll};
use tower_service::Service;

/// Definition of the gRPC trait alias for [`tower_service`].
///
/// This trait enforces that all tower services provided to [`Grpc`] implements
/// the correct traits.
///
/// [`Grpc`]: ../client/struct.Grpc.html
/// [`tower_service`]: https://docs.rs/tower-service
pub trait GrpcService<ReqBody> {
    /// Responses body given by the service.
    type ResponseBody: Body;
    /// Errors produced by the service.
    type Error: Into<crate::BoxError>;
    /// The future response value.
    type Future: Future<Output = Result<http::Response<Self::ResponseBody>, Self::Error>>;

    /// Returns `Ready` when the service is able to process requests.
    ///
    /// Reference [`Service::poll_ready`].
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

    /// Process the request and return the response asynchronously.
    ///
    /// Reference [`Service::call`].
    fn call(&mut self, request: http::Request<ReqBody>) -> Self::Future;
}

impl<T, ReqBody, ResBody> GrpcService<ReqBody> for T
where
    T: Service<http::Request<ReqBody>, Response = http::Response<ResBody>>,
    T::Error: Into<crate::BoxError>,
    ResBody: Body,
    <ResBody as Body>::Error: Into<crate::BoxError>,
{
    type ResponseBody = ResBody;
    type Error = T::Error;
    type Future = T::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Service::poll_ready(self, cx)
    }

    fn call(&mut self, request: http::Request<ReqBody>) -> Self::Future {
        Service::call(self, request)
    }
}
