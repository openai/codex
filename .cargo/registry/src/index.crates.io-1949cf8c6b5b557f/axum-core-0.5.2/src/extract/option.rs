use std::future::Future;

use http::request::Parts;

use crate::response::IntoResponse;

use super::{private, FromRequest, FromRequestParts, Request};

/// Customize the behavior of `Option<Self>` as a [`FromRequestParts`]
/// extractor.
pub trait OptionalFromRequestParts<S>: Sized {
    /// If the extractor fails, it will use this "rejection" type.
    ///
    /// A rejection is a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> impl Future<Output = Result<Option<Self>, Self::Rejection>> + Send;
}

/// Customize the behavior of `Option<Self>` as a [`FromRequest`] extractor.
pub trait OptionalFromRequest<S, M = private::ViaRequest>: Sized {
    /// If the extractor fails, it will use this "rejection" type.
    ///
    /// A rejection is a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    fn from_request(
        req: Request,
        state: &S,
    ) -> impl Future<Output = Result<Option<Self>, Self::Rejection>> + Send;
}

impl<S, T> FromRequestParts<S> for Option<T>
where
    T: OptionalFromRequestParts<S>,
    S: Send + Sync,
{
    type Rejection = T::Rejection;

    fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> impl Future<Output = Result<Option<T>, Self::Rejection>> {
        T::from_request_parts(parts, state)
    }
}

impl<S, T> FromRequest<S> for Option<T>
where
    T: OptionalFromRequest<S>,
    S: Send + Sync,
{
    type Rejection = T::Rejection;

    async fn from_request(req: Request, state: &S) -> Result<Option<T>, Self::Rejection> {
        T::from_request(req, state).await
    }
}
