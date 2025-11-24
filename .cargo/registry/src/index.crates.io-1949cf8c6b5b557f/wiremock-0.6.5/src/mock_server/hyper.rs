use crate::mock_server::bare_server::MockServerState;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

/// Work around a lifetime error where, for some reason,
/// `Box<dyn std::error::Error + Send + Sync + 'static>` can't be converted to a
/// `Box<dyn std::error::Error + Send + Sync>`
struct ErrorLifetimeCast(Box<dyn std::error::Error + Send + Sync + 'static>);

impl From<ErrorLifetimeCast> for Box<dyn std::error::Error + Send + Sync> {
    fn from(value: ErrorLifetimeCast) -> Self {
        value.0
    }
}

/// The actual HTTP server responding to incoming requests according to the specified mocks.
pub(super) async fn run_server(
    listener: std::net::TcpListener,
    server_state: Arc<RwLock<MockServerState>>,
    mut shutdown_signal: tokio::sync::watch::Receiver<()>,
) {
    listener
        .set_nonblocking(true)
        .expect("Cannot set non-blocking mode on TcpListener");
    let listener = TcpListener::from_std(listener).expect("Cannot upgrade TcpListener");

    let request_handler = move |request| {
        let server_state = server_state.clone();
        async move {
            let wiremock_request = crate::Request::from_hyper(request).await;
            let (response, delay) = server_state
                .write()
                .await
                .handle_request(wiremock_request)
                .await
                .map_err(ErrorLifetimeCast)?;

            // We do not wait for the delay within the handler otherwise we would be
            // holding on to the write-side of the `RwLock` on `mock_set`.
            // Holding on the lock while waiting prevents us from handling other requests until
            // we have waited the whole duration specified in the delay.
            // In particular, we cannot perform even perform read-only operation -
            // e.g. check that mock assumptions have been verified.
            // Using long delays in tests without handling the delay as we are doing here
            // caused tests to hang (see https://github.com/seanmonstar/reqwest/issues/1147)
            if let Some(delay) = delay {
                delay.await;
            }

            Ok::<_, ErrorLifetimeCast>(response)
        }
    };

    loop {
        let (stream, _) = tokio::select! { biased;
            accepted = listener.accept() => {
                match accepted {
                    Ok(accepted) => accepted,
                    Err(_) => break,
                }
            },
            _ = shutdown_signal.changed() => {
                log::info!("Mock server shutting down");
                break;
            }
        };
        let io = TokioIo::new(stream);

        let request_handler = request_handler.clone();
        let mut shutdown_signal = shutdown_signal.clone();
        tokio::task::spawn(async move {
            let http_server =
                hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());
            let conn = http_server.serve_connection_with_upgrades(io, service_fn(request_handler));
            tokio::pin!(conn);

            loop {
                tokio::select! {
                    _ = conn.as_mut() => break,
                    _ = shutdown_signal.changed() => conn.as_mut().graceful_shutdown(),
                }
            }
        });
    }
}
