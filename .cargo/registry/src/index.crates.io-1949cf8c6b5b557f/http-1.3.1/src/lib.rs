//! A general purpose library of common HTTP types
//!
//! This crate is a general purpose library for common types found when working
//! with the HTTP protocol. You'll find [`Request`] and [`Response`] types for
//! working as either a client or a server as well as all of their components.
//! Notably you'll find `Uri` for what a [`Request`] is requesting, a [`Method`]
//! for how it's being requested, a [`StatusCode`] for what sort of response came
//! back, a [`Version`] for how this was communicated, and
//! [`HeaderName`]/[`HeaderValue`] definitions to get grouped in a [`HeaderMap`] to
//! work with request/response headers.
//!
//! You will notably *not* find an implementation of sending requests or
//! spinning up a server in this crate. It's intended that this crate is the
//! "standard library" for HTTP clients and servers without dictating any
//! particular implementation.
//!
//! ## Requests and Responses
//!
//! Perhaps the main two types in this crate are the [`Request`] and [`Response`]
//! types. A [`Request`] could either be constructed to get sent off as a client
//! or it can also be received to generate a [`Response`] for a server. Similarly
//! as a client a [`Response`] is what you get after sending a [`Request`], whereas
//! on a server you'll be manufacturing a [`Response`] to send back to the client.
//!
//! Each type has a number of accessors for the component fields. For as a
//! server you might want to inspect a requests URI to dispatch it:
//!
//! ```
//! use http::{Request, Response};
//!
//! fn response(req: Request<()>) -> http::Result<Response<()>> {
//!     match req.uri().path() {
//!         "/" => index(req),
//!         "/foo" => foo(req),
//!         "/bar" => bar(req),
//!         _ => not_found(req),
//!     }
//! }
//! # fn index(_req: Request<()>) -> http::Result<Response<()>> { panic!() }
//! # fn foo(_req: Request<()>) -> http::Result<Response<()>> { panic!() }
//! # fn bar(_req: Request<()>) -> http::Result<Response<()>> { panic!() }
//! # fn not_found(_req: Request<()>) -> http::Result<Response<()>> { panic!() }
//! ```
//!
//! On a [`Request`] you'll also find accessors like [`method`][Request::method] to return a
//! [`Method`] and [`headers`][Request::method] to inspect the various headers. A [`Response`]
//! has similar methods for headers, the status code, etc.
//!
//! In addition to getters, request/response types also have mutable accessors
//! to edit the request/response:
//!
//! ```
//! use http::{HeaderValue, Response, StatusCode};
//! use http::header::CONTENT_TYPE;
//!
//! fn add_server_headers<T>(response: &mut Response<T>) {
//!     response.headers_mut()
//!         .insert(CONTENT_TYPE, HeaderValue::from_static("text/html"));
//!     *response.status_mut() = StatusCode::OK;
//! }
//! ```
//!
//! And finally, one of the most important aspects of requests/responses, the
//! body! The [`Request`] and [`Response`] types in this crate are *generic* in
//! what their body is. This allows downstream libraries to use different
//! representations such as `Request<Vec<u8>>`, `Response<impl Read>`,
//! `Request<impl Stream<Item = Vec<u8>, Error = _>>`, or even
//! `Response<MyCustomType>` where the custom type was deserialized from JSON.
//!
//! The body representation is intentionally flexible to give downstream
//! libraries maximal flexibility in implementing the body as appropriate.
//!
//! ## HTTP Headers
//!
//! Another major piece of functionality in this library is HTTP header
//! interpretation and generation. The `HeaderName` type serves as a way to
//! define header *names*, or what's to the left of the colon. A `HeaderValue`
//! conversely is the header *value*, or what's to the right of a colon.
//!
//! For example, if you have an HTTP request that looks like:
//!
//! ```http
//! GET /foo HTTP/1.1
//! Accept: text/html
//! ```
//!
//! Then `"Accept"` is a [`HeaderName`] while `"text/html"` is a [`HeaderValue`].
//! Each of these is a dedicated type to allow for a number of interesting
//! optimizations and to also encode the static guarantees of each type. For
//! example a [`HeaderName`] is always a valid `&str`, but a [`HeaderValue`] may
//! not be valid UTF-8.
//!
//! The most common header names are already defined for you as constant values
//! in the [`header`] module of this crate. For example:
//!
//! ```
//! use http::header::{self, HeaderName};
//!
//! let name: HeaderName = header::ACCEPT;
//! assert_eq!(name.as_str(), "accept");
//! ```
//!
//! You can, however, also parse header names from strings:
//!
//! ```
//! use http::header::{self, HeaderName};
//!
//! let name = "Accept".parse::<HeaderName>().unwrap();
//! assert_eq!(name, header::ACCEPT);
//! ```
//!
//! Header values can be created from string literals through the [`from_static`][header::HeaderValue::from_static]
//! function:
//!
//! ```
//! use http::HeaderValue;
//!
//! let value = HeaderValue::from_static("text/html");
//! assert_eq!(value.as_bytes(), b"text/html");
//! ```
//!
//! And header values can also be parsed like names:
//!
//! ```
//! use http::HeaderValue;
//!
//! let value = "text/html";
//! let value = value.parse::<HeaderValue>().unwrap();
//! ```
//!
//! Most HTTP requests and responses tend to come with more than one header, so
//! it's not too useful to just work with names and values only! This crate also
//! provides a [`HeaderMap`] type which is a specialized hash map for keys as
//! [`HeaderName`] and generic values. This type, like header names, is optimized
//! for common usage but should continue to scale with your needs over time.
//!
//! # URIs
//!
//! Each HTTP [`Request`] has an associated URI with it. This may just be a path
//! like `/index.html` but it could also be an absolute URL such as
//! `https://www.rust-lang.org/index.html`. A [`URI`][uri::Uri] has a number of accessors to
//! interpret it:
//!
//! ```
//! use http::Uri;
//! use http::uri::Scheme;
//!
//! let uri = "https://www.rust-lang.org/index.html".parse::<Uri>().unwrap();
//!
//! assert_eq!(uri.scheme(), Some(&Scheme::HTTPS));
//! assert_eq!(uri.host(), Some("www.rust-lang.org"));
//! assert_eq!(uri.path(), "/index.html");
//! assert_eq!(uri.query(), None);
//! ```

#![deny(warnings, missing_docs, missing_debug_implementations)]

//#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(not(feature = "std"))]
compile_error!("`std` feature currently required, support for `no_std` may be added later");

#[cfg(test)]
#[macro_use]
extern crate doc_comment;

#[cfg(test)]
doctest!("../README.md");

#[macro_use]
mod convert;

pub mod header;
pub mod method;
pub mod request;
pub mod response;
pub mod status;
pub mod uri;
pub mod version;

mod byte_str;
mod error;
mod extensions;

pub use crate::error::{Error, Result};
pub use crate::extensions::Extensions;
#[doc(no_inline)]
pub use crate::header::{HeaderMap, HeaderName, HeaderValue};
pub use crate::method::Method;
pub use crate::request::Request;
pub use crate::response::Response;
pub use crate::status::StatusCode;
pub use crate::uri::Uri;
pub use crate::version::Version;

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn request_satisfies_send_sync() {
        assert_send_sync::<Request<()>>();
    }

    #[test]
    fn response_satisfies_send_sync() {
        assert_send_sync::<Response<()>>();
    }
}
