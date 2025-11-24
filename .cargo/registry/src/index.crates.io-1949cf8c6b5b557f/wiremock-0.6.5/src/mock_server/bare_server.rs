use crate::mock_server::hyper::run_server;
use crate::mock_set::MockId;
use crate::mock_set::MountedMockSet;
use crate::request::BodyPrintLimit;
use crate::{ErrorResponse, Request, mock::Mock, verification::VerificationOutcome};
use http_body_util::Full;
use hyper::body::Bytes;
use std::fmt::{Debug, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::Notify;
use tokio::sync::RwLock;

/// An HTTP web-server running in the background to behave as one of your dependencies using `Mock`s
/// for testing purposes.
///
/// `BareMockServer` is the actual mock server behind the publicly-exposed `MockServer`, which
/// is instead a thin facade over a `BareMockServer` retrieved from a pool - see `get_pooled_server`
/// for more details.
pub(crate) struct BareMockServer {
    state: Arc<RwLock<MockServerState>>,
    server_address: SocketAddr,
    // When `_shutdown_trigger` gets dropped the listening server terminates gracefully.
    _shutdown_trigger: tokio::sync::watch::Sender<()>,
}

/// The elements of [`BareMockServer`] that are affected by each incoming request.
/// By bundling them together, we can expose a unified `handle_request` that ensures
/// they are kept in sync without having to leak logic across multiple corners of the `wiremock`'s codebase.
pub(super) struct MockServerState {
    mock_set: MountedMockSet,
    received_requests: Option<Vec<Request>>,
    body_print_limit: BodyPrintLimit,
}

impl MockServerState {
    pub(super) async fn handle_request(
        &mut self,
        request: Request,
    ) -> Result<(hyper::Response<Full<Bytes>>, Option<tokio::time::Sleep>), ErrorResponse> {
        // If request recording is enabled, record the incoming request
        // by adding it to the `received_requests` stack
        if let Some(received_requests) = &mut self.received_requests {
            received_requests.push(request.clone());
        }
        self.mock_set.handle_request(request).await
    }
}

impl BareMockServer {
    /// Start a new instance of a `BareMockServer` listening on the specified
    /// [`TcpListener`].
    pub(super) async fn start(
        listener: TcpListener,
        request_recording: RequestRecording,
        body_print_limit: BodyPrintLimit,
    ) -> Self {
        let (shutdown_trigger, shutdown_receiver) = tokio::sync::watch::channel(());
        let received_requests = match request_recording {
            RequestRecording::Enabled => Some(Vec::new()),
            RequestRecording::Disabled => None,
        };
        let state = Arc::new(RwLock::new(MockServerState {
            mock_set: MountedMockSet::new(body_print_limit),
            received_requests,
            body_print_limit,
        }));
        let server_address = listener
            .local_addr()
            .expect("Failed to get server address.");

        let server_state = state.clone();
        std::thread::spawn(move || {
            let server_future = run_server(listener, server_state, shutdown_receiver);

            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Cannot build local tokio runtime");

            runtime.block_on(server_future);
        });
        for _ in 0..40 {
            if TcpStream::connect_timeout(&server_address, std::time::Duration::from_millis(25))
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }

        Self {
            state,
            server_address,
            _shutdown_trigger: shutdown_trigger,
        }
    }

    /// Register a `Mock` on an instance of `BareMockServer`.
    ///
    /// Be careful! `Mock`s are not effective until they are `mount`ed or `register`ed on a
    /// `BareMockServer`.
    pub(crate) async fn register(&self, mock: Mock) {
        self.state.write().await.mock_set.register(mock);
    }

    /// Register a **scoped** `Mock` on an instance of `MockServer`.
    ///
    /// When using `register`, your `Mock`s will be active until the `MockServer` is shut down.  
    /// When using `register_as_scoped`, your `Mock`s will be active as long as the returned `MockGuard` is not dropped.
    /// When the returned `MockGuard` is dropped, `MockServer` will verify that the expectations set on the scoped `Mock` were
    /// verified - if not, it will panic.
    pub async fn register_as_scoped(&self, mock: Mock) -> MockGuard {
        let (notify, mock_id) = self.state.write().await.mock_set.register(mock);
        MockGuard {
            notify,
            mock_id,
            server_state: self.state.clone(),
        }
    }

    /// Drop all mounted `Mock`s from an instance of `BareMockServer`.
    /// Delete all recorded requests.
    ///
    /// It *must* be called if you plan to reuse a `BareMockServer` instance (i.e. in our
    /// `MockServerPoolManager`).
    pub(crate) async fn reset(&self) {
        let mut state = self.state.write().await;
        state.mock_set.reset();
        if let Some(received_requests) = &mut state.received_requests {
            received_requests.clear();
        }
    }

    /// Verify that all mounted `Mock`s on this instance of `BareMockServer` have satisfied
    /// their expectations on their number of invocations.
    pub(crate) async fn verify(&self) -> VerificationOutcome {
        let mock_set = &self.state.read().await.mock_set;
        mock_set.verify_all()
    }

    /// Return the base uri of this running instance of `BareMockServer`, e.g. `http://127.0.0.1:4372`.
    ///
    /// Use this method to compose uris when interacting with this instance of `BareMockServer` via
    /// an HTTP client.
    pub(crate) fn uri(&self) -> String {
        format!("http://{}", self.server_address)
    }

    /// Return the socket address of this running instance of `BareMockServer`, e.g. `127.0.0.1:4372`.
    ///
    /// Use this method to interact with the `BareMockServer` using `TcpStream`s.
    pub(crate) fn address(&self) -> &SocketAddr {
        &self.server_address
    }

    /// Return the body print limit of this running instance of `BareMockServer`.
    pub(crate) async fn body_print_limit(&self) -> BodyPrintLimit {
        self.state.read().await.body_print_limit
    }

    /// Return a vector with all the requests received by the `BareMockServer` since it started.  
    /// If no request has been served, it returns an empty vector.
    ///
    /// If request recording was disabled, it returns `None`.
    pub(crate) async fn received_requests(&self) -> Option<Vec<Request>> {
        let state = self.state.read().await;
        state.received_requests.clone()
    }
}

impl Debug for BareMockServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BareMockServer {{ address: {} }}", self.address())
    }
}

pub(super) enum RequestRecording {
    Enabled,
    Disabled,
}

/// You get a `MockGuard` when registering a **scoped** [`Mock`] using [`MockServer::register_as_scoped`](crate::MockServer::register_as_scoped)
/// or [`Mock::mount_as_scoped`].
///
/// When the [`MockGuard`] is dropped, the [`MockServer`](crate::MockServer) verifies that the expectations set on the
/// scoped [`Mock`] were verified - if not, it will panic.
///
/// # Limitations
///
/// When expectations of a scoped [`Mock`] are not verified, it will trigger a panic - just like a normal [`Mock`].
/// Due to [limitations](https://internals.rust-lang.org/t/should-drop-glue-use-track-caller/13682) in Rust's `Drop` trait,
/// the panic message will not include the filename and the line location
/// where the corresponding `MockGuard` was dropped - it will point into `wiremock`'s source code.
///
/// This can be an issue when you are using more than one scoped [`Mock`] in a single test - which of them panicked?
/// To improve your debugging experience it is strongly recommended to use [`Mock::named`] to assign a unique
/// identifier to your scoped [`Mock`]s, which will in turn be referenced in the panic message if their expectations are
/// not met.
#[must_use = "All *_scoped methods return a `MockGuard`.
This guard MUST be bound to a variable (e.g. _mock_guard), \
otherwise the mock will immediately be unmounted (and its expectations checked).
Check `wiremock`'s documentation on scoped mocks for more details."]
pub struct MockGuard {
    mock_id: MockId,
    server_state: Arc<RwLock<MockServerState>>,
    notify: Arc<(Notify, AtomicBool)>,
}

impl MockGuard {
    /// Return all the requests that have been matched by the corresponding
    /// scoped [`Mock`] since it was mounted.  
    /// The requests are returned in the order they were received.
    ///
    /// It returns an empty vector if no request has been matched.
    pub async fn received_requests(&self) -> Vec<crate::Request> {
        let state = self.server_state.read().await;
        let (mounted_mock, _) = &state.mock_set[self.mock_id];
        mounted_mock.received_requests()
    }

    /// This method doesn't return until the expectations set on the
    /// corresponding scoped [`Mock`] are satisfied.
    ///
    /// It can be useful when you are testing asynchronous flows (e.g. a
    /// message queue consumer) and you don't have a good event that can be used
    /// to trigger the verification of the expectations set on the scoped [`Mock`].
    ///
    /// # Timeouts
    ///
    /// There is no default timeout for this method, so it will end up waiting
    /// **forever** if your expectations are never met. Probably not what you
    /// want.
    ///
    /// It is strongly recommended that you set your own timeout using the
    /// appropriate timers from your chosen async runtime.  
    /// Since `wiremock` is runtime-agnostic, it cannot provide a default
    /// timeout mechanism that would work for all users.
    ///
    /// ```rust
    /// use std::time::Duration;
    /// use wiremock::{Mock, MockServer, ResponseTemplate};
    /// use wiremock::matchers::method;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     // Arrange
    ///     let mock_server = MockServer::start().await;
    ///
    ///     let response = ResponseTemplate::new(200);
    ///     let mock = Mock::given(method("GET")).respond_with(response);
    ///     let mock_guard = mock_server.register_as_scoped(mock).await;
    ///     
    ///     // Act
    ///     let waiter = mock_guard.wait_until_satisfied();
    ///     // Here we wrap the waiter in a tokio timeout
    ///     let outcome = tokio::time::timeout(Duration::from_millis(10), waiter).await;
    ///
    ///     // Assert
    ///     assert!(outcome.is_err());
    /// }
    /// ```
    pub async fn wait_until_satisfied(&self) {
        let (notify, flag) = &*self.notify;
        let mut notification = pin!(notify.notified());

        // listen for events of satisfaction.
        notification.as_mut().enable();

        // check if satisfaction has previously been recorded
        if flag.load(std::sync::atomic::Ordering::Acquire) {
            return;
        }

        // await event
        notification.await;
    }
}

impl Drop for MockGuard {
    fn drop(&mut self) {
        let future = async move {
            let MockGuard {
                mock_id,
                server_state,
                ..
            } = self;
            let mut state = server_state.write().await;
            let report = state.mock_set.verify(*mock_id);

            if !report.is_satisfied() {
                let received_requests_message = if let Some(received_requests) =
                    &state.received_requests
                {
                    if received_requests.is_empty() {
                        "The server did not receive any request.".into()
                    } else {
                        received_requests.iter().enumerate().fold(
                            "Received requests:\n".to_string(),
                            |mut message, (index, request)| {
                                _ = write!(message, "- Request #{}\n\t", index + 1,);
                                _ = request.print_with_limit(&mut message, state.body_print_limit);
                                message
                            },
                        )
                    }
                } else {
                    "Enable request recording on the mock server to get the list of incoming requests as part of the panic message.".into()
                };

                let verifications_error = format!("- {}\n", report.error_message());
                let error_message = format!(
                    "Verification failed for a scoped mock:\n{}\n{}",
                    verifications_error, received_requests_message
                );
                if std::thread::panicking() {
                    log::debug!("{}", &error_message);
                } else {
                    panic!("{}", &error_message);
                }
            } else {
                state.mock_set.deactivate(*mock_id);
            }
        };
        futures::executor::block_on(future);
    }
}
