use async_trait::async_trait;
use std::fmt::Debug;

#[doc(no_inline)]
pub use bytes::Bytes;
#[doc(no_inline)]
pub use http::{Request, Response};
use opentelemetry::propagation::{Extractor, Injector};

/// Helper for injecting headers into HTTP Requests. This is used for OpenTelemetry context
/// propagation over HTTP.
/// See [this](https://github.com/open-telemetry/opentelemetry-rust/blob/main/examples/tracing-http-propagator/README.md)
/// for example usage.
pub struct HeaderInjector<'a>(pub &'a mut http::HeaderMap);

impl Injector for HeaderInjector<'_> {
    /// Set a key and value in the HeaderMap.  Does nothing if the key or value are not valid inputs.
    fn set(&mut self, key: &str, value: String) {
        if let Ok(name) = http::header::HeaderName::from_bytes(key.as_bytes()) {
            if let Ok(val) = http::header::HeaderValue::from_str(&value) {
                self.0.insert(name, val);
            }
        }
    }
}

/// Helper for extracting headers from HTTP Requests. This is used for OpenTelemetry context
/// propagation over HTTP.
/// See [this](https://github.com/open-telemetry/opentelemetry-rust/blob/main/examples/tracing-http-propagator/README.md)
/// for example usage.
pub struct HeaderExtractor<'a>(pub &'a http::HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    /// Get a value for a key from the HeaderMap.  If the value is not valid ASCII, returns None.
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    /// Collect all the keys from the HeaderMap.
    fn keys(&self) -> Vec<&str> {
        self.0
            .keys()
            .map(|value| value.as_str())
            .collect::<Vec<_>>()
    }
}

pub type HttpError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// A minimal interface necessary for sending requests over HTTP.
/// Used primarily for exporting telemetry over HTTP. Also used for fetching
/// sampling strategies for JaegerRemoteSampler
///
/// Users sometime choose HTTP clients that relay on a certain async runtime. This trait allows
/// users to bring their choice of HTTP client.
#[async_trait]
pub trait HttpClient: Debug + Send + Sync {
    /// Send the specified HTTP request with `Vec<u8>` payload
    ///
    /// Returns the HTTP response including the status code and body.
    ///
    /// Returns an error if it can't connect to the server or the request could not be completed,
    /// e.g. because of a timeout, infinite redirects, or a loss of connection.
    #[deprecated(note = "Use `send_bytes` with `Bytes` payload instead.")]
    async fn send(&self, request: Request<Vec<u8>>) -> Result<Response<Bytes>, HttpError> {
        self.send_bytes(request.map(Into::into)).await
    }

    /// Send the specified HTTP request with `Bytes` payload.
    ///
    /// Returns the HTTP response including the status code and body.
    ///
    /// Returns an error if it can't connect to the server or the request could not be completed,
    /// e.g. because of a timeout, infinite redirects, or a loss of connection.
    async fn send_bytes(&self, request: Request<Bytes>) -> Result<Response<Bytes>, HttpError>;
}

#[cfg(feature = "reqwest")]
mod reqwest {
    use opentelemetry::otel_debug;

    use super::{async_trait, Bytes, HttpClient, HttpError, Request, Response};

    #[async_trait]
    impl HttpClient for reqwest::Client {
        async fn send_bytes(&self, request: Request<Bytes>) -> Result<Response<Bytes>, HttpError> {
            otel_debug!(name: "ReqwestClient.Send");
            let request = request.try_into()?;
            let mut response = self.execute(request).await?.error_for_status()?;
            let headers = std::mem::take(response.headers_mut());
            let mut http_response = Response::builder()
                .status(response.status())
                .body(response.bytes().await?)?;
            *http_response.headers_mut() = headers;

            Ok(http_response)
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[async_trait]
    impl HttpClient for reqwest::blocking::Client {
        async fn send_bytes(&self, request: Request<Bytes>) -> Result<Response<Bytes>, HttpError> {
            otel_debug!(name: "ReqwestBlockingClient.Send");
            let request = request.try_into()?;
            let mut response = self.execute(request)?.error_for_status()?;
            let headers = std::mem::take(response.headers_mut());
            let mut http_response = Response::builder()
                .status(response.status())
                .body(response.bytes()?)?;
            *http_response.headers_mut() = headers;

            Ok(http_response)
        }
    }
}

#[cfg(feature = "hyper")]
pub mod hyper {
    use super::{async_trait, Bytes, HttpClient, HttpError, Request, Response};
    use crate::ResponseExt;
    use http::HeaderValue;
    use http_body_util::{BodyExt, Full};
    use hyper::body::{Body as HttpBody, Frame};
    use hyper_util::client::legacy::{
        connect::{Connect, HttpConnector},
        Client,
    };
    use opentelemetry::otel_debug;
    use std::fmt::Debug;
    use std::pin::Pin;
    use std::task::{self, Poll};
    use std::time::Duration;
    use tokio::time;

    #[derive(Debug, Clone)]
    pub struct HyperClient<C = HttpConnector>
    where
        C: Connect + Clone + Send + Sync + 'static,
    {
        inner: Client<C, Body>,
        timeout: Duration,
        authorization: Option<HeaderValue>,
    }

    impl<C> HyperClient<C>
    where
        C: Connect + Clone + Send + Sync + 'static,
    {
        pub fn new(connector: C, timeout: Duration, authorization: Option<HeaderValue>) -> Self {
            // TODO - support custom executor
            let inner = Client::builder(hyper_util::rt::TokioExecutor::new()).build(connector);
            Self {
                inner,
                timeout,
                authorization,
            }
        }
    }

    impl HyperClient<HttpConnector> {
        /// Creates a new `HyperClient` with a default `HttpConnector`.
        pub fn with_default_connector(
            timeout: Duration,
            authorization: Option<HeaderValue>,
        ) -> Self {
            Self::new(HttpConnector::new(), timeout, authorization)
        }
    }

    #[async_trait]
    impl HttpClient for HyperClient {
        async fn send_bytes(&self, request: Request<Bytes>) -> Result<Response<Bytes>, HttpError> {
            otel_debug!(name: "HyperClient.Send");
            let (parts, body) = request.into_parts();
            let mut request = Request::from_parts(parts, Body(Full::from(body)));
            if let Some(ref authorization) = self.authorization {
                request
                    .headers_mut()
                    .insert(http::header::AUTHORIZATION, authorization.clone());
            }
            let mut response = time::timeout(self.timeout, self.inner.request(request)).await??;
            let headers = std::mem::take(response.headers_mut());

            let mut http_response = Response::builder()
                .status(response.status())
                .body(response.into_body().collect().await?.to_bytes())?;
            *http_response.headers_mut() = headers;

            Ok(http_response.error_for_status()?)
        }
    }

    pub struct Body(Full<Bytes>);

    impl HttpBody for Body {
        type Data = Bytes;
        type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

        #[inline]
        fn poll_frame(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
            let inner_body = unsafe { self.map_unchecked_mut(|b| &mut b.0) };
            inner_body.poll_frame(cx).map_err(Into::into)
        }

        #[inline]
        fn is_end_stream(&self) -> bool {
            self.0.is_end_stream()
        }

        #[inline]
        fn size_hint(&self) -> hyper::body::SizeHint {
            self.0.size_hint()
        }
    }
}

/// Methods to make working with responses from the [`HttpClient`] trait easier.
pub trait ResponseExt: Sized {
    /// Turn a response into an error if the HTTP status does not indicate success (200 - 299).
    fn error_for_status(self) -> Result<Self, HttpError>;
}

impl<T> ResponseExt for Response<T> {
    fn error_for_status(self) -> Result<Self, HttpError> {
        if self.status().is_success() {
            Ok(self)
        } else {
            Err(format!("request failed with status {}", self.status()).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_headers_get() {
        let mut carrier = http::HeaderMap::new();
        HeaderInjector(&mut carrier).set("headerName", "value".to_string());

        assert_eq!(
            HeaderExtractor(&carrier).get("HEADERNAME"),
            Some("value"),
            "case insensitive extraction"
        )
    }

    #[test]
    fn http_headers_keys() {
        let mut carrier = http::HeaderMap::new();
        HeaderInjector(&mut carrier).set("headerName1", "value1".to_string());
        HeaderInjector(&mut carrier).set("headerName2", "value2".to_string());

        let extractor = HeaderExtractor(&carrier);
        let got = extractor.keys();
        assert_eq!(got.len(), 2);
        assert!(got.contains(&"headername1"));
        assert!(got.contains(&"headername2"));
    }
}
