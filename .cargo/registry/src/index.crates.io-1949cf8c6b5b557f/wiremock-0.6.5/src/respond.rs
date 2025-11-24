use crate::{ErrorResponse, Request, ResponseTemplate};

/// Anything that implements `Respond` can be used to reply to an incoming request when a
/// [`Mock`] is activated.
///
/// ## Fixed responses
///
/// The simplest `Respond` is [`ResponseTemplate`]: no matter the request, it will
/// always return itself.
///
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
///     let response = reqwest::get(&mock_server.uri())
///         .await
///         .unwrap();
///
///     // Assert
///     assert_eq!(response.status(), 200);
///     assert_eq!(response.headers().get("X-Correlation-ID").unwrap().to_str().unwrap(), correlation_id);
/// }
/// ```
///
/// ## Dynamic responses
///
/// You can use `Respond`, though, to implement responses that depend on the data in
/// the request matched by a [`Mock`].  
///
/// Functions from `Request` to `ResponseTemplate` implement `Respond`, so for simple cases you
/// can use a closure to build a response dynamically, for instance to echo the request body back:
///
/// ```rust
/// use wiremock::{Match, MockServer, Mock, Request, ResponseTemplate};
/// use wiremock::matchers::path;
///
/// #[async_std::main]
/// async fn main() {
///     let mock_server = MockServer::start().await;
///     let body = "Mock Server!".to_string();
///
///     Mock::given(path("/echo"))
///         .respond_with(|req: &Request| {
///             let body_string = String::from_utf8(req.body.clone()).unwrap();
///             ResponseTemplate::new(200).set_body_string(body_string)
///         })
///         .mount(&mock_server)
///         .await;
///
///     let client = reqwest::Client::new();
///     let response = client.post(format!("{}/echo", &mock_server.uri()))
///         .body(body.clone())
///         .send()
///         .await
///         .unwrap();
///     assert_eq!(response.status(), 200);
///     assert_eq!(response.text().await.unwrap(), body);
/// }
/// ```
///
/// For more complex cases you may want to implement `Respond` yourself. As an example, this is a
/// `Respond` that propagates back a request header in the response:
///
/// ```rust
/// use http::HeaderName;
/// use wiremock::{Match, MockServer, Mock, Request, ResponseTemplate, Respond};
/// use wiremock::matchers::path;
/// use std::convert::TryInto;
/// use std::str::FromStr;
///
/// /// Responds using the specified `ResponseTemplate`, but it dynamically populates the
/// /// `X-Correlation-Id` header from the request data.
/// pub struct CorrelationIdResponder(pub ResponseTemplate);
///
/// impl Respond for CorrelationIdResponder {
///     fn respond(&self, request: &Request) -> ResponseTemplate {
///         const HEADER: HeaderName = HeaderName::from_static("x-correlation-id");
///         let mut response_template = self.0.clone();
///         if let Some(correlation_id) = request.headers.get(&HEADER) {
///             response_template = response_template.insert_header(
///                 HEADER,
///                 correlation_id.to_owned()
///             );
///         }
///         response_template
///     }
/// }
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///     let correlation_id = "1241-1245-1548-4567";
///
///     Mock::given(path("/hello"))
///         .respond_with(CorrelationIdResponder(ResponseTemplate::new(200)))
///         .mount(&mock_server)
///         .await;
///
///     let client = reqwest::Client::new();
///     let response = client
///         .get(format!("{}/hello", &mock_server.uri()))
///         .header("X-Correlation-Id", correlation_id)
///         .send()
///         .await
///         .unwrap();
///     assert_eq!(response.status(), 200);
///     assert_eq!(response.headers().get("X-Correlation-Id").unwrap().to_str().unwrap(), correlation_id);
/// }
/// ```
///
/// [`Mock`]: crate::Mock
/// [`ResponseTemplate`]: crate::ResponseTemplate
pub trait Respond: Send + Sync {
    /// Given a reference to a [`Request`] return a [`ResponseTemplate`] that will be used
    /// by the [`MockServer`] as blueprint for the response returned to the client.
    ///
    /// [`Request`]: crate::Request
    /// [`MockServer`]: crate::MockServer
    /// [`ResponseTemplate`]: crate::ResponseTemplate
    fn respond(&self, request: &Request) -> ResponseTemplate;
}

/// A `ResponseTemplate` is the simplest `Respond` implementation: it returns a clone of itself
/// no matter what the incoming request contains!
impl Respond for ResponseTemplate {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        self.clone()
    }
}

impl<F> Respond for F
where
    F: Send + Sync + Fn(&Request) -> ResponseTemplate,
{
    fn respond(&self, request: &Request) -> ResponseTemplate {
        (self)(request)
    }
}

/// Like [`Respond`], but it only allows returning an error through a function.
pub trait RespondErr: Send + Sync {
    fn respond_err(&self, request: &Request) -> ErrorResponse;
}

impl<F, Err> RespondErr for F
where
    F: Send + Sync + Fn(&Request) -> Err,
    Err: std::error::Error + Send + Sync + 'static,
{
    fn respond_err(&self, request: &Request) -> ErrorResponse {
        Box::new((self)(request))
    }
}
