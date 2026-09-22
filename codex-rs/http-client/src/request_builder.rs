//! Builds direct and routed requests without constructing a second transport client.
//! Routed requests select their transport only when sent; debug output redacts their URLs.

use bytes::Bytes;
use futures::TryStream;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::Method;
use http::header::AUTHORIZATION;
use http::header::CONTENT_TYPE;
use reqwest::IntoUrl;
use serde::Serialize;
use std::fmt;
use std::fmt::Display;
use std::sync::Arc;
use std::time::Duration;

use crate::HttpError;
use crate::HttpResponse;
use crate::RouteAwareClientPool;
use crate::client::HttpClientBackend;
use crate::client::TransportClient;
use crate::request_draft::HeaderUpdate;
use crate::request_draft::RequestDraft;

#[must_use = "requests are not sent unless `send` is awaited"]
pub struct RequestBuilder {
    backend: HttpClientBackend,
    request: Result<RequestDraft, HttpError>,
}

impl fmt::Debug for RequestBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequestBuilder")
            .field(
                "method",
                &self
                    .request
                    .as_ref()
                    .ok()
                    .map(|request| request.request.method()),
            )
            .field("url", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl RequestBuilder {
    pub(crate) fn direct<U: IntoUrl>(client: &TransportClient, method: Method, url: U) -> Self {
        Self {
            backend: HttpClientBackend::Direct(client.clone()),
            request: RequestDraft::new(method, url).map_err(HttpError::Request),
        }
    }

    pub(crate) fn routed<U: IntoUrl>(
        pool: Arc<RouteAwareClientPool>,
        method: Method,
        url: U,
    ) -> Self {
        Self {
            backend: HttpClientBackend::Routed(pool),
            request: RequestDraft::new(method, url).map_err(HttpError::Request),
        }
    }

    fn map(mut self, update: impl FnOnce(&mut RequestDraft) -> Result<(), HttpError>) -> Self {
        self.request = self.request.and_then(|mut request| {
            update(&mut request)?;
            Ok(request)
        });
        self
    }

    pub fn headers(self, headers: HeaderMap) -> Self {
        self.map(|request| {
            request.extend_headers(headers);
            Ok(())
        })
    }

    pub fn header<K, V>(self, key: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.map(|request| {
            let key = HeaderName::try_from(key)
                .map_err(Into::into)
                .map_err(|error: http::Error| HttpError::Build(error.to_string()))?;
            let value = HeaderValue::try_from(value)
                .map_err(Into::into)
                .map_err(|error: http::Error| HttpError::Build(error.to_string()))?;
            request.headers.push(HeaderUpdate::Append(key, value));
            Ok(())
        })
    }

    pub fn bearer_auth<T: Display>(self, token: T) -> Self {
        self.map(|request| {
            let mut value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|error| HttpError::Build(error.to_string()))?;
            value.set_sensitive(true);
            request
                .headers
                .push(HeaderUpdate::Append(AUTHORIZATION, value));
            Ok(())
        })
    }

    /// Bounds route selection, connection establishment, and the response headers.
    /// Use [`crate::HttpClientBuilder::connect_timeout`] to bound only connection establishment.
    pub fn timeout(self, timeout: Duration) -> Self {
        self.map(|request| {
            *request.request.timeout_mut() = Some(timeout);
            Ok(())
        })
    }

    pub fn version(self, version: http::Version) -> Self {
        self.map(|request| {
            *request.request.version_mut() = version;
            Ok(())
        })
    }

    pub fn json<T: ?Sized + Serialize>(self, value: &T) -> Self {
        self.map(|request| {
            let body =
                serde_json::to_vec(value).map_err(|error| HttpError::Build(error.to_string()))?;
            let has_content_type = request.headers.iter().any(|update| match update {
                HeaderUpdate::Replace(headers) => headers.contains_key(CONTENT_TYPE),
                HeaderUpdate::Append(name, _) => *name == CONTENT_TYPE,
            });
            if !has_content_type {
                request.headers.push(HeaderUpdate::Append(
                    CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                ));
            }
            *request.request.body_mut() = Some(body.into());
            Ok(())
        })
    }

    pub fn query<T: ?Sized + Serialize>(self, query: &T) -> Self {
        self.map(|request| {
            query
                .serialize(serde_urlencoded::Serializer::new(
                    &mut request.request.url_mut().query_pairs_mut(),
                ))
                .map_err(|error| HttpError::Build(error.to_string()))?;
            if request.request.url().query() == Some("") {
                request.request.url_mut().set_query(None);
            }
            Ok(())
        })
    }

    pub fn body<B: Into<reqwest::Body>>(self, body: B) -> Self {
        self.map(|request| {
            *request.request.body_mut() = Some(body.into());
            Ok(())
        })
    }

    /// Sets a streaming body without exposing the underlying HTTP implementation.
    pub fn body_stream<S>(self, stream: S) -> Self
    where
        S: TryStream + Send + 'static,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        Bytes: From<S::Ok>,
    {
        self.body(reqwest::Body::wrap_stream(stream))
    }

    pub async fn send(self) -> Result<HttpResponse, HttpError> {
        match self.backend {
            HttpClientBackend::Routed(pool) => pool.send(self.request?).await,
            HttpClientBackend::Direct(client) => {
                Ok(client.execute(self.request?.build(&client)?).await?.into())
            }
        }
    }
}

#[cfg(test)]
#[path = "request_builder_tests.rs"]
mod tests;
