//! A collection of different matching strategies provided out-of-the-box by `wiremock`.
//!
//! If the set of matchers provided out-of-the-box is not enough for your specific testing needs
//! you can implement your own thanks to the [`Match`] trait.
//!
//! Furthermore, `Fn` closures that take an immutable [`Request`] reference as input and return a boolean
//! as input automatically implement [`Match`] and can be used where a matcher is expected.
//!
//! Check [`Match`]'s documentation for examples.
use crate::{Match, Request};
use assert_json_diff::{CompareMode, assert_json_matches_no_panic};
use base64::prelude::{BASE64_STANDARD, Engine as _};
use http::{HeaderName, HeaderValue, Method};
use log::debug;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::convert::TryInto;
use std::str::{self, FromStr};
use url::Url;

/// Implement the `Match` trait for all closures, out of the box,
/// if their signature is compatible.
impl<F> Match for F
where
    F: Fn(&Request) -> bool,
    F: Send + Sync,
{
    fn matches(&self, request: &Request) -> bool {
        // Just call the closure itself!
        self(request)
    }
}

#[derive(Debug)]
/// Match **exactly** the method of a request.
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
///
///     let response = ResponseTemplate::new(200);
///     let mock = Mock::given(method("GET")).respond_with(response);
///
///     mock_server.register(mock).await;
///
///     // Act
///     let status = reqwest::get(&mock_server.uri())
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct MethodExactMatcher(Method);

/// Shorthand for [`MethodExactMatcher::new`].
pub fn method<T>(method: T) -> MethodExactMatcher
where
    T: AsRef<str>,
{
    MethodExactMatcher::new(method)
}

impl MethodExactMatcher {
    pub fn new<T>(method: T) -> Self
    where
        T: AsRef<str>,
    {
        let method = Method::from_str(&method.as_ref().to_ascii_uppercase())
            .expect("Failed to convert to HTTP method.");
        Self(method)
    }
}

impl Match for MethodExactMatcher {
    fn matches(&self, request: &Request) -> bool {
        request.method == self.0
    }
}

#[derive(Debug)]
/// Match all incoming requests, regardless of their method, path, headers or body.
///
/// You can use it to verify that a request has been fired towards the server, without making
/// any other assertion about it.
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::any;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     let response = ResponseTemplate::new(200);
///     // Respond with a `200 OK` to all requests hitting
///     // the mock server
///     let mock = Mock::given(any()).respond_with(response);
///
///     mock_server.register(mock).await;
///
///     // Act
///     let status = reqwest::get(&mock_server.uri())
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct AnyMatcher;

/// Shorthand for [`AnyMatcher`].
pub fn any() -> AnyMatcher {
    AnyMatcher
}

impl Match for AnyMatcher {
    fn matches(&self, _request: &Request) -> bool {
        true
    }
}

#[derive(Debug)]
/// Match **exactly** the path of a request.
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::path;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     let response = ResponseTemplate::new(200).set_body_string("world");
///     let mock = Mock::given(path("/hello")).respond_with(response);
///
///     mock_server.register(mock).await;
///
///     // Act
///     let status = reqwest::get(format!("{}/hello", &mock_server.uri()))
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
///
/// ### Example:
///
/// The path matcher ignores query parameters:
///
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::path;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     let response = ResponseTemplate::new(200).set_body_string("world");
///     let mock = Mock::given(path("/hello")).respond_with(response);
///
///     mock_server.register(mock).await;
///
///     // Act
///     let status = reqwest::get(format!("{}/hello?a_parameter=some_value", &mock_server.uri()))
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct PathExactMatcher(String);

/// Shorthand for [`PathExactMatcher::new`].
pub fn path<T>(path: T) -> PathExactMatcher
where
    T: Into<String>,
{
    PathExactMatcher::new(path)
}

impl PathExactMatcher {
    pub fn new<T: Into<String>>(path: T) -> Self {
        let path = path.into();

        if path.contains('?') {
            panic!(
                "Wiremock can't match the path `{}` because it contains a `?`. You must use `wiremock::matchers::query_param` to match on query parameters (the part of the path after the `?`).",
                path
            );
        }

        if let Ok(url) = Url::parse(&path)
            && let Some(host) = url.host_str()
        {
            panic!(
                "Wiremock can't match the path `{}` because it contains the host `{}`. You don't have to specify the host - wiremock knows it. Try replacing your path with `path(\"{}\")`",
                path,
                host,
                url.path()
            );
        }

        // Prepend "/" to the path if missing.
        if path.starts_with('/') {
            Self(path)
        } else {
            Self(format!("/{}", path))
        }
    }
}

impl Match for PathExactMatcher {
    fn matches(&self, request: &Request) -> bool {
        request.url.path() == self.0
    }
}

#[derive(Debug)]
/// Match the path of a request against a regular expression.
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::path_regex;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     let response = ResponseTemplate::new(200).set_body_string("world");
///     let mock = Mock::given(path_regex(r"^/hello/\d{3}$")).respond_with(response);
///
///     mock_server.register(mock).await;
///
///     // Act
///     let status = reqwest::get(format!("{}/hello/123", &mock_server.uri()))
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::path_regex;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     let response = ResponseTemplate::new(200).set_body_string("world");
///     let mock = Mock::given(path_regex(r"^/users/[a-z0-9-~_]{1,}/posts$")).respond_with(response);
///
///     mock_server.register(mock).await;
///
///     // Act
///     let status = reqwest::get(format!("{}/users/da2854ea-b70f-46e7-babc-2846eff4d33c/posts", &mock_server.uri()))
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct PathRegexMatcher(Regex);

/// Shorthand for [`PathRegexMatcher::new`].
pub fn path_regex<T>(path: T) -> PathRegexMatcher
where
    T: Into<String>,
{
    PathRegexMatcher::new(path)
}

impl PathRegexMatcher {
    pub fn new<T: Into<String>>(path: T) -> Self {
        let path = path.into();

        Self(Regex::new(&path).expect("Failed to create regex for path matcher"))
    }
}

impl Match for PathRegexMatcher {
    fn matches(&self, request: &Request) -> bool {
        self.0.is_match(request.url.path())
    }
}

#[derive(Debug)]
/// Match **exactly** the header of a request.
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::{header, headers};
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     Mock::given(header("custom", "header"))
///         .and(headers("cache-control", vec!["no-cache", "no-store"]))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     // Act
///     let client = reqwest::Client::new();
///     let status = client
///         .get(&mock_server.uri())
///         .header("custom", "header")
///         .header("cache-control", "no-cache, no-store")
///         .send()
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct HeaderExactMatcher(HeaderName, Vec<HeaderValue>);

/// Shorthand for [`HeaderExactMatcher::new`].
pub fn header<K, V>(key: K, value: V) -> HeaderExactMatcher
where
    K: TryInto<HeaderName>,
    <K as TryInto<HeaderName>>::Error: std::fmt::Debug,
    V: TryInto<HeaderValue>,
    <V as TryInto<HeaderValue>>::Error: std::fmt::Debug,
{
    HeaderExactMatcher::new(key, vec![value])
}

/// Shorthand for [`HeaderExactMatcher::new`] supporting multi valued headers.
pub fn headers<K, V>(key: K, values: Vec<V>) -> HeaderExactMatcher
where
    K: TryInto<HeaderName>,
    <K as TryInto<HeaderName>>::Error: std::fmt::Debug,
    V: TryInto<HeaderValue>,
    <V as TryInto<HeaderValue>>::Error: std::fmt::Debug,
{
    HeaderExactMatcher::new(key, values)
}

impl HeaderExactMatcher {
    pub fn new<K, V>(key: K, values: Vec<V>) -> Self
    where
        K: TryInto<HeaderName>,
        <K as TryInto<HeaderName>>::Error: std::fmt::Debug,
        V: TryInto<HeaderValue>,
        <V as TryInto<HeaderValue>>::Error: std::fmt::Debug,
    {
        let key = key.try_into().expect("Failed to convert to header name.");
        let values = values
            .into_iter()
            .map(|value| {
                value
                    .try_into()
                    .expect("Failed to convert to header value.")
            })
            .collect();
        Self(key, values)
    }
}

impl Match for HeaderExactMatcher {
    fn matches(&self, request: &Request) -> bool {
        let values = request
            .headers
            .get_all(&self.0)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter_map(|v| HeaderValue::from_str(v).ok())
            })
            .collect::<Vec<_>>();
        values == self.1 // order matters
    }
}

#[derive(Debug)]
/// Match **exactly** the header name of a request. It checks that the
/// header is present but does not validate the value.
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::header;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     use wiremock::matchers::header_exists;
///     let mock_server = MockServer::start().await;
///
///     Mock::given(header_exists("custom"))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     // Act
///     let client = reqwest::Client::new();
///     let status = client
///         .get(&mock_server.uri())
///         .header("custom", "header")
///         .send()
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct HeaderExistsMatcher(HeaderName);

/// Shorthand for [`HeaderExistsMatcher::new`].
pub fn header_exists<K>(key: K) -> HeaderExistsMatcher
where
    K: TryInto<HeaderName>,
    <K as TryInto<HeaderName>>::Error: std::fmt::Debug,
{
    HeaderExistsMatcher::new(key)
}

impl HeaderExistsMatcher {
    pub fn new<K>(key: K) -> Self
    where
        K: TryInto<HeaderName>,
        <K as TryInto<HeaderName>>::Error: std::fmt::Debug,
    {
        let key = key.try_into().expect("Failed to convert to header name.");
        Self(key)
    }
}

impl Match for HeaderExistsMatcher {
    fn matches(&self, request: &Request) -> bool {
        request.headers.get(&self.0).is_some()
    }
}

#[derive(Debug)]
/// Match the value of a header using a regular expression.
/// If the header is multi-valued, all values must satisfy the regular expression.
/// If the header is missing, the mock will not match.
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::header_regex;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     Mock::given(header_regex("custom", "header"))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     // Act
///     let client = reqwest::Client::new();
///     let status = client.get(&mock_server.uri())
///         .header("custom", "headers are fun to match on with a regex")
///         .send()
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct HeaderRegexMatcher(HeaderName, Regex);

/// Shorthand for [`HeaderRegexMatcher::new`].
pub fn header_regex<K>(key: K, value: &str) -> HeaderRegexMatcher
where
    K: TryInto<HeaderName>,
    <K as TryInto<HeaderName>>::Error: std::fmt::Debug,
{
    HeaderRegexMatcher::new(key, value)
}

impl HeaderRegexMatcher {
    pub fn new<K>(key: K, value: &str) -> Self
    where
        K: TryInto<HeaderName>,
        <K as TryInto<HeaderName>>::Error: std::fmt::Debug,
    {
        let key = key.try_into().expect("Failed to convert to header name.");
        let value_matcher = Regex::new(value).expect("Failed to create regex for value matcher");
        Self(key, value_matcher)
    }
}

impl Match for HeaderRegexMatcher {
    fn matches(&self, request: &Request) -> bool {
        let mut it = request
            .headers
            .get_all(&self.0)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .peekable();
        if it.peek().is_some() {
            it.all(|v| self.1.is_match(v))
        } else {
            false
        }
    }
}

#[derive(Debug)]
/// Match **exactly** the body of a request.
///
/// ### Example (string):
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::body_string;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     Mock::given(body_string("hello world!"))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     // Act
///     let client = reqwest::Client::new();
///     let status = client.post(&mock_server.uri())
///         .body("hello world!")
///         .send()
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
///
/// ### Example (json):
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::body_json;
/// use serde_json::json;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     let expected_body = json!({
///         "hello": "world!"
///     });
///     Mock::given(body_json(&expected_body))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     // Act
///     let client = reqwest::Client::new();
///     let status = client.post(&mock_server.uri())
///         .json(&expected_body)
///         .send()
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct BodyExactMatcher(Body);

#[derive(Debug)]
enum Body {
    Bytes(Vec<u8>),
    Json(Value),
}

impl BodyExactMatcher {
    /// Specify the expected body as a string.
    pub fn string<T: Into<String>>(body: T) -> Self {
        let body = body.into();
        Self(Body::Bytes(body.into_bytes()))
    }

    /// Specify the expected body as a vector of bytes.
    pub fn bytes<T: Into<Vec<u8>>>(body: T) -> Self {
        let body = body.into();
        Self(Body::Bytes(body))
    }

    /// Specify something JSON-serializable as the expected body.
    pub fn json<T: Serialize>(body: T) -> Self {
        let bytes = serde_json::to_vec(&body).expect("Failed to serialize JSON body");
        Self::json_string(bytes)
    }

    /// Specify a JSON string as the expected body.
    pub fn json_string(body: impl AsRef<[u8]>) -> Self {
        let body = serde_json::from_slice(body.as_ref()).expect("Failed to parse JSON string");
        Self(Body::Json(body))
    }
}

/// Shorthand for [`BodyExactMatcher::string`].
pub fn body_string<T>(body: T) -> BodyExactMatcher
where
    T: Into<String>,
{
    BodyExactMatcher::string(body)
}

/// Shorthand for [`BodyExactMatcher::bytes`].
pub fn body_bytes<T>(body: T) -> BodyExactMatcher
where
    T: Into<Vec<u8>>,
{
    BodyExactMatcher::bytes(body)
}

/// Shorthand for [`BodyExactMatcher::json`].
pub fn body_json<T>(body: T) -> BodyExactMatcher
where
    T: Serialize,
{
    BodyExactMatcher::json(body)
}

/// Shorthand for [`BodyExactMatcher::json_string`].
pub fn body_json_string(body: impl AsRef<[u8]>) -> BodyExactMatcher {
    BodyExactMatcher::json_string(body)
}

impl Match for BodyExactMatcher {
    fn matches(&self, request: &Request) -> bool {
        match &self.0 {
            Body::Bytes(bytes) => request.body == *bytes,
            Body::Json(json) => {
                if let Ok(body) = serde_json::from_slice::<Value>(&request.body) {
                    body == *json
                } else {
                    false
                }
            }
        }
    }
}

#[derive(Debug)]
/// Match part of the body of a request.
///
/// ### Example (string):
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::body_string_contains;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     Mock::given(body_string_contains("hello world"))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     // Act
///     let client = reqwest::Client::new();
///     let status = client.post(&mock_server.uri())
///         .body("this is a hello world example!")
///         .send()
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct BodyContainsMatcher(Vec<u8>);

impl BodyContainsMatcher {
    /// Specify the part of the body that should be matched as a string.
    pub fn string<T: Into<String>>(body: T) -> Self {
        Self(body.into().as_bytes().into())
    }
}

/// Shorthand for [`BodyContainsMatcher::string`].
pub fn body_string_contains<T>(body: T) -> BodyContainsMatcher
where
    T: Into<String>,
{
    BodyContainsMatcher::string(body)
}

impl Match for BodyContainsMatcher {
    fn matches(&self, request: &Request) -> bool {
        let body = match str::from_utf8(&request.body) {
            Ok(body) => body.to_string(),
            Err(err) => {
                debug!("can't convert body from byte slice to string: {}", err);
                return false;
            }
        };

        let part = match str::from_utf8(&self.0) {
            Ok(part) => part,
            Err(err) => {
                debug!(
                    "can't convert expected part from byte slice to string: {}",
                    err
                );
                return false;
            }
        };

        body.contains(part)
    }
}

#[derive(Debug)]
/// Match part JSON body of a request.
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::body_partial_json;
/// use serde_json::json;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     let expected_body = json!({
///         "hello": "world!"
///     });
///     Mock::given(body_partial_json(&expected_body))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     // Act
///     let body = json!({
///         "hello": "world!",
///         "foo": "bar"
///     });
///     let client = reqwest::Client::new();
///     let status = client.post(&mock_server.uri())
///         .json(&body)
///         .send()
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct BodyPartialJsonMatcher(Value);

impl BodyPartialJsonMatcher {
    /// Specify the part of the body that should be matched as a JSON value.
    pub fn json<T: Serialize>(body: T) -> Self {
        Self(serde_json::to_value(body).expect("Can't serialize to JSON"))
    }

    /// Specify the part of the body that should be matched as a string.
    pub fn json_string(body: impl AsRef<str>) -> Self {
        Self(serde_json::from_str(body.as_ref()).expect("Can't deserialize JSON"))
    }
}

/// Shorthand for [`BodyPartialJsonMatcher::json`].
pub fn body_partial_json<T: Serialize>(body: T) -> BodyPartialJsonMatcher {
    BodyPartialJsonMatcher::json(body)
}

/// Shorthand for [`BodyPartialJsonMatcher::json_string`].
pub fn body_partial_json_string(body: impl AsRef<str>) -> BodyPartialJsonMatcher {
    BodyPartialJsonMatcher::json_string(body)
}

impl Match for BodyPartialJsonMatcher {
    fn matches(&self, request: &Request) -> bool {
        if let Ok(body) = serde_json::from_slice::<Value>(&request.body) {
            let config = assert_json_diff::Config::new(CompareMode::Inclusive);
            assert_json_matches_no_panic(&body, &self.0, config).is_ok()
        } else {
            false
        }
    }
}

#[derive(Debug)]
/// Match **exactly** the query parameter of a request.
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::query_param;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     Mock::given(query_param("hello", "world"))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     // Act
///     let status = reqwest::get(format!("{}?hello=world", &mock_server.uri()))
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct QueryParamExactMatcher(String, String);

impl QueryParamExactMatcher {
    /// Specify the expected value for a query parameter.
    pub fn new<K: Into<String>, V: Into<String>>(key: K, value: V) -> Self {
        let key = key.into();
        let value = value.into();
        Self(key, value)
    }
}

/// Shorthand for [`QueryParamExactMatcher::new`].
pub fn query_param<K, V>(key: K, value: V) -> QueryParamExactMatcher
where
    K: Into<String>,
    V: Into<String>,
{
    QueryParamExactMatcher::new(key, value)
}

impl Match for QueryParamExactMatcher {
    fn matches(&self, request: &Request) -> bool {
        request
            .url
            .query_pairs()
            .any(|q| q.0 == self.0.as_str() && q.1 == self.1.as_str())
    }
}

#[derive(Debug)]
/// Match when a query parameter contains the specified value as a substring.
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::query_param_contains;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     // It matches since "world" is a substring of "some_world".
///     Mock::given(query_param_contains("hello", "world"))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     // Act
///     let status = reqwest::get(format!("{}?hello=some_world", &mock_server.uri()))
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct QueryParamContainsMatcher(String, String);

impl QueryParamContainsMatcher {
    /// Specify the substring that the query parameter should contain.
    pub fn new<K: Into<String>, V: Into<String>>(key: K, value: V) -> Self {
        let key = key.into();
        let value = value.into();
        Self(key, value)
    }
}

/// Shorthand for [`QueryParamContainsMatcher::new`].
pub fn query_param_contains<K, V>(key: K, value: V) -> QueryParamContainsMatcher
where
    K: Into<String>,
    V: Into<String>,
{
    QueryParamContainsMatcher::new(key, value)
}

impl Match for QueryParamContainsMatcher {
    fn matches(&self, request: &Request) -> bool {
        request
            .url
            .query_pairs()
            .any(|q| q.0 == self.0.as_str() && q.1.contains(self.1.as_str()))
    }
}

#[derive(Debug)]
/// Only match requests that do **not** contain a specified query parameter.
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::{method, query_param_is_missing};
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     Mock::given(method("GET"))
///         .and(query_param_is_missing("unexpected"))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     // Act
///     let ok_status = reqwest::get(mock_server.uri().to_string())
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(ok_status, 200);
///
///     // Act
///     let err_status = reqwest::get(format!("{}?unexpected=foo", mock_server.uri()))
///     .await.
///     unwrap().status();
///
///     // Assert
///     assert_eq!(err_status, 404);
/// }
/// ```
pub struct QueryParamIsMissingMatcher(String);

impl QueryParamIsMissingMatcher {
    /// Specify the query parameter that is expected to not exist.
    pub fn new<K: Into<String>>(key: K) -> Self {
        let key = key.into();
        Self(key)
    }
}

/// Shorthand for [`QueryParamIsMissingMatcher::new`].
pub fn query_param_is_missing<K>(key: K) -> QueryParamIsMissingMatcher
where
    K: Into<String>,
{
    QueryParamIsMissingMatcher::new(key)
}

impl Match for QueryParamIsMissingMatcher {
    fn matches(&self, request: &Request) -> bool {
        !request.url.query_pairs().any(|(k, _)| k == self.0)
    }
}
/// Match an incoming request if its body is encoded as JSON and can be deserialized
/// according to the specified schema.
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::body_json_schema;
/// use serde_json::json;
/// use serde::{Deserialize, Serialize};
///
/// // The schema we expect the body to conform to.
/// #[derive(Deserialize, Serialize)]
/// struct Greeting {
///     hello: String,
/// }
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     Mock::given(body_json_schema::<Greeting>)
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     // Both JSON objects have the same fields,
///     // therefore they'll match.
///     let success_cases = vec![
///         json!({"hello": "world!"}),
///         json!({"hello": "everyone!"}),
///     ];
///     let client = reqwest::Client::new();
///     for case in &success_cases {
///         let status = client.post(&mock_server.uri())
///             .json(case)
///             .send()
///             .await
///             .unwrap()
///             .status();
///
///         // Assert
///         assert_eq!(status, 200);
///     }
///
///     // This JSON object cannot be deserialized as `Greeting`
///     // because it does not have the `hello` field.
///     // It won't match.
///     let failure_case = json!({"world": "hello!"});
///     let status = client.post(&mock_server.uri())
///         .json(&failure_case)
///         .send()
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 404);
/// }
/// ```
pub fn body_json_schema<T>(request: &Request) -> bool
where
    for<'de> T: serde::de::Deserialize<'de>,
{
    serde_json::from_slice::<T>(&request.body).is_ok()
}

#[derive(Debug)]
/// Match an incoming request if it contains the basic authentication header with the username and password
/// as per [RFC 7617](https://datatracker.ietf.org/doc/html/rfc7617).
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::basic_auth;
/// use serde::{Deserialize, Serialize};
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///
///     Mock::given(basic_auth("username", "password"))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     let client = reqwest::Client::new();
///
///     // Act
///     let status = client
///         .get(&mock_server.uri())
///         .header("Authorization", "Basic dXNlcm5hbWU6cGFzc3dvcmQ=")
///         .send()
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct BasicAuthMatcher(HeaderExactMatcher);

impl BasicAuthMatcher {
    /// Match basic authentication header using the given username and password.
    pub fn from_credentials(username: impl AsRef<str>, password: impl AsRef<str>) -> Self {
        Self::from_token(BASE64_STANDARD.encode(format!(
            "{}:{}",
            username.as_ref(),
            password.as_ref()
        )))
    }

    /// Match basic authentication header with the exact token given.
    pub fn from_token(token: impl AsRef<str>) -> Self {
        Self(header(
            "Authorization",
            &*format!("Basic {}", token.as_ref()),
        ))
    }
}

/// Shorthand for [`BasicAuthMatcher::from_credentials`].
pub fn basic_auth<U, P>(username: U, password: P) -> BasicAuthMatcher
where
    U: AsRef<str>,
    P: AsRef<str>,
{
    BasicAuthMatcher::from_credentials(username, password)
}

impl Match for BasicAuthMatcher {
    fn matches(&self, request: &Request) -> bool {
        self.0.matches(request)
    }
}

#[derive(Debug)]
/// Match an incoming request if it contains the bearer token header
/// as per [RFC 6750](https://datatracker.ietf.org/doc/html/rfc6750).
///
/// ### Example:
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::bearer_token;
/// use serde::{Deserialize, Serialize};
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     Mock::given(bearer_token("token"))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     let client = reqwest::Client::new();
///
///     // Act
///     let status = client.get(&mock_server.uri())
///         .header("Authorization", "Bearer token")
///         .send()
///         .await
///         .unwrap()
///         .status();
///
///     // Assert
///     assert_eq!(status, 200);
/// }
/// ```
pub struct BearerTokenMatcher(HeaderExactMatcher);

impl BearerTokenMatcher {
    pub fn from_token(token: impl AsRef<str>) -> Self {
        Self(header(
            "Authorization",
            &*format!("Bearer {}", token.as_ref()),
        ))
    }
}

impl Match for BearerTokenMatcher {
    fn matches(&self, request: &Request) -> bool {
        self.0.matches(request)
    }
}

/// Shorthand for [`BearerTokenMatcher::from_token`].
pub fn bearer_token<T>(token: T) -> BearerTokenMatcher
where
    T: AsRef<str>,
{
    BearerTokenMatcher::from_token(token)
}
