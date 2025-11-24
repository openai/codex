//! Predicates for disabling compression of responses.
//!
//! Predicates are applied with [`Compression::compress_when`] or
//! [`CompressionLayer::compress_when`].
//!
//! [`Compression::compress_when`]: super::Compression::compress_when
//! [`CompressionLayer::compress_when`]: super::CompressionLayer::compress_when

use http::{header, Extensions, HeaderMap, StatusCode, Version};
use http_body::Body;
use std::{fmt, sync::Arc};

/// Predicate used to determine if a response should be compressed or not.
pub trait Predicate: Clone {
    /// Should this response be compressed or not?
    fn should_compress<B>(&self, response: &http::Response<B>) -> bool
    where
        B: Body;

    /// Combine two predicates into one.
    ///
    /// The resulting predicate enables compression if both inner predicates do.
    fn and<Other>(self, other: Other) -> And<Self, Other>
    where
        Self: Sized,
        Other: Predicate,
    {
        And {
            lhs: self,
            rhs: other,
        }
    }
}

impl<F> Predicate for F
where
    F: Fn(StatusCode, Version, &HeaderMap, &Extensions) -> bool + Clone,
{
    fn should_compress<B>(&self, response: &http::Response<B>) -> bool
    where
        B: Body,
    {
        let status = response.status();
        let version = response.version();
        let headers = response.headers();
        let extensions = response.extensions();
        self(status, version, headers, extensions)
    }
}

impl<T> Predicate for Option<T>
where
    T: Predicate,
{
    fn should_compress<B>(&self, response: &http::Response<B>) -> bool
    where
        B: Body,
    {
        self.as_ref()
            .map(|inner| inner.should_compress(response))
            .unwrap_or(true)
    }
}

/// Two predicates combined into one.
///
/// Created with [`Predicate::and`]
#[derive(Debug, Clone, Default, Copy)]
pub struct And<Lhs, Rhs> {
    lhs: Lhs,
    rhs: Rhs,
}

impl<Lhs, Rhs> Predicate for And<Lhs, Rhs>
where
    Lhs: Predicate,
    Rhs: Predicate,
{
    fn should_compress<B>(&self, response: &http::Response<B>) -> bool
    where
        B: Body,
    {
        self.lhs.should_compress(response) && self.rhs.should_compress(response)
    }
}

/// The default predicate used by [`Compression`] and [`CompressionLayer`].
///
/// This will compress responses unless:
///
/// - They're gRPC, which has its own protocol specific compression scheme.
/// - It's an image as determined by the `content-type` starting with `image/`.
/// - They're Server-Sent Events (SSE) as determined by the `content-type` being `text/event-stream`.
/// - The response is less than 32 bytes.
///
/// # Configuring the defaults
///
/// `DefaultPredicate` doesn't support any configuration. Instead you can build your own predicate
/// by combining types in this module:
///
/// ```rust
/// use tower_http::compression::predicate::{SizeAbove, NotForContentType, Predicate};
///
/// // slightly large min size than the default 32
/// let predicate = SizeAbove::new(256)
///     // still don't compress gRPC
///     .and(NotForContentType::GRPC)
///     // still don't compress images
///     .and(NotForContentType::IMAGES)
///     // also don't compress JSON
///     .and(NotForContentType::const_new("application/json"));
/// ```
///
/// [`Compression`]: super::Compression
/// [`CompressionLayer`]: super::CompressionLayer
#[derive(Clone)]
pub struct DefaultPredicate(
    And<And<And<SizeAbove, NotForContentType>, NotForContentType>, NotForContentType>,
);

impl DefaultPredicate {
    /// Create a new `DefaultPredicate`.
    pub fn new() -> Self {
        let inner = SizeAbove::new(SizeAbove::DEFAULT_MIN_SIZE)
            .and(NotForContentType::GRPC)
            .and(NotForContentType::IMAGES)
            .and(NotForContentType::SSE);
        Self(inner)
    }
}

impl Default for DefaultPredicate {
    fn default() -> Self {
        Self::new()
    }
}

impl Predicate for DefaultPredicate {
    fn should_compress<B>(&self, response: &http::Response<B>) -> bool
    where
        B: Body,
    {
        self.0.should_compress(response)
    }
}

/// [`Predicate`] that will only allow compression of responses above a certain size.
#[derive(Clone, Copy, Debug)]
pub struct SizeAbove(u16);

impl SizeAbove {
    pub(crate) const DEFAULT_MIN_SIZE: u16 = 32;

    /// Create a new `SizeAbove` predicate that will only compress responses larger than
    /// `min_size_bytes`.
    ///
    /// The response will be compressed if the exact size cannot be determined through either the
    /// `content-length` header or [`Body::size_hint`].
    pub const fn new(min_size_bytes: u16) -> Self {
        Self(min_size_bytes)
    }
}

impl Default for SizeAbove {
    fn default() -> Self {
        Self(Self::DEFAULT_MIN_SIZE)
    }
}

impl Predicate for SizeAbove {
    fn should_compress<B>(&self, response: &http::Response<B>) -> bool
    where
        B: Body,
    {
        let content_size = response.body().size_hint().exact().or_else(|| {
            response
                .headers()
                .get(header::CONTENT_LENGTH)
                .and_then(|h| h.to_str().ok())
                .and_then(|val| val.parse().ok())
        });

        match content_size {
            Some(size) => size >= (self.0 as u64),
            _ => true,
        }
    }
}

/// Predicate that wont allow responses with a specific `content-type` to be compressed.
#[derive(Clone, Debug)]
pub struct NotForContentType {
    content_type: Str,
    exception: Option<Str>,
}

impl NotForContentType {
    /// Predicate that wont compress gRPC responses.
    pub const GRPC: Self = Self::const_new("application/grpc");

    /// Predicate that wont compress images.
    pub const IMAGES: Self = Self {
        content_type: Str::Static("image/"),
        exception: Some(Str::Static("image/svg+xml")),
    };

    /// Predicate that wont compress Server-Sent Events (SSE) responses.
    pub const SSE: Self = Self::const_new("text/event-stream");

    /// Create a new `NotForContentType`.
    pub fn new(content_type: &str) -> Self {
        Self {
            content_type: Str::Shared(content_type.into()),
            exception: None,
        }
    }

    /// Create a new `NotForContentType` from a static string.
    pub const fn const_new(content_type: &'static str) -> Self {
        Self {
            content_type: Str::Static(content_type),
            exception: None,
        }
    }
}

impl Predicate for NotForContentType {
    fn should_compress<B>(&self, response: &http::Response<B>) -> bool
    where
        B: Body,
    {
        if let Some(except) = &self.exception {
            if content_type(response) == except.as_str() {
                return true;
            }
        }

        !content_type(response).starts_with(self.content_type.as_str())
    }
}

#[derive(Clone)]
enum Str {
    Static(&'static str),
    Shared(Arc<str>),
}

impl Str {
    fn as_str(&self) -> &str {
        match self {
            Str::Static(s) => s,
            Str::Shared(s) => s,
        }
    }
}

impl fmt::Debug for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(inner) => inner.fmt(f),
            Self::Shared(inner) => inner.fmt(f),
        }
    }
}

fn content_type<B>(response: &http::Response<B>) -> &str {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
}
