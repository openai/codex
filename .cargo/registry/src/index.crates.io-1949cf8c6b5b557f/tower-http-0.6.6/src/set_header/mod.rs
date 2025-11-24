//! Middleware for setting headers on requests and responses.
//!
//! See [request] and [response] for more details.

use http::{header::HeaderName, HeaderMap, HeaderValue, Request, Response};

pub mod request;
pub mod response;

#[doc(inline)]
pub use self::{
    request::{SetRequestHeader, SetRequestHeaderLayer},
    response::{SetResponseHeader, SetResponseHeaderLayer},
};

/// Trait for producing header values.
///
/// Used by [`SetRequestHeader`] and [`SetResponseHeader`].
///
/// This trait is implemented for closures with the correct type signature. Typically users will
/// not have to implement this trait for their own types.
///
/// It is also implemented directly for [`HeaderValue`]. When a fixed header value should be added
/// to all responses, it can be supplied directly to the middleware.
pub trait MakeHeaderValue<T> {
    /// Try to create a header value from the request or response.
    fn make_header_value(&mut self, message: &T) -> Option<HeaderValue>;
}

impl<F, T> MakeHeaderValue<T> for F
where
    F: FnMut(&T) -> Option<HeaderValue>,
{
    fn make_header_value(&mut self, message: &T) -> Option<HeaderValue> {
        self(message)
    }
}

impl<T> MakeHeaderValue<T> for HeaderValue {
    fn make_header_value(&mut self, _message: &T) -> Option<HeaderValue> {
        Some(self.clone())
    }
}

impl<T> MakeHeaderValue<T> for Option<HeaderValue> {
    fn make_header_value(&mut self, _message: &T) -> Option<HeaderValue> {
        self.clone()
    }
}

#[derive(Debug, Clone, Copy)]
enum InsertHeaderMode {
    Override,
    Append,
    IfNotPresent,
}

impl InsertHeaderMode {
    fn apply<T, M>(self, header_name: &HeaderName, target: &mut T, make: &mut M)
    where
        T: Headers,
        M: MakeHeaderValue<T>,
    {
        match self {
            InsertHeaderMode::Override => {
                if let Some(value) = make.make_header_value(target) {
                    target.headers_mut().insert(header_name.clone(), value);
                }
            }
            InsertHeaderMode::IfNotPresent => {
                if !target.headers().contains_key(header_name) {
                    if let Some(value) = make.make_header_value(target) {
                        target.headers_mut().insert(header_name.clone(), value);
                    }
                }
            }
            InsertHeaderMode::Append => {
                if let Some(value) = make.make_header_value(target) {
                    target.headers_mut().append(header_name.clone(), value);
                }
            }
        }
    }
}

trait Headers {
    fn headers(&self) -> &HeaderMap;

    fn headers_mut(&mut self) -> &mut HeaderMap;
}

impl<B> Headers for Request<B> {
    fn headers(&self) -> &HeaderMap {
        Request::headers(self)
    }

    fn headers_mut(&mut self) -> &mut HeaderMap {
        Request::headers_mut(self)
    }
}

impl<B> Headers for Response<B> {
    fn headers(&self) -> &HeaderMap {
        Response::headers(self)
    }

    fn headers_mut(&mut self) -> &mut HeaderMap {
        Response::headers_mut(self)
    }
}
