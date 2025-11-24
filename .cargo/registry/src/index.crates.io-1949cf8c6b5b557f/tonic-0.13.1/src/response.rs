use http::Extensions;

use crate::metadata::MetadataMap;

/// A gRPC response and metadata from an RPC call.
#[derive(Debug)]
pub struct Response<T> {
    metadata: MetadataMap,
    message: T,
    extensions: Extensions,
}

impl<T> Response<T> {
    /// Create a new gRPC response.
    ///
    /// ```rust
    /// # use tonic::Response;
    /// # pub struct HelloReply {
    /// #   pub message: String,
    /// # }
    /// # let name = "";
    /// Response::new(HelloReply {
    ///     message: format!("Hello, {}!", name).into(),
    /// });
    /// ```
    pub fn new(message: T) -> Self {
        Response {
            metadata: MetadataMap::new(),
            message,
            extensions: Extensions::new(),
        }
    }

    /// Get a immutable reference to `T`.
    pub fn get_ref(&self) -> &T {
        &self.message
    }

    /// Get a mutable reference to the message
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.message
    }

    /// Get a reference to the custom response metadata.
    pub fn metadata(&self) -> &MetadataMap {
        &self.metadata
    }

    /// Get a mutable reference to the response metadata.
    pub fn metadata_mut(&mut self) -> &mut MetadataMap {
        &mut self.metadata
    }

    /// Consumes `self`, returning the message
    pub fn into_inner(self) -> T {
        self.message
    }

    /// Consumes `self` returning the parts of the response.
    pub fn into_parts(self) -> (MetadataMap, T, Extensions) {
        (self.metadata, self.message, self.extensions)
    }

    /// Create a new gRPC response from metadata, message and extensions.
    pub fn from_parts(metadata: MetadataMap, message: T, extensions: Extensions) -> Self {
        Self {
            metadata,
            message,
            extensions,
        }
    }

    pub(crate) fn from_http(res: http::Response<T>) -> Self {
        let (head, message) = res.into_parts();
        Response {
            metadata: MetadataMap::from_headers(head.headers),
            message,
            extensions: head.extensions,
        }
    }

    pub(crate) fn into_http(self) -> http::Response<T> {
        let mut res = http::Response::new(self.message);

        *res.version_mut() = http::Version::HTTP_2;
        *res.headers_mut() = self.metadata.into_sanitized_headers();
        *res.extensions_mut() = self.extensions;

        res
    }

    #[doc(hidden)]
    pub fn map<F, U>(self, f: F) -> Response<U>
    where
        F: FnOnce(T) -> U,
    {
        let message = f(self.message);
        Response {
            metadata: self.metadata,
            message,
            extensions: self.extensions,
        }
    }

    /// Returns a reference to the associated extensions.
    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Returns a mutable reference to the associated extensions.
    pub fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }

    /// Disable compression of the response body.
    ///
    /// This disables compression of the body of this response, even if compression is enabled on
    /// the server.
    ///
    /// **Note**: This only has effect on responses to unary requests and responses to client to
    /// server streams. Response streams (server to client stream and bidirectional streams) will
    /// still be compressed according to the configuration of the server.
    #[cfg(any(feature = "gzip", feature = "deflate", feature = "zstd"))]
    pub fn disable_compression(&mut self) {
        self.extensions_mut()
            .insert(crate::codec::compression::SingleMessageCompressionOverride::Disable);
    }
}

impl<T> From<T> for Response<T> {
    fn from(inner: T) -> Self {
        Response::new(inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{MetadataKey, MetadataValue};

    #[test]
    fn reserved_headers_are_excluded() {
        let mut r = Response::new(1);

        for header in &MetadataMap::GRPC_RESERVED_HEADERS {
            r.metadata_mut().insert(
                MetadataKey::unchecked_from_header_name(header.clone()),
                MetadataValue::from_static("invalid"),
            );
        }

        let http_response = r.into_http();
        assert!(http_response.headers().is_empty());
    }
}
