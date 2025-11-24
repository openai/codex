use super::{IntoResponseParts, Response, ResponseParts};
use crate::{body::Body, BoxError};
use bytes::{buf::Chain, Buf, Bytes, BytesMut};
use http::{
    header::{self, HeaderMap, HeaderName, HeaderValue},
    Extensions, StatusCode,
};
use http_body::{Frame, SizeHint};
use std::{
    borrow::Cow,
    convert::Infallible,
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

/// Trait for generating responses.
///
/// Types that implement `IntoResponse` can be returned from handlers.
///
/// # Implementing `IntoResponse`
///
/// You generally shouldn't have to implement `IntoResponse` manually, as axum
/// provides implementations for many common types.
///
/// However it might be necessary if you have a custom error type that you want
/// to return from handlers:
///
/// ```rust
/// use axum::{
///     Router,
///     body::{self, Bytes},
///     routing::get,
///     http::StatusCode,
///     response::{IntoResponse, Response},
/// };
///
/// enum MyError {
///     SomethingWentWrong,
///     SomethingElseWentWrong,
/// }
///
/// impl IntoResponse for MyError {
///     fn into_response(self) -> Response {
///         let body = match self {
///             MyError::SomethingWentWrong => "something went wrong",
///             MyError::SomethingElseWentWrong => "something else went wrong",
///         };
///
///         // it's often easiest to implement `IntoResponse` by calling other implementations
///         (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
///     }
/// }
///
/// // `Result<impl IntoResponse, MyError>` can now be returned from handlers
/// let app = Router::new().route("/", get(handler));
///
/// async fn handler() -> Result<(), MyError> {
///     Err(MyError::SomethingWentWrong)
/// }
/// # let _: Router = app;
/// ```
///
/// Or if you have a custom body type you'll also need to implement
/// `IntoResponse` for it:
///
/// ```rust
/// use axum::{
///     body,
///     routing::get,
///     response::{IntoResponse, Response},
///     body::Body,
///     Router,
/// };
/// use http::HeaderMap;
/// use bytes::Bytes;
/// use http_body::Frame;
/// use std::{
///     convert::Infallible,
///     task::{Poll, Context},
///     pin::Pin,
/// };
///
/// struct MyBody;
///
/// // First implement `Body` for `MyBody`. This could for example use
/// // some custom streaming protocol.
/// impl http_body::Body for MyBody {
///     type Data = Bytes;
///     type Error = Infallible;
///
///     fn poll_frame(
///         self: Pin<&mut Self>,
///         cx: &mut Context<'_>,
///     ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
///         # unimplemented!()
///         // ...
///     }
/// }
///
/// // Now we can implement `IntoResponse` directly for `MyBody`
/// impl IntoResponse for MyBody {
///     fn into_response(self) -> Response {
///         Response::new(Body::new(self))
///     }
/// }
///
/// // `MyBody` can now be returned from handlers.
/// let app = Router::new().route("/", get(|| async { MyBody }));
/// # let _: Router = app;
/// ```
pub trait IntoResponse {
    /// Create a response.
    #[must_use]
    fn into_response(self) -> Response;
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.status_mut() = self;
        res
    }
}

impl IntoResponse for () {
    fn into_response(self) -> Response {
        Body::empty().into_response()
    }
}

impl IntoResponse for Infallible {
    fn into_response(self) -> Response {
        match self {}
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            Ok(value) => value.into_response(),
            Err(err) => err.into_response(),
        }
    }
}

impl<B> IntoResponse for Response<B>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    fn into_response(self) -> Response {
        self.map(Body::new)
    }
}

impl IntoResponse for http::response::Parts {
    fn into_response(self) -> Response {
        Response::from_parts(self, Body::empty())
    }
}

impl IntoResponse for Body {
    fn into_response(self) -> Response {
        Response::new(self)
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        Cow::Borrowed(self).into_response()
    }
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        Cow::<'static, str>::Owned(self).into_response()
    }
}

impl IntoResponse for Box<str> {
    fn into_response(self) -> Response {
        String::from(self).into_response()
    }
}

impl IntoResponse for Cow<'static, str> {
    fn into_response(self) -> Response {
        let mut res = Body::from(self).into_response();
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
        );
        res
    }
}

impl IntoResponse for Bytes {
    fn into_response(self) -> Response {
        let mut res = Body::from(self).into_response();
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl IntoResponse for BytesMut {
    fn into_response(self) -> Response {
        self.freeze().into_response()
    }
}

impl<T, U> IntoResponse for Chain<T, U>
where
    T: Buf + Unpin + Send + 'static,
    U: Buf + Unpin + Send + 'static,
{
    fn into_response(self) -> Response {
        let (first, second) = self.into_inner();
        let mut res = Response::new(Body::new(BytesChainBody {
            first: Some(first),
            second: Some(second),
        }));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

struct BytesChainBody<T, U> {
    first: Option<T>,
    second: Option<U>,
}

impl<T, U> http_body::Body for BytesChainBody<T, U>
where
    T: Buf + Unpin,
    U: Buf + Unpin,
{
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if let Some(mut buf) = self.first.take() {
            let bytes = buf.copy_to_bytes(buf.remaining());
            return Poll::Ready(Some(Ok(Frame::data(bytes))));
        }

        if let Some(mut buf) = self.second.take() {
            let bytes = buf.copy_to_bytes(buf.remaining());
            return Poll::Ready(Some(Ok(Frame::data(bytes))));
        }

        Poll::Ready(None)
    }

    fn is_end_stream(&self) -> bool {
        self.first.is_none() && self.second.is_none()
    }

    fn size_hint(&self) -> SizeHint {
        match (self.first.as_ref(), self.second.as_ref()) {
            (Some(first), Some(second)) => {
                let total_size = first.remaining() + second.remaining();
                SizeHint::with_exact(total_size as u64)
            }
            (Some(buf), None) => SizeHint::with_exact(buf.remaining() as u64),
            (None, Some(buf)) => SizeHint::with_exact(buf.remaining() as u64),
            (None, None) => SizeHint::with_exact(0),
        }
    }
}

impl IntoResponse for &'static [u8] {
    fn into_response(self) -> Response {
        Cow::Borrowed(self).into_response()
    }
}

impl<const N: usize> IntoResponse for &'static [u8; N] {
    fn into_response(self) -> Response {
        self.as_slice().into_response()
    }
}

impl<const N: usize> IntoResponse for [u8; N] {
    fn into_response(self) -> Response {
        self.to_vec().into_response()
    }
}

impl IntoResponse for Vec<u8> {
    fn into_response(self) -> Response {
        Cow::<'static, [u8]>::Owned(self).into_response()
    }
}

impl IntoResponse for Box<[u8]> {
    fn into_response(self) -> Response {
        Vec::from(self).into_response()
    }
}

impl IntoResponse for Cow<'static, [u8]> {
    fn into_response(self) -> Response {
        let mut res = Body::from(self).into_response();
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl<R> IntoResponse for (StatusCode, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl IntoResponse for HeaderMap {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.headers_mut() = self;
        res
    }
}

impl IntoResponse for Extensions {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.extensions_mut() = self;
        res
    }
}

impl<K, V, const N: usize> IntoResponse for [(K, V); N]
where
    K: TryInto<HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<HeaderValue>,
    V::Error: fmt::Display,
{
    fn into_response(self) -> Response {
        (self, ()).into_response()
    }
}

impl<R> IntoResponse for (http::response::Parts, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let (parts, res) = self;
        (parts.status, parts.headers, parts.extensions, res).into_response()
    }
}

impl<R> IntoResponse for (http::response::Response<()>, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let (template, res) = self;
        let (parts, ()) = template.into_parts();
        (parts, res).into_response()
    }
}

impl<R> IntoResponse for (R,)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let (res,) = self;
        res.into_response()
    }
}

macro_rules! impl_into_response {
    ( $($ty:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        impl<R, $($ty,)*> IntoResponse for ($($ty),*, R)
        where
            $( $ty: IntoResponseParts, )*
            R: IntoResponse,
        {
            fn into_response(self) -> Response {
                let ($($ty),*, res) = self;

                let res = res.into_response();
                let parts = ResponseParts { res };

                $(
                    let parts = match $ty.into_response_parts(parts) {
                        Ok(parts) => parts,
                        Err(err) => {
                            return err.into_response();
                        }
                    };
                )*

                parts.res
            }
        }

        #[allow(non_snake_case)]
        impl<R, $($ty,)*> IntoResponse for (StatusCode, $($ty),*, R)
        where
            $( $ty: IntoResponseParts, )*
            R: IntoResponse,
        {
            fn into_response(self) -> Response {
                let (status, $($ty),*, res) = self;

                let res = res.into_response();
                let parts = ResponseParts { res };

                $(
                    let parts = match $ty.into_response_parts(parts) {
                        Ok(parts) => parts,
                        Err(err) => {
                            return err.into_response();
                        }
                    };
                )*

                (status, parts.res).into_response()
            }
        }

        #[allow(non_snake_case)]
        impl<R, $($ty,)*> IntoResponse for (http::response::Parts, $($ty),*, R)
        where
            $( $ty: IntoResponseParts, )*
            R: IntoResponse,
        {
            fn into_response(self) -> Response {
                let (outer_parts, $($ty),*, res) = self;

                let res = res.into_response();
                let parts = ResponseParts { res };
                $(
                    let parts = match $ty.into_response_parts(parts) {
                        Ok(parts) => parts,
                        Err(err) => {
                            return err.into_response();
                        }
                    };
                )*

                (outer_parts, parts.res).into_response()
            }
        }

        #[allow(non_snake_case)]
        impl<R, $($ty,)*> IntoResponse for (http::response::Response<()>, $($ty),*, R)
        where
            $( $ty: IntoResponseParts, )*
            R: IntoResponse,
        {
            fn into_response(self) -> Response {
                let (template, $($ty),*, res) = self;
                let (parts, ()) = template.into_parts();
                (parts, $($ty),*, res).into_response()
            }
        }
    }
}

all_the_tuples_no_last_special_case!(impl_into_response);
