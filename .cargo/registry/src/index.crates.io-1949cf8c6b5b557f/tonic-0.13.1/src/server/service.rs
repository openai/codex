use crate::{Request, Response, Status, Streaming};
use std::future::Future;
use tokio_stream::Stream;
use tower_service::Service;

/// A specialization of tower_service::Service.
///
/// Existing tower_service::Service implementations with the correct form will
/// automatically implement `UnaryService`.
pub trait UnaryService<R> {
    /// Protobuf response message type
    type Response;

    /// Response future
    type Future: Future<Output = Result<Response<Self::Response>, Status>>;

    /// Call the service
    fn call(&mut self, request: Request<R>) -> Self::Future;
}

impl<T, M1, M2> UnaryService<M1> for T
where
    T: Service<Request<M1>, Response = Response<M2>, Error = crate::Status>,
{
    type Response = M2;
    type Future = T::Future;

    fn call(&mut self, request: Request<M1>) -> Self::Future {
        Service::call(self, request)
    }
}

/// A specialization of tower_service::Service.
///
/// Existing tower_service::Service implementations with the correct form will
/// automatically implement `ServerStreamingService`.
pub trait ServerStreamingService<R> {
    /// Protobuf response message type
    type Response;

    /// Stream of outbound response messages
    type ResponseStream: Stream<Item = Result<Self::Response, Status>>;

    /// Response future
    type Future: Future<Output = Result<Response<Self::ResponseStream>, Status>>;

    /// Call the service
    fn call(&mut self, request: Request<R>) -> Self::Future;
}

impl<T, S, M1, M2> ServerStreamingService<M1> for T
where
    T: Service<Request<M1>, Response = Response<S>, Error = crate::Status>,
    S: Stream<Item = Result<M2, crate::Status>>,
{
    type Response = M2;
    type ResponseStream = S;
    type Future = T::Future;

    fn call(&mut self, request: Request<M1>) -> Self::Future {
        Service::call(self, request)
    }
}

/// A specialization of tower_service::Service.
///
/// Existing tower_service::Service implementations with the correct form will
/// automatically implement `ClientStreamingService`.
pub trait ClientStreamingService<R> {
    /// Protobuf response message type
    type Response;

    /// Response future
    type Future: Future<Output = Result<Response<Self::Response>, Status>>;

    /// Call the service
    fn call(&mut self, request: Request<Streaming<R>>) -> Self::Future;
}

impl<T, M1, M2> ClientStreamingService<M1> for T
where
    T: Service<Request<Streaming<M1>>, Response = Response<M2>, Error = crate::Status>,
{
    type Response = M2;
    type Future = T::Future;

    fn call(&mut self, request: Request<Streaming<M1>>) -> Self::Future {
        Service::call(self, request)
    }
}

/// A specialization of tower_service::Service.
///
/// Existing tower_service::Service implementations with the correct form will
/// automatically implement `StreamingService`.
pub trait StreamingService<R> {
    /// Protobuf response message type
    type Response;

    /// Stream of outbound response messages
    type ResponseStream: Stream<Item = Result<Self::Response, Status>>;

    /// Response future
    type Future: Future<Output = Result<Response<Self::ResponseStream>, Status>>;

    /// Call the service
    fn call(&mut self, request: Request<Streaming<R>>) -> Self::Future;
}

impl<T, S, M1, M2> StreamingService<M1> for T
where
    T: Service<Request<Streaming<M1>>, Response = Response<S>, Error = crate::Status>,
    S: Stream<Item = Result<M2, crate::Status>>,
{
    type Response = M2;
    type ResponseStream = S;
    type Future = T::Future;

    fn call(&mut self, request: Request<Streaming<M1>>) -> Self::Future {
        Service::call(self, request)
    }
}
