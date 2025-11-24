//! Various utility types and functions that are generally used with Tower.

mod and_then;
mod boxed;
mod boxed_clone;
mod boxed_clone_sync;
mod call_all;
mod either;

mod future_service;
mod map_err;
mod map_request;
mod map_response;
mod map_result;

mod map_future;
mod oneshot;
mod optional;
mod ready;
mod service_fn;
mod then;

pub mod rng;

pub use self::{
    and_then::{AndThen, AndThenLayer},
    boxed::{
        BoxCloneServiceLayer, BoxCloneSyncServiceLayer, BoxLayer, BoxService, UnsyncBoxService,
    },
    boxed_clone::BoxCloneService,
    boxed_clone_sync::BoxCloneSyncService,
    either::Either,
    future_service::{future_service, FutureService},
    map_err::{MapErr, MapErrLayer},
    map_future::{MapFuture, MapFutureLayer},
    map_request::{MapRequest, MapRequestLayer},
    map_response::{MapResponse, MapResponseLayer},
    map_result::{MapResult, MapResultLayer},
    oneshot::Oneshot,
    optional::Optional,
    ready::{Ready, ReadyOneshot},
    service_fn::{service_fn, ServiceFn},
    then::{Then, ThenLayer},
};

pub use self::call_all::{CallAll, CallAllUnordered};
use std::future::Future;

use crate::layer::util::Identity;

pub mod error {
    //! Error types

    pub use super::optional::error as optional;
}

pub mod future {
    //! Future types

    pub use super::and_then::AndThenFuture;
    pub use super::either::EitherResponseFuture;
    pub use super::map_err::MapErrFuture;
    pub use super::map_response::MapResponseFuture;
    pub use super::map_result::MapResultFuture;
    pub use super::optional::future as optional;
    pub use super::then::ThenFuture;
}

/// An extension trait for `Service`s that provides a variety of convenient
/// adapters
pub trait ServiceExt<Request>: tower_service::Service<Request> {
    /// Yields a mutable reference to the service when it is ready to accept a request.
    fn ready(&mut self) -> Ready<'_, Self, Request>
    where
        Self: Sized,
    {
        Ready::new(self)
    }

    /// Yields the service when it is ready to accept a request.
    fn ready_oneshot(self) -> ReadyOneshot<Self, Request>
    where
        Self: Sized,
    {
        ReadyOneshot::new(self)
    }

    /// Consume this `Service`, calling it with the provided request once it is ready.
    fn oneshot(self, req: Request) -> Oneshot<Self, Request>
    where
        Self: Sized,
    {
        Oneshot::new(self, req)
    }

    /// Process all requests from the given [`Stream`], and produce a [`Stream`] of their responses.
    ///
    /// This is essentially [`Stream<Item = Request>`][stream] + `Self` => [`Stream<Item =
    /// Response>`][stream]. See the documentation for [`CallAll`] for
    /// details.
    ///
    /// [`Stream`]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html
    /// [stream]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html
    fn call_all<S>(self, reqs: S) -> CallAll<Self, S>
    where
        Self: Sized,
        S: futures_core::Stream<Item = Request>,
    {
        CallAll::new(self, reqs)
    }

    /// Executes a new future after this service's future resolves. This does
    /// not alter the behaviour of the [`poll_ready`] method.
    ///
    /// This method can be used to change the [`Response`] type of the service
    /// into a different type. You can use this method to chain along a computation once the
    /// service's response has been resolved.
    ///
    /// [`Response`]: crate::Service::Response
    /// [`poll_ready`]: crate::Service::poll_ready
    ///
    /// # Example
    /// ```
    /// # use std::task::{Poll, Context};
    /// # use tower::{Service, ServiceExt};
    /// #
    /// # struct DatabaseService;
    /// # impl DatabaseService {
    /// #   fn new(address: &str) -> Self {
    /// #       DatabaseService
    /// #   }
    /// # }
    /// #
    /// # struct Record {
    /// #   pub name: String,
    /// #   pub age: u16
    /// # }
    /// #
    /// # impl Service<u32> for DatabaseService {
    /// #   type Response = Record;
    /// #   type Error = u8;
    /// #   type Future = futures_util::future::Ready<Result<Record, u8>>;
    /// #
    /// #   fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    /// #       Poll::Ready(Ok(()))
    /// #   }
    /// #
    /// #   fn call(&mut self, request: u32) -> Self::Future {
    /// #       futures_util::future::ready(Ok(Record { name: "Jack".into(), age: 32 }))
    /// #   }
    /// # }
    /// #
    /// # async fn avatar_lookup(name: String) -> Result<Vec<u8>, u8> { Ok(vec![]) }
    /// #
    /// # fn main() {
    /// #    async {
    /// // A service returning Result<Record, _>
    /// let service = DatabaseService::new("127.0.0.1:8080");
    ///
    /// // Map the response into a new response
    /// let mut new_service = service.and_then(|record: Record| async move {
    ///     let name = record.name;
    ///     avatar_lookup(name).await
    /// });
    ///
    /// // Call the new service
    /// let id = 13;
    /// let avatar = new_service.call(id).await.unwrap();
    /// #    };
    /// # }
    /// ```
    fn and_then<F>(self, f: F) -> AndThen<Self, F>
    where
        Self: Sized,
        F: Clone,
    {
        AndThen::new(self, f)
    }

    /// Maps this service's response value to a different value. This does not
    /// alter the behaviour of the [`poll_ready`] method.
    ///
    /// This method can be used to change the [`Response`] type of the service
    /// into a different type. It is similar to the [`Result::map`]
    /// method. You can use this method to chain along a computation once the
    /// service's response has been resolved.
    ///
    /// [`Response`]: crate::Service::Response
    /// [`poll_ready`]: crate::Service::poll_ready
    ///
    /// # Example
    /// ```
    /// # use std::task::{Poll, Context};
    /// # use tower::{Service, ServiceExt};
    /// #
    /// # struct DatabaseService;
    /// # impl DatabaseService {
    /// #   fn new(address: &str) -> Self {
    /// #       DatabaseService
    /// #   }
    /// # }
    /// #
    /// # struct Record {
    /// #   pub name: String,
    /// #   pub age: u16
    /// # }
    /// #
    /// # impl Service<u32> for DatabaseService {
    /// #   type Response = Record;
    /// #   type Error = u8;
    /// #   type Future = futures_util::future::Ready<Result<Record, u8>>;
    /// #
    /// #   fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    /// #       Poll::Ready(Ok(()))
    /// #   }
    /// #
    /// #   fn call(&mut self, request: u32) -> Self::Future {
    /// #       futures_util::future::ready(Ok(Record { name: "Jack".into(), age: 32 }))
    /// #   }
    /// # }
    /// #
    /// # fn main() {
    /// #    async {
    /// // A service returning Result<Record, _>
    /// let service = DatabaseService::new("127.0.0.1:8080");
    ///
    /// // Map the response into a new response
    /// let mut new_service = service.map_response(|record| record.name);
    ///
    /// // Call the new service
    /// let id = 13;
    /// let name = new_service
    ///     .ready()
    ///     .await?
    ///     .call(id)
    ///     .await?;
    /// # Ok::<(), u8>(())
    /// #    };
    /// # }
    /// ```
    fn map_response<F, Response>(self, f: F) -> MapResponse<Self, F>
    where
        Self: Sized,
        F: FnOnce(Self::Response) -> Response + Clone,
    {
        MapResponse::new(self, f)
    }

    /// Maps this service's error value to a different value. This does not
    /// alter the behaviour of the [`poll_ready`] method.
    ///
    /// This method can be used to change the [`Error`] type of the service
    /// into a different type. It is similar to the [`Result::map_err`] method.
    ///
    /// [`Error`]: crate::Service::Error
    /// [`poll_ready`]: crate::Service::poll_ready
    ///
    /// # Example
    /// ```
    /// # use std::task::{Poll, Context};
    /// # use tower::{Service, ServiceExt};
    /// #
    /// # struct DatabaseService;
    /// # impl DatabaseService {
    /// #   fn new(address: &str) -> Self {
    /// #       DatabaseService
    /// #   }
    /// # }
    /// #
    /// # struct Error {
    /// #   pub code: u32,
    /// #   pub message: String
    /// # }
    /// #
    /// # impl Service<u32> for DatabaseService {
    /// #   type Response = String;
    /// #   type Error = Error;
    /// #   type Future = futures_util::future::Ready<Result<String, Error>>;
    /// #
    /// #   fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    /// #       Poll::Ready(Ok(()))
    /// #   }
    /// #
    /// #   fn call(&mut self, request: u32) -> Self::Future {
    /// #       futures_util::future::ready(Ok(String::new()))
    /// #   }
    /// # }
    /// #
    /// # fn main() {
    /// #   async {
    /// // A service returning Result<_, Error>
    /// let service = DatabaseService::new("127.0.0.1:8080");
    ///
    /// // Map the error to a new error
    /// let mut new_service = service.map_err(|err| err.code);
    ///
    /// // Call the new service
    /// let id = 13;
    /// let code = new_service
    ///     .ready()
    ///     .await?
    ///     .call(id)
    ///     .await
    ///     .unwrap_err();
    /// # Ok::<(), u32>(())
    /// #   };
    /// # }
    /// ```
    fn map_err<F, Error>(self, f: F) -> MapErr<Self, F>
    where
        Self: Sized,
        F: FnOnce(Self::Error) -> Error + Clone,
    {
        MapErr::new(self, f)
    }

    /// Maps this service's result type (`Result<Self::Response, Self::Error>`)
    /// to a different value, regardless of whether the future succeeds or
    /// fails.
    ///
    /// This is similar to the [`map_response`] and [`map_err`] combinators,
    /// except that the *same* function is invoked when the service's future
    /// completes, whether it completes successfully or fails. This function
    /// takes the [`Result`] returned by the service's future, and returns a
    /// [`Result`].
    ///
    /// Like the standard library's [`Result::and_then`], this method can be
    /// used to implement control flow based on `Result` values. For example, it
    /// may be used to implement error recovery, by turning some [`Err`]
    /// responses from the service into [`Ok`] responses. Similarly, some
    /// successful responses from the service could be rejected, by returning an
    /// [`Err`] conditionally, depending on the value inside the [`Ok`.] Finally,
    /// this method can also be used to implement behaviors that must run when a
    /// service's future completes, regardless of whether it succeeded or failed.
    ///
    /// This method can be used to change the [`Response`] type of the service
    /// into a different type. It can also be used to change the [`Error`] type
    /// of the service. However, because the [`map_result`] function is not applied
    /// to the errors returned by the service's [`poll_ready`] method, it must
    /// be possible to convert the service's [`Error`] type into the error type
    /// returned by the [`map_result`] function. This is trivial when the function
    /// returns the same error type as the service, but in other cases, it can
    /// be useful to use [`BoxError`] to erase differing error types.
    ///
    /// # Examples
    ///
    /// Recovering from certain errors:
    ///
    /// ```
    /// # use std::task::{Poll, Context};
    /// # use tower::{Service, ServiceExt};
    /// #
    /// # struct DatabaseService;
    /// # impl DatabaseService {
    /// #   fn new(address: &str) -> Self {
    /// #       DatabaseService
    /// #   }
    /// # }
    /// #
    /// # struct Record {
    /// #   pub name: String,
    /// #   pub age: u16
    /// # }
    /// # #[derive(Debug)]
    /// # enum DbError {
    /// #   Parse(std::num::ParseIntError),
    /// #   NoRecordsFound,
    /// # }
    /// #
    /// # impl Service<u32> for DatabaseService {
    /// #   type Response = Vec<Record>;
    /// #   type Error = DbError;
    /// #   type Future = futures_util::future::Ready<Result<Vec<Record>, DbError>>;
    /// #
    /// #   fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    /// #       Poll::Ready(Ok(()))
    /// #   }
    /// #
    /// #   fn call(&mut self, request: u32) -> Self::Future {
    /// #       futures_util::future::ready(Ok(vec![Record { name: "Jack".into(), age: 32 }]))
    /// #   }
    /// # }
    /// #
    /// # fn main() {
    /// #    async {
    /// // A service returning Result<Vec<Record>, DbError>
    /// let service = DatabaseService::new("127.0.0.1:8080");
    ///
    /// // If the database returns no records for the query, we just want an empty `Vec`.
    /// let mut new_service = service.map_result(|result| match result {
    ///     // If the error indicates that no records matched the query, return an empty
    ///     // `Vec` instead.
    ///     Err(DbError::NoRecordsFound) => Ok(Vec::new()),
    ///     // Propagate all other responses (`Ok` and `Err`) unchanged
    ///     x => x,
    /// });
    ///
    /// // Call the new service
    /// let id = 13;
    /// let name = new_service
    ///     .ready()
    ///     .await?
    ///     .call(id)
    ///     .await?;
    /// # Ok::<(), DbError>(())
    /// #    };
    /// # }
    /// ```
    ///
    /// Rejecting some `Ok` responses:
    ///
    /// ```
    /// # use std::task::{Poll, Context};
    /// # use tower::{Service, ServiceExt};
    /// #
    /// # struct DatabaseService;
    /// # impl DatabaseService {
    /// #   fn new(address: &str) -> Self {
    /// #       DatabaseService
    /// #   }
    /// # }
    /// #
    /// # struct Record {
    /// #   pub name: String,
    /// #   pub age: u16
    /// # }
    /// # type DbError = String;
    /// # type AppError = String;
    /// #
    /// # impl Service<u32> for DatabaseService {
    /// #   type Response = Record;
    /// #   type Error = DbError;
    /// #   type Future = futures_util::future::Ready<Result<Record, DbError>>;
    /// #
    /// #   fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    /// #       Poll::Ready(Ok(()))
    /// #   }
    /// #
    /// #   fn call(&mut self, request: u32) -> Self::Future {
    /// #       futures_util::future::ready(Ok(Record { name: "Jack".into(), age: 32 }))
    /// #   }
    /// # }
    /// #
    /// # fn main() {
    /// #    async {
    /// use tower::BoxError;
    ///
    /// // A service returning Result<Record, DbError>
    /// let service = DatabaseService::new("127.0.0.1:8080");
    ///
    /// // If the user is zero years old, return an error.
    /// let mut new_service = service.map_result(|result| {
    ///    let record = result?;
    ///
    ///    if record.age == 0 {
    ///         // Users must have been born to use our app!
    ///         let app_error = AppError::from("users cannot be 0 years old!");
    ///
    ///         // Box the error to erase its type (as it can be an `AppError`
    ///         // *or* the inner service's `DbError`).
    ///         return Err(BoxError::from(app_error));
    ///     }
    ///
    ///     // Otherwise, return the record.
    ///     Ok(record)
    /// });
    ///
    /// // Call the new service
    /// let id = 13;
    /// let record = new_service
    ///     .ready()
    ///     .await?
    ///     .call(id)
    ///     .await?;
    /// # Ok::<(), BoxError>(())
    /// #    };
    /// # }
    /// ```
    ///
    /// Performing an action that must be run for both successes and failures:
    ///
    /// ```
    /// # use std::convert::TryFrom;
    /// # use std::task::{Poll, Context};
    /// # use tower::{Service, ServiceExt};
    /// #
    /// # struct DatabaseService;
    /// # impl DatabaseService {
    /// #   fn new(address: &str) -> Self {
    /// #       DatabaseService
    /// #   }
    /// # }
    /// #
    /// # impl Service<u32> for DatabaseService {
    /// #   type Response = String;
    /// #   type Error = u8;
    /// #   type Future = futures_util::future::Ready<Result<String, u8>>;
    /// #
    /// #   fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    /// #       Poll::Ready(Ok(()))
    /// #   }
    /// #
    /// #   fn call(&mut self, request: u32) -> Self::Future {
    /// #       futures_util::future::ready(Ok(String::new()))
    /// #   }
    /// # }
    /// #
    /// # fn main() {
    /// #   async {
    /// // A service returning Result<Record, DbError>
    /// let service = DatabaseService::new("127.0.0.1:8080");
    ///
    /// // Print a message whenever a query completes.
    /// let mut new_service = service.map_result(|result| {
    ///     println!("query completed; success={}", result.is_ok());
    ///     result
    /// });
    ///
    /// // Call the new service
    /// let id = 13;
    /// let response = new_service
    ///     .ready()
    ///     .await?
    ///     .call(id)
    ///     .await;
    /// # response
    /// #    };
    /// # }
    /// ```
    ///
    /// [`map_response`]: ServiceExt::map_response
    /// [`map_err`]: ServiceExt::map_err
    /// [`map_result`]: ServiceExt::map_result
    /// [`Error`]: crate::Service::Error
    /// [`Response`]: crate::Service::Response
    /// [`poll_ready`]: crate::Service::poll_ready
    /// [`BoxError`]: crate::BoxError
    fn map_result<F, Response, Error>(self, f: F) -> MapResult<Self, F>
    where
        Self: Sized,
        Error: From<Self::Error>,
        F: FnOnce(Result<Self::Response, Self::Error>) -> Result<Response, Error> + Clone,
    {
        MapResult::new(self, f)
    }

    /// Composes a function *in front of* the service.
    ///
    /// This adapter produces a new service that passes each value through the
    /// given function `f` before sending it to `self`.
    ///
    /// # Example
    /// ```
    /// # use std::convert::TryFrom;
    /// # use std::task::{Poll, Context};
    /// # use tower::{Service, ServiceExt};
    /// #
    /// # struct DatabaseService;
    /// # impl DatabaseService {
    /// #   fn new(address: &str) -> Self {
    /// #       DatabaseService
    /// #   }
    /// # }
    /// #
    /// # impl Service<String> for DatabaseService {
    /// #   type Response = String;
    /// #   type Error = u8;
    /// #   type Future = futures_util::future::Ready<Result<String, u8>>;
    /// #
    /// #   fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    /// #       Poll::Ready(Ok(()))
    /// #   }
    /// #
    /// #   fn call(&mut self, request: String) -> Self::Future {
    /// #       futures_util::future::ready(Ok(String::new()))
    /// #   }
    /// # }
    /// #
    /// # fn main() {
    /// #   async {
    /// // A service taking a String as a request
    /// let service = DatabaseService::new("127.0.0.1:8080");
    ///
    /// // Map the request to a new request
    /// let mut new_service = service.map_request(|id: u32| id.to_string());
    ///
    /// // Call the new service
    /// let id = 13;
    /// let response = new_service
    ///     .ready()
    ///     .await?
    ///     .call(id)
    ///     .await;
    /// # response
    /// #    };
    /// # }
    /// ```
    fn map_request<F, NewRequest>(self, f: F) -> MapRequest<Self, F>
    where
        Self: Sized,
        F: FnMut(NewRequest) -> Request,
    {
        MapRequest::new(self, f)
    }

    /// Composes this service with a [`Filter`] that conditionally accepts or
    /// rejects requests based on a [predicate].
    ///
    /// This adapter produces a new service that passes each value through the
    /// given function `predicate` before sending it to `self`.
    ///
    /// # Example
    /// ```
    /// # use std::convert::TryFrom;
    /// # use std::task::{Poll, Context};
    /// # use tower::{Service, ServiceExt};
    /// #
    /// # struct DatabaseService;
    /// # impl DatabaseService {
    /// #   fn new(address: &str) -> Self {
    /// #       DatabaseService
    /// #   }
    /// # }
    /// #
    /// # #[derive(Debug)] enum DbError {
    /// #   Parse(std::num::ParseIntError)
    /// # }
    /// #
    /// # impl std::fmt::Display for DbError {
    /// #    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { std::fmt::Debug::fmt(self, f) }
    /// # }
    /// # impl std::error::Error for DbError {}
    /// # impl Service<u32> for DatabaseService {
    /// #   type Response = String;
    /// #   type Error = DbError;
    /// #   type Future = futures_util::future::Ready<Result<String, DbError>>;
    /// #
    /// #   fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    /// #       Poll::Ready(Ok(()))
    /// #   }
    /// #
    /// #   fn call(&mut self, request: u32) -> Self::Future {
    /// #       futures_util::future::ready(Ok(String::new()))
    /// #   }
    /// # }
    /// #
    /// # fn main() {
    /// #    async {
    /// // A service taking a u32 as a request and returning Result<_, DbError>
    /// let service = DatabaseService::new("127.0.0.1:8080");
    ///
    /// // Fallibly map the request to a new request
    /// let mut new_service = service
    ///     .filter(|id_str: &str| id_str.parse().map_err(DbError::Parse));
    ///
    /// // Call the new service
    /// let id = "13";
    /// let response = new_service
    ///     .ready()
    ///     .await?
    ///     .call(id)
    ///     .await;
    /// # response
    /// #    };
    /// # }
    /// ```
    ///
    /// [`Filter`]: crate::filter::Filter
    /// [predicate]: crate::filter::Predicate
    #[cfg(feature = "filter")]
    fn filter<F, NewRequest>(self, filter: F) -> crate::filter::Filter<Self, F>
    where
        Self: Sized,
        F: crate::filter::Predicate<NewRequest>,
    {
        crate::filter::Filter::new(self, filter)
    }

    /// Composes this service with an [`AsyncFilter`] that conditionally accepts or
    /// rejects requests based on an [async predicate].
    ///
    /// This adapter produces a new service that passes each value through the
    /// given function `predicate` before sending it to `self`.
    ///
    /// # Example
    /// ```
    /// # use std::convert::TryFrom;
    /// # use std::task::{Poll, Context};
    /// # use tower::{Service, ServiceExt};
    /// #
    /// # #[derive(Clone)] struct DatabaseService;
    /// # impl DatabaseService {
    /// #   fn new(address: &str) -> Self {
    /// #       DatabaseService
    /// #   }
    /// # }
    /// # #[derive(Debug)]
    /// # enum DbError {
    /// #   Rejected
    /// # }
    /// # impl std::fmt::Display for DbError {
    /// #    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { std::fmt::Debug::fmt(self, f) }
    /// # }
    /// # impl std::error::Error for DbError {}
    /// #
    /// # impl Service<u32> for DatabaseService {
    /// #   type Response = String;
    /// #   type Error = DbError;
    /// #   type Future = futures_util::future::Ready<Result<String, DbError>>;
    /// #
    /// #   fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    /// #       Poll::Ready(Ok(()))
    /// #   }
    /// #
    /// #   fn call(&mut self, request: u32) -> Self::Future {
    /// #       futures_util::future::ready(Ok(String::new()))
    /// #   }
    /// # }
    /// #
    /// # fn main() {
    /// #    async {
    /// // A service taking a u32 as a request and returning Result<_, DbError>
    /// let service = DatabaseService::new("127.0.0.1:8080");
    ///
    /// /// Returns `true` if we should query the database for an ID.
    /// async fn should_query(id: u32) -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// // Filter requests based on `should_query`.
    /// let mut new_service = service
    ///     .filter_async(|id: u32| async move {
    ///         if should_query(id).await {
    ///             return Ok(id);
    ///         }
    ///
    ///         Err(DbError::Rejected)
    ///     });
    ///
    /// // Call the new service
    /// let id = 13;
    /// # let id: u32 = id;
    /// let response = new_service
    ///     .ready()
    ///     .await?
    ///     .call(id)
    ///     .await;
    /// # response
    /// #    };
    /// # }
    /// ```
    ///
    /// [`AsyncFilter`]: crate::filter::AsyncFilter
    /// [asynchronous predicate]: crate::filter::AsyncPredicate
    #[cfg(feature = "filter")]
    fn filter_async<F, NewRequest>(self, filter: F) -> crate::filter::AsyncFilter<Self, F>
    where
        Self: Sized,
        F: crate::filter::AsyncPredicate<NewRequest>,
    {
        crate::filter::AsyncFilter::new(self, filter)
    }

    /// Composes an asynchronous function *after* this service.
    ///
    /// This takes a function or closure returning a future, and returns a new
    /// `Service` that chains that function after this service's [`Future`]. The
    /// new `Service`'s future will consist of this service's future, followed
    /// by the future returned by calling the chained function with the future's
    /// [`Output`] type. The chained function is called regardless of whether
    /// this service's future completes with a successful response or with an
    /// error.
    ///
    /// This method can be thought of as an equivalent to the [`futures`
    /// crate]'s [`FutureExt::then`] combinator, but acting on `Service`s that
    /// _return_ futures, rather than on an individual future. Similarly to that
    /// combinator, [`ServiceExt::then`] can be used to implement asynchronous
    /// error recovery, by calling some asynchronous function with errors
    /// returned by this service. Alternatively, it may also be used to call a
    /// fallible async function with the successful response of this service.
    ///
    /// This method can be used to change the [`Response`] type of the service
    /// into a different type. It can also be used to change the [`Error`] type
    /// of the service. However, because the `then` function is not applied
    /// to the errors returned by the service's [`poll_ready`] method, it must
    /// be possible to convert the service's [`Error`] type into the error type
    /// returned by the `then` future. This is trivial when the function
    /// returns the same error type as the service, but in other cases, it can
    /// be useful to use [`BoxError`] to erase differing error types.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::task::{Poll, Context};
    /// # use tower::{Service, ServiceExt};
    /// #
    /// # struct DatabaseService;
    /// # impl DatabaseService {
    /// #   fn new(address: &str) -> Self {
    /// #       DatabaseService
    /// #   }
    /// # }
    /// #
    /// # type Record = ();
    /// # type DbError = ();
    /// #
    /// # impl Service<u32> for DatabaseService {
    /// #   type Response = Record;
    /// #   type Error = DbError;
    /// #   type Future = futures_util::future::Ready<Result<Record, DbError>>;
    /// #
    /// #   fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    /// #       Poll::Ready(Ok(()))
    /// #   }
    /// #
    /// #   fn call(&mut self, request: u32) -> Self::Future {
    /// #       futures_util::future::ready(Ok(()))
    /// #   }
    /// # }
    /// #
    /// # fn main() {
    /// // A service returning Result<Record, DbError>
    /// let service = DatabaseService::new("127.0.0.1:8080");
    ///
    /// // An async function that attempts to recover from errors returned by the
    /// // database.
    /// async fn recover_from_error(error: DbError) -> Result<Record, DbError> {
    ///     // ...
    ///     # Ok(())
    /// }
    /// #    async {
    ///
    /// // If the database service returns an error, attempt to recover by
    /// // calling `recover_from_error`. Otherwise, return the successful response.
    /// let mut new_service = service.then(|result| async move {
    ///     match result {
    ///         Ok(record) => Ok(record),
    ///         Err(e) => recover_from_error(e).await,
    ///     }
    /// });
    ///
    /// // Call the new service
    /// let id = 13;
    /// let record = new_service
    ///     .ready()
    ///     .await?
    ///     .call(id)
    ///     .await?;
    /// # Ok::<(), DbError>(())
    /// #    };
    /// # }
    /// ```
    ///
    /// [`Future`]: crate::Service::Future
    /// [`Output`]: std::future::Future::Output
    /// [`futures` crate]: https://docs.rs/futures
    /// [`FutureExt::then`]: https://docs.rs/futures/latest/futures/future/trait.FutureExt.html#method.then
    /// [`Error`]: crate::Service::Error
    /// [`Response`]: crate::Service::Response
    /// [`poll_ready`]: crate::Service::poll_ready
    /// [`BoxError`]: crate::BoxError
    fn then<F, Response, Error, Fut>(self, f: F) -> Then<Self, F>
    where
        Self: Sized,
        Error: From<Self::Error>,
        F: FnOnce(Result<Self::Response, Self::Error>) -> Fut + Clone,
        Fut: Future<Output = Result<Response, Error>>,
    {
        Then::new(self, f)
    }

    /// Composes a function that transforms futures produced by the service.
    ///
    /// This takes a function or closure returning a future computed from the future returned by
    /// the service's [`call`] method, as opposed to the responses produced by the future.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::task::{Poll, Context};
    /// # use tower::{Service, ServiceExt, BoxError};
    /// #
    /// # struct DatabaseService;
    /// # impl DatabaseService {
    /// #   fn new(address: &str) -> Self {
    /// #       DatabaseService
    /// #   }
    /// # }
    /// #
    /// # type Record = ();
    /// # type DbError = crate::BoxError;
    /// #
    /// # impl Service<u32> for DatabaseService {
    /// #   type Response = Record;
    /// #   type Error = DbError;
    /// #   type Future = futures_util::future::Ready<Result<Record, DbError>>;
    /// #
    /// #   fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    /// #       Poll::Ready(Ok(()))
    /// #   }
    /// #
    /// #   fn call(&mut self, request: u32) -> Self::Future {
    /// #       futures_util::future::ready(Ok(()))
    /// #   }
    /// # }
    /// #
    /// # fn main() {
    /// use std::time::Duration;
    /// use tokio::time::timeout;
    ///
    /// // A service returning Result<Record, DbError>
    /// let service = DatabaseService::new("127.0.0.1:8080");
    /// #    async {
    ///
    /// let mut new_service = service.map_future(|future| async move {
    ///     let res = timeout(Duration::from_secs(1), future).await?;
    ///     Ok::<_, BoxError>(res)
    /// });
    ///
    /// // Call the new service
    /// let id = 13;
    /// let record = new_service
    ///     .ready()
    ///     .await?
    ///     .call(id)
    ///     .await?;
    /// # Ok::<(), BoxError>(())
    /// #    };
    /// # }
    /// ```
    ///
    /// Note that normally you wouldn't implement timeouts like this and instead use [`Timeout`].
    ///
    /// [`call`]: crate::Service::call
    /// [`Timeout`]: crate::timeout::Timeout
    fn map_future<F, Fut, Response, Error>(self, f: F) -> MapFuture<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Future) -> Fut,
        Error: From<Self::Error>,
        Fut: Future<Output = Result<Response, Error>>,
    {
        MapFuture::new(self, f)
    }

    /// Convert the service into a [`Service`] + [`Send`] trait object.
    ///
    /// See [`BoxService`] for more details.
    ///
    /// If `Self` implements the [`Clone`] trait, the [`boxed_clone`] method
    /// can be used instead, to produce a boxed service which will also
    /// implement [`Clone`].
    ///
    /// # Example
    ///
    /// ```
    /// use tower::{Service, ServiceExt, BoxError, service_fn, util::BoxService};
    /// #
    /// # struct Request;
    /// # struct Response;
    /// # impl Response {
    /// #     fn new() -> Self { Self }
    /// # }
    ///
    /// let service = service_fn(|req: Request| async {
    ///     Ok::<_, BoxError>(Response::new())
    /// });
    ///
    /// let service: BoxService<Request, Response, BoxError> = service
    ///     .map_request(|req| {
    ///         println!("received request");
    ///         req
    ///     })
    ///     .map_response(|res| {
    ///         println!("response produced");
    ///         res
    ///     })
    ///     .boxed();
    /// # let service = assert_service(service);
    /// # fn assert_service<S, R>(svc: S) -> S
    /// # where S: Service<R> { svc }
    /// ```
    ///
    /// [`Service`]: crate::Service
    /// [`boxed_clone`]: Self::boxed_clone
    fn boxed(self) -> BoxService<Request, Self::Response, Self::Error>
    where
        Self: Sized + Send + 'static,
        Self::Future: Send + 'static,
    {
        BoxService::new(self)
    }

    /// Convert the service into a [`Service`] + [`Clone`] + [`Send`] trait object.
    ///
    /// This is similar to the [`boxed`] method, but it requires that `Self` implement
    /// [`Clone`], and the returned boxed service implements [`Clone`].
    /// See [`BoxCloneService`] for more details.
    ///
    /// # Example
    ///
    /// ```
    /// use tower::{Service, ServiceExt, BoxError, service_fn, util::BoxCloneService};
    /// #
    /// # struct Request;
    /// # struct Response;
    /// # impl Response {
    /// #     fn new() -> Self { Self }
    /// # }
    ///
    /// let service = service_fn(|req: Request| async {
    ///     Ok::<_, BoxError>(Response::new())
    /// });
    ///
    /// let service: BoxCloneService<Request, Response, BoxError> = service
    ///     .map_request(|req| {
    ///         println!("received request");
    ///         req
    ///     })
    ///     .map_response(|res| {
    ///         println!("response produced");
    ///         res
    ///     })
    ///     .boxed_clone();
    ///
    /// // The boxed service can still be cloned.
    /// service.clone();
    /// # let service = assert_service(service);
    /// # fn assert_service<S, R>(svc: S) -> S
    /// # where S: Service<R> { svc }
    /// ```
    ///
    /// [`Service`]: crate::Service
    /// [`boxed`]: Self::boxed
    fn boxed_clone(self) -> BoxCloneService<Request, Self::Response, Self::Error>
    where
        Self: Clone + Sized + Send + 'static,
        Self::Future: Send + 'static,
    {
        BoxCloneService::new(self)
    }
}

impl<T: ?Sized, Request> ServiceExt<Request> for T where T: tower_service::Service<Request> {}

/// Convert an `Option<Layer>` into a [`Layer`].
///
/// ```
/// # use std::time::Duration;
/// # use tower::Service;
/// # use tower::builder::ServiceBuilder;
/// use tower::util::option_layer;
/// # use tower::timeout::TimeoutLayer;
/// # async fn wrap<S>(svc: S) where S: Service<(), Error = &'static str> + 'static + Send, S::Future: Send {
/// # let timeout = Some(Duration::new(10, 0));
/// // Layer to apply a timeout if configured
/// let maybe_timeout = option_layer(timeout.map(TimeoutLayer::new));
///
/// ServiceBuilder::new()
///     .layer(maybe_timeout)
///     .service(svc);
/// # }
/// ```
///
/// [`Layer`]: crate::layer::Layer
pub fn option_layer<L>(layer: Option<L>) -> Either<L, Identity> {
    if let Some(layer) = layer {
        Either::Left(layer)
    } else {
        Either::Right(Identity::new())
    }
}
