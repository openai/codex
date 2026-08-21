use std::collections::VecDeque;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use futures::FutureExt;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;

use super::ExecServerClient;
use super::ExecServerReconnectStrategy;
use super::INITIAL_REGISTRY_MAX_RETRIES;
use super::INITIAL_REGISTRY_OPERATION_TIMEOUT;
use super::INITIAL_REGISTRY_REQUEST_TIMEOUT;
use crate::ExecServerError;
use crate::NoiseChannelIdentity;
use crate::NoiseChannelPublicKey;
use crate::NoiseRendezvousConnectBundle;
use crate::NoiseRendezvousConnectProvider;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT;

#[derive(Default)]
struct SequenceNoiseConnectProvider {
    bundles:
        Mutex<VecDeque<BoxFuture<'static, Result<NoiseRendezvousConnectBundle, ExecServerError>>>>,
    returned_urls: Mutex<Vec<String>>,
    requested_keys: Mutex<Vec<NoiseChannelPublicKey>>,
}

impl SequenceNoiseConnectProvider {
    fn push_response(
        &self,
        response: impl Future<Output = Result<NoiseRendezvousConnectBundle, ExecServerError>>
        + Send
        + 'static,
    ) {
        self.bundles.lock().unwrap().push_back(response.boxed());
    }

    fn push_error(&self, error: ExecServerError) {
        self.push_response(futures::future::ready(Err(error)));
    }

    fn push_pending(&self) {
        self.push_response(futures::future::pending());
    }

    fn requested_keys(&self) -> Vec<NoiseChannelPublicKey> {
        self.requested_keys.lock().unwrap().clone()
    }

    fn assert_requested_identity(&self, identity: &NoiseChannelIdentity, requests: usize) {
        assert_eq!(self.requested_keys(), vec![identity.public_key(); requests]);
    }

    fn returned_urls(&self) -> Vec<String> {
        self.returned_urls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    async fn connect(
        self: &Arc<Self>,
        identity: &NoiseChannelIdentity,
    ) -> Result<
        (
            super::JsonRpcConnection,
            super::ExecServerClientConnectOptions,
        ),
        ExecServerError,
    > {
        let provider: Arc<dyn NoiseRendezvousConnectProvider> = self.clone();
        ExecServerClient::open_initial_noise_rendezvous_connection(
            &provider,
            identity,
            codex_http_client::HttpClientFactory::new(
                codex_http_client::OutboundProxyPolicy::ReqwestDefault,
            ),
        )
        .await
    }
}

impl NoiseRendezvousConnectProvider for SequenceNoiseConnectProvider {
    fn connect_bundle(
        &self,
        harness_public_key: NoiseChannelPublicKey,
    ) -> BoxFuture<'_, Result<NoiseRendezvousConnectBundle, ExecServerError>> {
        self.requested_keys.lock().unwrap().push(harness_public_key);
        let response = self
            .bundles
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
            .expect("test Noise provider exhausted");
        Box::pin(async move {
            let result = response.await;
            if let Ok(bundle) = &result {
                self.returned_urls
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(bundle.websocket_url.clone());
            }
            result
        })
    }
}

fn test_bundle(websocket_url: String) -> Result<NoiseRendezvousConnectBundle> {
    Ok(NoiseRendezvousConnectBundle {
        websocket_url,
        environment_id: "environment".to_string(),
        executor_registration_id: "registration".to_string(),
        executor_public_key: NoiseChannelIdentity::generate()?.public_key(),
        harness_key_authorization: "authorization".to_string(),
    })
}

fn registry_error(status: http::StatusCode, code: &str) -> ExecServerError {
    ExecServerError::EnvironmentRegistryHttp {
        status,
        code: Some(code.to_string()),
        message: "registry unavailable".to_string(),
    }
}

#[tokio::test(start_paused = true)]
async fn initial_noise_connection_bounds_offline_retries() -> Result<()> {
    let sequence = Arc::new(SequenceNoiseConnectProvider::default());
    for _ in 0..=INITIAL_REGISTRY_MAX_RETRIES {
        sequence.push_error(registry_error(
            http::StatusCode::CONFLICT,
            "environment_offline",
        ));
    }
    let identity = NoiseChannelIdentity::generate()?;
    let started = tokio::time::Instant::now();
    let error = sequence
        .connect(&identity)
        .await
        .err()
        .expect("offline retries must end");

    assert!(crate::client::is_environment_offline_error(&error));
    let requests = sequence.requested_keys().len();
    assert!((4..=INITIAL_REGISTRY_MAX_RETRIES as usize + 1).contains(&requests));
    sequence.assert_requested_identity(&identity, requests);
    assert!(started.elapsed() <= INITIAL_REGISTRY_OPERATION_TIMEOUT);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn initial_noise_connection_bounds_a_stalled_retry_request() -> Result<()> {
    let sequence = Arc::new(SequenceNoiseConnectProvider::default());
    sequence.push_error(registry_error(
        http::StatusCode::CONFLICT,
        "environment_offline",
    ));
    for _ in 0..INITIAL_REGISTRY_MAX_RETRIES {
        sequence.push_pending();
    }
    let identity = NoiseChannelIdentity::generate()?;
    let started = tokio::time::Instant::now();
    let error = sequence
        .connect(&identity)
        .await
        .err()
        .expect("stalled retry must time out");

    assert!(matches!(
        error,
        ExecServerError::EnvironmentRegistryRequest(error) if error.is_timeout()
    ));
    assert_eq!(started.elapsed(), INITIAL_REGISTRY_OPERATION_TIMEOUT);
    let requests = sequence.requested_keys().len();
    assert!((2..=3).contains(&requests));
    sequence.assert_requested_identity(&identity, requests);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn initial_noise_connection_bounds_a_stalled_initial_request() -> Result<()> {
    let sequence = Arc::new(SequenceNoiseConnectProvider::default());
    for _ in 0..=INITIAL_REGISTRY_MAX_RETRIES {
        sequence.push_pending();
    }
    let identity = NoiseChannelIdentity::generate()?;
    let started = tokio::time::Instant::now();

    let error = sequence
        .connect(&identity)
        .await
        .err()
        .expect("stalled initial request must time out");

    assert!(matches!(
        error,
        ExecServerError::EnvironmentRegistryRequest(error) if error.is_timeout()
    ));
    assert_eq!(started.elapsed(), INITIAL_REGISTRY_OPERATION_TIMEOUT);
    let requests = sequence.requested_keys().len();
    assert!((2..=3).contains(&requests));
    sequence.assert_requested_identity(&identity, requests);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn initial_noise_connection_retries_a_stalled_initial_request() -> Result<()> {
    let sequence = Arc::new(SequenceNoiseConnectProvider::default());
    sequence.push_pending();
    sequence.push_error(registry_error(http::StatusCode::FORBIDDEN, "forbidden"));
    let identity = NoiseChannelIdentity::generate()?;
    let started = tokio::time::Instant::now();

    let error = sequence
        .connect(&identity)
        .await
        .err()
        .expect("terminal response must stop the retry sequence");

    assert!(matches!(
        error,
        ExecServerError::EnvironmentRegistryHttp {
            status: http::StatusCode::FORBIDDEN,
            ..
        }
    ));
    assert!(started.elapsed() >= INITIAL_REGISTRY_REQUEST_TIMEOUT);
    assert!(started.elapsed() < INITIAL_REGISTRY_OPERATION_TIMEOUT);
    sequence.assert_requested_identity(&identity, /*requests*/ 2);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn initial_noise_connection_retries_transient_registry_statuses() -> Result<()> {
    for status in [
        http::StatusCode::REQUEST_TIMEOUT,
        http::StatusCode::TOO_MANY_REQUESTS,
        http::StatusCode::INTERNAL_SERVER_ERROR,
        http::StatusCode::BAD_GATEWAY,
        http::StatusCode::SERVICE_UNAVAILABLE,
    ] {
        let sequence = Arc::new(SequenceNoiseConnectProvider::default());
        sequence.push_error(registry_error(status, "temporarily_unavailable"));
        sequence.push_error(registry_error(http::StatusCode::FORBIDDEN, "forbidden"));
        let identity = NoiseChannelIdentity::generate()?;

        let error = sequence
            .connect(&identity)
            .await
            .err()
            .expect("terminal response must stop the retry sequence");

        assert!(matches!(
            error,
            ExecServerError::EnvironmentRegistryHttp {
                status: http::StatusCode::FORBIDDEN,
                ..
            }
        ));
        sequence.assert_requested_identity(&identity, /*requests*/ 2);
    }
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn initial_noise_connection_retries_registry_request_timeouts() -> Result<()> {
    let sequence = Arc::new(SequenceNoiseConnectProvider::default());
    sequence.push_error(ExecServerError::EnvironmentRegistryRequest(
        codex_http_client::RouteAwareRequestError::Timeout,
    ));
    sequence.push_error(registry_error(http::StatusCode::FORBIDDEN, "forbidden"));
    let identity = NoiseChannelIdentity::generate()?;

    let error = sequence
        .connect(&identity)
        .await
        .err()
        .expect("terminal response must stop the retry sequence");

    assert!(matches!(
        error,
        ExecServerError::EnvironmentRegistryHttp {
            status: http::StatusCode::FORBIDDEN,
            ..
        }
    ));
    sequence.assert_requested_identity(&identity, /*requests*/ 2);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn initial_noise_connection_does_not_retry_permanent_registry_errors() -> Result<()> {
    for (status, code) in [
        (http::StatusCode::UNAUTHORIZED, "unauthorized"),
        (http::StatusCode::FORBIDDEN, "forbidden"),
        (http::StatusCode::BAD_REQUEST, "bad_request"),
        (http::StatusCode::NOT_FOUND, "environment_not_found"),
        (http::StatusCode::CONFLICT, "registration_conflict"),
        (http::StatusCode::CONFLICT, "route_unavailable"),
    ] {
        // A terminal error must also stop a retry sequence already in progress.
        for initial_offline in [false, true] {
            let sequence = Arc::new(SequenceNoiseConnectProvider::default());
            if initial_offline {
                sequence.push_error(registry_error(
                    http::StatusCode::CONFLICT,
                    "environment_offline",
                ));
            }
            sequence.push_error(registry_error(status, code));
            let identity = NoiseChannelIdentity::generate()?;
            let error = sequence
                .connect(&identity)
                .await
                .err()
                .expect("other errors must propagate");
            assert!(
                matches!(error, ExecServerError::EnvironmentRegistryHttp { status: actual_status, code: Some(actual_code), .. } if actual_status == status && actual_code == code)
            );
            sequence.assert_requested_identity(&identity, 1 + usize::from(initial_offline));
        }
    }
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn noise_session_resume_leaves_offline_retries_to_recovery() -> Result<()> {
    let sequence = Arc::new(SequenceNoiseConnectProvider::default());
    sequence.push_error(registry_error(
        http::StatusCode::CONFLICT,
        "environment_offline",
    ));
    let identity = NoiseChannelIdentity::generate()?;
    let strategy = ExecServerReconnectStrategy::NoiseRendezvous {
        provider: sequence.clone(),
        identity: identity.clone(),
        client_name: "test".to_string(),
        connect_timeout: DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
        initialize_timeout: DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT,
        http_client_factory: codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    };
    let started = tokio::time::Instant::now();
    let error = strategy
        .resume("session")
        .await
        .err()
        .expect("resume must return the offline error");
    assert!(crate::client::is_environment_offline_error(&error));
    assert_eq!(started.elapsed(), std::time::Duration::ZERO);
    sequence.assert_requested_identity(&identity, /*requests*/ 1);
    Ok(())
}

#[tokio::test]
async fn initial_noise_connection_refreshes_bundle_after_exhausting_initial_retries() -> Result<()>
{
    let unauthorized_listener = TcpListener::bind("127.0.0.1:0").await?;
    let unauthorized_url = format!("ws://{}", unauthorized_listener.local_addr()?);
    let unauthorized_server = tokio::spawn(async move {
        let (mut socket, _) = unauthorized_listener.accept().await?;
        let mut request = [0_u8; 4096];
        let _ = socket.read(&mut request).await?;
        socket
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await?;
        socket.shutdown().await?;
        anyhow::Ok(())
    });
    let accepted_listener = TcpListener::bind("127.0.0.1:0").await?;
    let accepted_url = format!("ws://{}", accepted_listener.local_addr()?);
    let accepted_server = tokio::spawn(async move {
        let (socket, _) = accepted_listener.accept().await?;
        let _websocket = accept_async(socket).await?;
        anyhow::Ok(())
    });
    let sequence = Arc::new(SequenceNoiseConnectProvider::default());
    let unauthorized_bundle = test_bundle(unauthorized_url.clone())?;
    let accepted_bundle = test_bundle(accepted_url.clone())?;
    sequence.push_response(async {
        tokio::time::pause();
        Err(registry_error(
            http::StatusCode::CONFLICT,
            "environment_offline",
        ))
    });
    for _ in 1..INITIAL_REGISTRY_MAX_RETRIES {
        sequence.push_error(registry_error(
            http::StatusCode::CONFLICT,
            "environment_offline",
        ));
    }
    sequence.push_response(async move {
        tokio::time::resume();
        Ok(unauthorized_bundle)
    });
    sequence.push_response(async {
        tokio::time::pause();
        Err(registry_error(
            http::StatusCode::CONFLICT,
            "environment_offline",
        ))
    });
    sequence.push_response(async move {
        tokio::time::resume();
        Ok(accepted_bundle)
    });
    let identity = NoiseChannelIdentity::generate()?;

    let _connection = sequence.connect(&identity).await?;

    assert_eq!(
        sequence.returned_urls(),
        vec![unauthorized_url, accepted_url]
    );
    sequence.assert_requested_identity(&identity, INITIAL_REGISTRY_MAX_RETRIES as usize + 3);
    unauthorized_server.await??;
    accepted_server.await??;
    Ok(())
}
