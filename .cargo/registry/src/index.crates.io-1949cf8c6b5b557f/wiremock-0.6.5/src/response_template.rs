use http::{HeaderMap, HeaderName, HeaderValue, Response, StatusCode};
use http_body_util::Full;
use hyper::body::Bytes;
use serde::Serialize;
use std::convert::TryInto;
use std::time::Duration;

/// The blueprint for the response returned by a [`MockServer`] when a [`Mock`] matches on an incoming request.
///
/// [`Mock`]: crate::Mock
/// [`MockServer`]: crate::MockServer
#[derive(Clone, Debug)]
pub struct ResponseTemplate {
    mime: String,
    status_code: StatusCode,
    headers: HeaderMap,
    body: Option<Vec<u8>>,
    delay: Option<Duration>,
}

// `wiremock` is a crate meant for testing - failures are most likely not handled/temporary mistakes.
// Hence we prefer to panic and provide an easier API than to use `Result`s thus pushing
// the burden of "correctness" (and conversions) on the user.
//
// All methods try to accept the widest possible set of inputs and then perform the fallible conversion
// internally, bailing if the fallible conversion fails.
//
// Same principle applies to allocation/cloning, freely used where convenient.
impl ResponseTemplate {
    /// Start building a `ResponseTemplate` specifying the status code of the response.
    pub fn new<S>(s: S) -> Self
    where
        S: TryInto<StatusCode>,
        <S as TryInto<StatusCode>>::Error: std::fmt::Debug,
    {
        let status_code = s.try_into().expect("Failed to convert into status code.");
        Self {
            status_code,
            headers: HeaderMap::new(),
            mime: String::new(),
            body: None,
            delay: None,
        }
    }

    /// Append a header `value` to list of headers with `key` as header name.
    ///
    /// Unlike `insert_header`, this function will not override the contents of a header:
    /// - if there are no header values with `key` as header name, it will insert one;
    /// - if there are already some values with `key` as header name, it will append to the
    ///   existing list.
    pub fn append_header<K, V>(mut self, key: K, value: V) -> Self
    where
        K: TryInto<HeaderName>,
        <K as TryInto<HeaderName>>::Error: std::fmt::Debug,
        V: TryInto<HeaderValue>,
        <V as TryInto<HeaderValue>>::Error: std::fmt::Debug,
    {
        let key = key.try_into().expect("Failed to convert into header name.");
        let value = value
            .try_into()
            .expect("Failed to convert into header value.");
        self.headers.append(key, value);
        self
    }

    /// Insert a header `value` with `key` as header name.
    ///
    /// This function will override the contents of a header:
    /// - if there are no header values with `key` as header name, it will insert one;
    /// - if there are already some values with `key` as header name, it will drop them and
    ///   start a new list of header values, containing only `value`.
    ///
    /// ### Example:
    /// ```rust
    /// use wiremock::{MockServer, Mock, ResponseTemplate};
    /// use wiremock::matchers::method;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     // Arrange
    ///     let mock_server = MockServer::start().await;
    ///     let correlation_id = "1311db4f-fe65-4cb2-b514-1bb47f781aa7";
    ///     let template = ResponseTemplate::new(200).insert_header(
    ///         "X-Correlation-ID",
    ///         correlation_id
    ///     );
    ///     Mock::given(method("GET"))
    ///         .respond_with(template)
    ///         .mount(&mock_server)
    ///         .await;
    ///
    ///     // Act
    ///     let res = reqwest::get(&mock_server.uri())
    ///         .await
    ///         .unwrap();
    ///
    ///     // Assert
    ///     assert_eq!(res.headers().get("X-Correlation-ID").unwrap().to_str().unwrap(), correlation_id);
    /// }
    /// ```
    pub fn insert_header<K, V>(mut self, key: K, value: V) -> Self
    where
        K: TryInto<HeaderName>,
        <K as TryInto<HeaderName>>::Error: std::fmt::Debug,
        V: TryInto<HeaderValue>,
        <V as TryInto<HeaderValue>>::Error: std::fmt::Debug,
    {
        let key = key.try_into().expect("Failed to convert into header name.");
        let value = value
            .try_into()
            .expect("Failed to convert into header value.");
        self.headers.insert(key, value);
        self
    }

    /// Append multiple header key-value pairs.
    ///
    /// Existing header values will not be overridden.
    ///
    /// # Example
    /// ```rust
    /// use wiremock::{MockServer, Mock, ResponseTemplate};
    /// use wiremock::matchers::method;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     // Arrange
    ///     let mock_server = MockServer::start().await;
    ///     let headers = vec![
    ///         ("Set-Cookie", "name=value"),
    ///         ("Set-Cookie", "name2=value2; Domain=example.com"),
    ///     ];
    ///     let template = ResponseTemplate::new(200).append_headers(headers);
    ///     Mock::given(method("GET"))
    ///         .respond_with(template)
    ///         .mount(&mock_server)
    ///         .await;
    ///
    ///     // Act
    ///     let res = reqwest::get(&mock_server.uri())
    ///         .await
    ///         .unwrap();
    ///
    ///     // Assert
    ///     assert_eq!(res.headers().get_all("Set-Cookie").iter().count(), 2);
    /// }
    /// ```
    pub fn append_headers<K, V, I>(mut self, headers: I) -> Self
    where
        K: TryInto<HeaderName>,
        <K as TryInto<HeaderName>>::Error: std::fmt::Debug,
        V: TryInto<HeaderValue>,
        <V as TryInto<HeaderValue>>::Error: std::fmt::Debug,
        I: IntoIterator<Item = (K, V)>,
    {
        let headers = headers.into_iter().map(|(key, value)| {
            (
                key.try_into().expect("Failed to convert into header name."),
                value
                    .try_into()
                    .expect("Failed to convert into header value."),
            )
        });
        // The `Extend<(HeaderName, T)>` impl uses `HeaderMap::append` internally: https://docs.rs/http/1.0.0/src/http/header/map.rs.html#1953
        self.headers.extend(headers);
        self
    }

    /// Set the response body with bytes.
    ///
    /// It sets "Content-Type" to "application/octet-stream".
    ///
    /// To set a body with bytes but a different "Content-Type"
    /// [`set_body_raw`](#method.set_body_raw) can be used.
    pub fn set_body_bytes<B>(mut self, body: B) -> Self
    where
        B: TryInto<Vec<u8>>,
        <B as TryInto<Vec<u8>>>::Error: std::fmt::Debug,
    {
        let body = body.try_into().expect("Failed to convert into body.");
        self.body = Some(body);
        self
    }

    /// Set the response body from a JSON-serializable value.
    ///
    /// It sets "Content-Type" to "application/json".
    pub fn set_body_json<B: Serialize>(mut self, body: B) -> Self {
        let body = serde_json::to_vec(&body).expect("Failed to convert into body.");

        self.body = Some(body);
        self.mime = "application/json".to_string();
        self
    }

    /// Set the response body to a string.
    ///
    /// It sets "Content-Type" to "text/plain".
    pub fn set_body_string<T>(mut self, body: T) -> Self
    where
        T: TryInto<String>,
        <T as TryInto<String>>::Error: std::fmt::Debug,
    {
        let body = body.try_into().expect("Failed to convert into body.");

        self.body = Some(body.into_bytes());
        self.mime = "text/plain".to_string();
        self
    }

    /// Set a raw response body. The mime type needs to be set because the
    /// raw body could be of any type.
    ///
    /// ### Example:
    /// ```rust
    /// use wiremock::{MockServer, Mock, ResponseTemplate};
    /// use wiremock::matchers::method;
    ///
    /// mod external {
    ///     // This could be a method of a struct that is
    ///     // implemented in another crate and the struct
    ///     // does not implement Serialize.
    ///     pub fn body() -> Vec<u8>{
    ///         r#"{"hello": "world"}"#.as_bytes().to_owned()
    ///     }
    /// }
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     // Arrange
    ///     let mock_server = MockServer::start().await;
    ///     let template = ResponseTemplate::new(200).set_body_raw(
    ///         external::body(),
    ///         "application/json"
    ///     );
    ///     Mock::given(method("GET"))
    ///         .respond_with(template)
    ///         .mount(&mock_server)
    ///         .await;
    ///
    ///     // Act
    ///     let mut res = reqwest::get(&mock_server.uri())
    ///         .await
    ///         .unwrap();
    ///     let content_type = res.headers().get(reqwest::header::CONTENT_TYPE).cloned();
    ///     let body = res.text()
    ///         .await
    ///         .unwrap();
    ///
    ///     // Assert
    ///     assert_eq!(body, r#"{"hello": "world"}"#);
    ///     assert_eq!(content_type, Some("application/json".parse().unwrap()));
    /// }
    /// ```
    pub fn set_body_raw<B>(mut self, body: B, mime: &str) -> Self
    where
        B: TryInto<Vec<u8>>,
        <B as TryInto<Vec<u8>>>::Error: std::fmt::Debug,
    {
        let body = body.try_into().expect("Failed to convert into body.");
        self.body = Some(body);
        self.mime = mime.to_string();
        self
    }

    /// By default the [`MockServer`] tries to fulfill incoming requests as fast as possible.
    ///
    /// You can use `set_delay` to introduce an artificial delay to simulate the behaviour of
    /// a real server with a non-negligible latency.
    ///
    /// In particular, you can use it to test the behaviour of your timeout policies.
    ///
    /// ### Example:
    /// ```rust
    /// use wiremock::{MockServer, Mock, ResponseTemplate};
    /// use wiremock::matchers::method;
    /// use std::time::Duration;
    /// use async_std::prelude::FutureExt;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     // Arrange
    ///     let mock_server = MockServer::start().await;
    ///     let delay = Duration::from_secs(1);
    ///     let template = ResponseTemplate::new(200).set_delay(delay);
    ///     Mock::given(method("GET"))
    ///         .respond_with(template)
    ///         .mount(&mock_server)
    ///         .await;
    ///
    ///     // Act
    ///     let mut res = async_std::future::timeout(
    ///         // Shorter than the response delay!
    ///         delay / 3,
    ///         reqwest::get(&mock_server.uri())
    ///     )
    ///     .await;
    ///
    ///     // Assert - Timeout error!
    ///     assert!(res.is_err());
    /// }
    /// ```
    ///
    /// [`MockServer`]: crate::mock_server::MockServer
    pub fn set_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);

        self
    }

    /// Generate a response from the template.
    pub(crate) fn generate_response(&self) -> Response<Full<Bytes>> {
        let mut response = Response::builder().status(self.status_code);

        let mut headers = self.headers.clone();
        // Set content-type, if needed
        if !self.mime.is_empty() {
            headers.insert(http::header::CONTENT_TYPE, self.mime.parse().unwrap());
        }
        *response.headers_mut().unwrap() = headers;

        let body = self.body.clone().unwrap_or_default();
        response.body(body.into()).unwrap()
    }

    /// Retrieve the response delay.
    pub(crate) fn delay(&self) -> &Option<Duration> {
        &self.delay
    }
}
