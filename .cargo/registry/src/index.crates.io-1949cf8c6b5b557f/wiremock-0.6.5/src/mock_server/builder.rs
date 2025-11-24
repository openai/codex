use crate::MockServer;
use crate::mock_server::bare_server::{BareMockServer, RequestRecording};
use crate::mock_server::exposed_server::InnerServer;
use crate::request::{BODY_PRINT_LIMIT, BodyPrintLimit};
use std::env;
use std::net::TcpListener;

/// A builder providing a fluent API to assemble a [`MockServer`] step-by-step.  
/// Use [`MockServer::builder`] to get started.
pub struct MockServerBuilder {
    listener: Option<TcpListener>,
    record_incoming_requests: bool,
    body_print_limit: BodyPrintLimit,
}

impl MockServerBuilder {
    pub(super) fn new() -> Self {
        let body_print_limit = match env::var("WIREMOCK_BODY_PRINT_LIMIT")
            .ok()
            .and_then(|x| x.parse::<usize>().ok())
        {
            Some(limit) => BodyPrintLimit::Limited(limit),
            None => BodyPrintLimit::Limited(BODY_PRINT_LIMIT),
        };
        Self {
            listener: None,
            record_incoming_requests: true,
            body_print_limit,
        }
    }

    /// Each instance of [`MockServer`] is, by default, running on a random
    /// port available on your local machine.
    /// With `MockServerBuilder::listener` you can choose to start the `MockServer`
    /// instance on a specific port you have already bound.
    ///
    /// ### Example:
    /// ```rust
    /// use wiremock::MockServer;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     // Arrange
    ///     let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    ///     let expected_server_address = listener
    ///         .local_addr()
    ///         .expect("Failed to get server address.");
    ///
    ///     // Act
    ///     let mock_server = MockServer::builder().listener(listener).start().await;
    ///
    ///     // Assert
    ///     assert_eq!(&expected_server_address, mock_server.address());
    /// }
    /// ```
    pub fn listener(mut self, listener: TcpListener) -> Self {
        self.listener = Some(listener);
        self
    }

    /// By default, [`MockServer`] will record all incoming requests to display
    /// more meaningful error messages when your expectations are not verified.
    ///
    /// This can sometimes be undesirable (e.g. a long-lived server serving
    /// high volumes of traffic) - you can disable request recording using
    /// `MockServerBuilder::disable_request_recording`.
    ///
    /// ### Example (Request recording disabled):
    ///
    /// ```rust
    /// use wiremock::MockServer;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     // Arrange
    ///     let mock_server = MockServer::builder().disable_request_recording().start().await;
    ///
    ///     // Act
    ///     let received_requests = mock_server.received_requests().await;
    ///     
    ///     // Assert
    ///     assert!(received_requests.is_none());
    /// }
    /// ```
    pub fn disable_request_recording(mut self) -> Self {
        self.record_incoming_requests = false;
        self
    }

    /// The mock server prints the requests it received when one or more mocks have expectations that have not been satisfied.
    /// By default, the size of the printed body is limited.
    ///
    /// You may want to change this if you're working with services with very large
    /// bodies, or when printing wiremock output to a file where size matters
    /// less than in a terminal window. You can configure this limit with
    /// `MockServerBuilder::body_print_limit`.
    pub fn body_print_limit(mut self, limit: BodyPrintLimit) -> Self {
        self.body_print_limit = limit;
        self
    }

    /// Finalise the builder to get an instance of a [`BareMockServer`].
    pub(super) async fn build_bare(self) -> BareMockServer {
        let listener = if let Some(listener) = self.listener {
            listener
        } else {
            TcpListener::bind("127.0.0.1:0").expect("Failed to bind an OS port for a mock server.")
        };
        let recording = if self.record_incoming_requests {
            RequestRecording::Enabled
        } else {
            RequestRecording::Disabled
        };
        BareMockServer::start(listener, recording, self.body_print_limit).await
    }

    /// Finalise the builder and launch the [`MockServer`] instance!
    pub async fn start(self) -> MockServer {
        MockServer::new(InnerServer::Bare(self.build_bare().await))
    }
}
