use http::Error as HttpRequestBuildError;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use opentelemetry::global;
use opentelemetry::propagation::Injector;
use reqwest::IntoUrl;
use reqwest::Method;
use serde::Serialize;
use std::fmt::Display;
use std::time::Duration;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub type HttpError = reqwest::Error;
pub type HttpResponse = reqwest::Response;

#[derive(Clone, Debug)]
pub struct HttpClient {
    inner: reqwest::Client,
    request_logging: RequestLogging,
}

impl HttpClient {
    pub fn new(inner: reqwest::Client) -> Self {
        Self {
            inner,
            request_logging: RequestLogging::Enabled,
        }
    }

    /// Creates a client that suppresses request URL and response-header diagnostics.
    ///
    /// Use this for endpoints whose URLs or headers may contain credentials that are redacted by
    /// the caller above the HTTP transport boundary.
    pub fn new_without_request_logging(inner: reqwest::Client) -> Self {
        Self {
            inner,
            request_logging: RequestLogging::Disabled,
        }
    }

    pub fn get<U>(&self, url: U) -> RequestBuilder
    where
        U: IntoUrl,
    {
        self.request(Method::GET, url)
    }

    pub fn post<U>(&self, url: U) -> RequestBuilder
    where
        U: IntoUrl,
    {
        self.request(Method::POST, url)
    }

    pub fn request<U>(&self, method: Method, url: U) -> RequestBuilder
    where
        U: IntoUrl,
    {
        let url_str = url.as_str().to_string();
        RequestBuilder::new(
            self.inner.request(method.clone(), url),
            method,
            url_str,
            self.request_logging,
        )
    }

    pub(crate) async fn execute(
        &self,
        request: reqwest::Request,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let method = request.method().clone();
        let url = request.url().to_string();

        match self.execute_without_request_logging(request).await {
            Ok(response) => {
                self.log_response(&method, &url, &response);
                Ok(response)
            }
            Err(error) => {
                self.log_error(&method, &url, &error);
                Err(error)
            }
        }
    }

    pub(crate) async fn execute_without_request_logging(
        &self,
        mut request: reqwest::Request,
    ) -> Result<reqwest::Response, reqwest::Error> {
        request.headers_mut().extend(trace_headers());
        self.inner.execute(request).await
    }

    pub(crate) fn log_response(&self, method: &Method, url: &str, response: &reqwest::Response) {
        if self.request_logging == RequestLogging::Enabled {
            tracing::debug!(
                method = %method,
                url = %url,
                status = %response.status(),
                headers = ?response.headers(),
                version = ?response.version(),
                "Request completed"
            );
        }
    }

    pub(crate) fn log_error(&self, method: &Method, url: &str, error: &reqwest::Error) {
        if self.request_logging == RequestLogging::Enabled {
            tracing::debug!(
                method = %method,
                url = %url,
                status = error.status().map(|status| status.as_u16()),
                error = %error,
                "Request failed"
            );
        }
    }
    pub(crate) fn log_error_summary(&self, method: &Method, url: &str, error: &reqwest::Error) {
        if self.request_logging == RequestLogging::Enabled {
            tracing::debug!(
                method = %method,
                url = %url,
                status = error.status().map(|status| status.as_u16()),
                is_timeout = error.is_timeout(),
                is_connect = error.is_connect(),
                "Request failed"
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestLogging {
    Enabled,
    Disabled,
}

#[must_use = "requests are not sent unless `send` is awaited"]
#[derive(Debug)]
pub struct RequestBuilder {
    builder: reqwest::RequestBuilder,
    method: Method,
    url: String,
    request_logging: RequestLogging,
}

impl RequestBuilder {
    fn new(
        builder: reqwest::RequestBuilder,
        method: Method,
        url: String,
        request_logging: RequestLogging,
    ) -> Self {
        Self {
            builder,
            method,
            url,
            request_logging,
        }
    }

    fn map(self, f: impl FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder) -> Self {
        Self {
            builder: f(self.builder),
            method: self.method,
            url: self.url,
            request_logging: self.request_logging,
        }
    }

    pub fn headers(self, headers: HeaderMap) -> Self {
        self.map(|builder| builder.headers(headers))
    }

    pub fn header<K, V>(self, key: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<HttpRequestBuildError>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<HttpRequestBuildError>,
    {
        self.map(|builder| builder.header(key, value))
    }

    pub fn bearer_auth<T>(self, token: T) -> Self
    where
        T: Display,
    {
        self.map(|builder| builder.bearer_auth(token))
    }

    pub fn timeout(self, timeout: Duration) -> Self {
        self.map(|builder| builder.timeout(timeout))
    }

    pub fn json<T>(self, value: &T) -> Self
    where
        T: ?Sized + Serialize,
    {
        self.map(|builder| builder.json(value))
    }

    pub fn body<B>(self, body: B) -> Self
    where
        B: Into<reqwest::Body>,
    {
        self.map(|builder| builder.body(body))
    }

    pub async fn send(self) -> Result<HttpResponse, HttpError> {
        let headers = trace_headers();

        match self.builder.headers(headers).send().await {
            Ok(response) => {
                if self.request_logging == RequestLogging::Enabled {
                    tracing::debug!(
                        method = %self.method,
                        url = %self.url,
                        status = %response.status(),
                        headers = ?response.headers(),
                        version = ?response.version(),
                        "Request completed"
                    );
                }

                Ok(response)
            }
            Err(error) => {
                if self.request_logging == RequestLogging::Enabled {
                    let status = error.status();
                    tracing::debug!(
                        method = %self.method,
                        url = %self.url,
                        status = status.map(|s| s.as_u16()),
                        error = %error,
                        "Request failed"
                    );
                }
                Err(error)
            }
        }
    }
}

struct HeaderMapInjector<'a>(&'a mut HeaderMap);

impl<'a> Injector for HeaderMapInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let (Ok(name), Ok(val)) = (
            HeaderName::from_bytes(key.as_bytes()),
            HeaderValue::from_str(&value),
        ) {
            self.0.insert(name, val);
        }
    }
}

pub(crate) fn trace_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    global::get_text_map_propagator(|prop| {
        prop.inject_context(
            &Span::current().context(),
            &mut HeaderMapInjector(&mut headers),
        );
    });
    headers
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::propagation::Extractor;
    use opentelemetry::propagation::TextMapPropagator;
    use opentelemetry::trace::TraceContextExt;
    use opentelemetry::trace::TracerProvider;
    use opentelemetry_sdk::propagation::TraceContextPropagator;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use tracing::trace_span;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    #[test]
    fn inject_trace_headers_uses_current_span_context() {
        global::set_text_map_propagator(TraceContextPropagator::new());

        let provider = SdkTracerProvider::builder().build();
        let tracer = provider.tracer("test-tracer");
        let subscriber =
            tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));
        let _guard = subscriber.set_default();

        let span = trace_span!("client_request");
        let _entered = span.enter();
        let span_context = span.context().span().span_context().clone();

        let headers = trace_headers();

        let extractor = HeaderMapExtractor(&headers);
        let extracted = TraceContextPropagator::new().extract(&extractor);
        let extracted_span = extracted.span();
        let extracted_context = extracted_span.span_context();

        assert!(extracted_context.is_valid());
        assert_eq!(extracted_context.trace_id(), span_context.trace_id());
        assert_eq!(extracted_context.span_id(), span_context.span_id());
    }

    struct HeaderMapExtractor<'a>(&'a HeaderMap);

    impl<'a> Extractor for HeaderMapExtractor<'a> {
        fn get(&self, key: &str) -> Option<&str> {
            self.0.get(key).and_then(|value| value.to_str().ok())
        }

        fn keys(&self) -> Vec<&str> {
            self.0.keys().map(HeaderName::as_str).collect()
        }
    }
}
