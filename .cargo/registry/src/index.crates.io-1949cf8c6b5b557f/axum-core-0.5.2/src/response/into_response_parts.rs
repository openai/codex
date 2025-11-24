use super::{IntoResponse, Response};
use http::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Extensions, StatusCode,
};
use std::{convert::Infallible, fmt};

/// Trait for adding headers and extensions to a response.
///
/// # Example
///
/// ```rust
/// use axum::{
///     response::{ResponseParts, IntoResponse, IntoResponseParts, Response},
///     http::{StatusCode, header::{HeaderName, HeaderValue}},
/// };
///
/// // Hypothetical helper type for setting a single header
/// struct SetHeader<'a>(&'a str, &'a str);
///
/// impl<'a> IntoResponseParts for SetHeader<'a> {
///     type Error = (StatusCode, String);
///
///     fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
///         match (self.0.parse::<HeaderName>(), self.1.parse::<HeaderValue>()) {
///             (Ok(name), Ok(value)) => {
///                 res.headers_mut().insert(name, value);
///             },
///             (Err(_), _) => {
///                 return Err((
///                     StatusCode::INTERNAL_SERVER_ERROR,
///                     format!("Invalid header name {}", self.0),
///                 ));
///             },
///             (_, Err(_)) => {
///                 return Err((
///                     StatusCode::INTERNAL_SERVER_ERROR,
///                     format!("Invalid header value {}", self.1),
///                 ));
///             },
///         }
///
///         Ok(res)
///     }
/// }
///
/// // It's also recommended to implement `IntoResponse` so `SetHeader` can be used on its own as
/// // the response
/// impl<'a> IntoResponse for SetHeader<'a> {
///     fn into_response(self) -> Response {
///         // This gives an empty response with the header
///         (self, ()).into_response()
///     }
/// }
///
/// // We can now return `SetHeader` in responses
/// //
/// // Note that returning `impl IntoResponse` might be easier if the response has many parts to
/// // it. The return type is written out here for clarity.
/// async fn handler() -> (SetHeader<'static>, SetHeader<'static>, &'static str) {
///     (
///         SetHeader("server", "axum"),
///         SetHeader("x-foo", "custom"),
///         "body",
///     )
/// }
///
/// // Or on its own as the whole response
/// async fn other_handler() -> SetHeader<'static> {
///     SetHeader("x-foo", "custom")
/// }
/// ```
pub trait IntoResponseParts {
    /// The type returned in the event of an error.
    ///
    /// This can be used to fallibly convert types into headers or extensions.
    type Error: IntoResponse;

    /// Set parts of the response
    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error>;
}

impl<T> IntoResponseParts for Option<T>
where
    T: IntoResponseParts,
{
    type Error = T::Error;

    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        if let Some(inner) = self {
            inner.into_response_parts(res)
        } else {
            Ok(res)
        }
    }
}

/// Parts of a response.
///
/// Used with [`IntoResponseParts`].
#[derive(Debug)]
pub struct ResponseParts {
    pub(crate) res: Response,
}

impl ResponseParts {
    /// Gets a reference to the response headers.
    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        self.res.headers()
    }

    /// Gets a mutable reference to the response headers.
    #[must_use]
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        self.res.headers_mut()
    }

    /// Gets a reference to the response extensions.
    #[must_use]
    pub fn extensions(&self) -> &Extensions {
        self.res.extensions()
    }

    /// Gets a mutable reference to the response extensions.
    #[must_use]
    pub fn extensions_mut(&mut self) -> &mut Extensions {
        self.res.extensions_mut()
    }
}

impl IntoResponseParts for HeaderMap {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        res.headers_mut().extend(self);
        Ok(res)
    }
}

impl<K, V, const N: usize> IntoResponseParts for [(K, V); N]
where
    K: TryInto<HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: fmt::Display,
{
    type Error = TryIntoHeaderError<K::Error, V::Error>;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        for (key, value) in self {
            let key = key.try_into().map_err(TryIntoHeaderError::key)?;
            let value = value.try_into().map_err(TryIntoHeaderError::value)?;
            res.headers_mut().insert(key, value);
        }

        Ok(res)
    }
}

/// Error returned if converting a value to a header fails.
#[derive(Debug)]
pub struct TryIntoHeaderError<K, V> {
    kind: TryIntoHeaderErrorKind<K, V>,
}

impl<K, V> TryIntoHeaderError<K, V> {
    pub(super) fn key(err: K) -> Self {
        Self {
            kind: TryIntoHeaderErrorKind::Key(err),
        }
    }

    pub(super) fn value(err: V) -> Self {
        Self {
            kind: TryIntoHeaderErrorKind::Value(err),
        }
    }
}

#[derive(Debug)]
enum TryIntoHeaderErrorKind<K, V> {
    Key(K),
    Value(V),
}

impl<K, V> IntoResponse for TryIntoHeaderError<K, V>
where
    K: fmt::Display,
    V: fmt::Display,
{
    fn into_response(self) -> Response {
        match self.kind {
            TryIntoHeaderErrorKind::Key(inner) => {
                (StatusCode::INTERNAL_SERVER_ERROR, inner.to_string()).into_response()
            }
            TryIntoHeaderErrorKind::Value(inner) => {
                (StatusCode::INTERNAL_SERVER_ERROR, inner.to_string()).into_response()
            }
        }
    }
}

impl<K, V> fmt::Display for TryIntoHeaderError<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            TryIntoHeaderErrorKind::Key(_) => write!(f, "failed to convert key to a header name"),
            TryIntoHeaderErrorKind::Value(_) => {
                write!(f, "failed to convert value to a header value")
            }
        }
    }
}

impl<K, V> std::error::Error for TryIntoHeaderError<K, V>
where
    K: std::error::Error + 'static,
    V: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            TryIntoHeaderErrorKind::Key(inner) => Some(inner),
            TryIntoHeaderErrorKind::Value(inner) => Some(inner),
        }
    }
}

macro_rules! impl_into_response_parts {
    ( $($ty:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        impl<$($ty,)*> IntoResponseParts for ($($ty,)*)
        where
            $( $ty: IntoResponseParts, )*
        {
            type Error = Response;

            fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error> {
                let ($($ty,)*) = self;

                $(
                    let res = match $ty.into_response_parts(res) {
                        Ok(res) => res,
                        Err(err) => {
                            return Err(err.into_response());
                        }
                    };
                )*

                Ok(res)
            }
        }
    }
}

all_the_tuples_no_last_special_case!(impl_into_response_parts);

impl IntoResponseParts for Extensions {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        res.extensions_mut().extend(self);
        Ok(res)
    }
}

impl IntoResponseParts for () {
    type Error = Infallible;

    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        Ok(res)
    }
}
