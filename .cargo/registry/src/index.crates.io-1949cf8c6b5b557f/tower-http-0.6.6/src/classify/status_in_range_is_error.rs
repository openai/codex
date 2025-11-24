use super::{ClassifiedResponse, ClassifyResponse, NeverClassifyEos, SharedClassifier};
use http::StatusCode;
use std::{fmt, ops::RangeInclusive};

/// Response classifier that considers responses with a status code within some range to be
/// failures.
///
/// # Example
///
/// A client with tracing where server errors _and_ client errors are considered failures.
///
/// ```no_run
/// use tower_http::{trace::TraceLayer, classify::StatusInRangeAsFailures};
/// use tower::{ServiceBuilder, Service, ServiceExt};
/// use http::{Request, Method};
/// use http_body_util::Full;
/// use bytes::Bytes;
/// use hyper_util::{rt::TokioExecutor, client::legacy::Client};
///
/// # async fn foo() -> Result<(), tower::BoxError> {
/// let classifier = StatusInRangeAsFailures::new(400..=599);
///
/// let client = Client::builder(TokioExecutor::new()).build_http();
/// let mut client = ServiceBuilder::new()
///     .layer(TraceLayer::new(classifier.into_make_classifier()))
///     .service(client);
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("https://example.com")
///     .body(Full::<Bytes>::default())
///     .unwrap();
///
/// let response = client.ready().await?.call(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct StatusInRangeAsFailures {
    range: RangeInclusive<u16>,
}

impl StatusInRangeAsFailures {
    /// Creates a new `StatusInRangeAsFailures`.
    ///
    /// # Panics
    ///
    /// Panics if the start or end of `range` aren't valid status codes as determined by
    /// [`StatusCode::from_u16`].
    ///
    /// [`StatusCode::from_u16`]: https://docs.rs/http/latest/http/status/struct.StatusCode.html#method.from_u16
    pub fn new(range: RangeInclusive<u16>) -> Self {
        assert!(
            StatusCode::from_u16(*range.start()).is_ok(),
            "range start isn't a valid status code"
        );
        assert!(
            StatusCode::from_u16(*range.end()).is_ok(),
            "range end isn't a valid status code"
        );

        Self { range }
    }

    /// Creates a new `StatusInRangeAsFailures` that classifies client and server responses as
    /// failures.
    ///
    /// This is a convenience for `StatusInRangeAsFailures::new(400..=599)`.
    pub fn new_for_client_and_server_errors() -> Self {
        Self::new(400..=599)
    }

    /// Convert this `StatusInRangeAsFailures` into a [`MakeClassifier`].
    ///
    /// [`MakeClassifier`]: super::MakeClassifier
    pub fn into_make_classifier(self) -> SharedClassifier<Self> {
        SharedClassifier::new(self)
    }
}

impl ClassifyResponse for StatusInRangeAsFailures {
    type FailureClass = StatusInRangeFailureClass;
    type ClassifyEos = NeverClassifyEos<Self::FailureClass>;

    fn classify_response<B>(
        self,
        res: &http::Response<B>,
    ) -> ClassifiedResponse<Self::FailureClass, Self::ClassifyEos> {
        if self.range.contains(&res.status().as_u16()) {
            let class = StatusInRangeFailureClass::StatusCode(res.status());
            ClassifiedResponse::Ready(Err(class))
        } else {
            ClassifiedResponse::Ready(Ok(()))
        }
    }

    fn classify_error<E>(self, error: &E) -> Self::FailureClass
    where
        E: std::fmt::Display + 'static,
    {
        StatusInRangeFailureClass::Error(error.to_string())
    }
}

/// The failure class for [`StatusInRangeAsFailures`].
#[derive(Debug)]
pub enum StatusInRangeFailureClass {
    /// A response was classified as a failure with the corresponding status.
    StatusCode(StatusCode),
    /// A response was classified as an error with the corresponding error description.
    Error(String),
}

impl fmt::Display for StatusInRangeFailureClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StatusCode(code) => write!(f, "Status code: {}", code),
            Self::Error(error) => write!(f, "Error: {}", error),
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use http::Response;

    #[test]
    fn basic() {
        let classifier = StatusInRangeAsFailures::new(400..=599);

        assert!(matches!(
            classifier
                .clone()
                .classify_response(&response_with_status(200)),
            ClassifiedResponse::Ready(Ok(())),
        ));

        assert!(matches!(
            classifier
                .clone()
                .classify_response(&response_with_status(400)),
            ClassifiedResponse::Ready(Err(StatusInRangeFailureClass::StatusCode(
                StatusCode::BAD_REQUEST
            ))),
        ));

        assert!(matches!(
            classifier.classify_response(&response_with_status(500)),
            ClassifiedResponse::Ready(Err(StatusInRangeFailureClass::StatusCode(
                StatusCode::INTERNAL_SERVER_ERROR
            ))),
        ));
    }

    fn response_with_status(status: u16) -> Response<()> {
        Response::builder().status(status).body(()).unwrap()
    }
}
