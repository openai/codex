use std::collections::VecDeque;
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

struct SequenceNoiseConnectProvider {
    bundles:
        Mutex<VecDeque<BoxFuture<'static, Result<NoiseRendezvousConnectBundle, ExecServerError>>>>,
    returned_urls: Mutex<Vec<String>>,
    requested_keys: Mutex<Vec<NoiseChannelPublicKey>>,
}

impl SequenceNoiseConnectProvider {
    fn new(bundles: Vec<Result<NoiseRendezvousConnectBundle, ExecServerError>>) -> Self {
        Self {
            bundles: Mutex::new(
                bundles
                    .into_iter()
                    .map(|bundle| futures::future::ready(bundle).boxed())
                    .collect(),
            ),
            returned_urls: Mutex::new(Vec::new()),
            requested_keys: Mutex::new(Vec::new()),
        }
    }

    fn returned_urls(&self) -> Vec<String> {
        self.returned_urls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
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
    let sequence = Arc::new(SequenceNoiseConnectProvider::new(
        (0..=INITIAL_REGISTRY_MAX_RETRIES)
            .map(|_| {
                Err(registry_error(
                    http::StatusCode::CONFLICT,
                    "environment_offline",
                ))
            })
            .collect(),
    ));
    let provider: Arc<dyn NoiseRendezvousConnectProvider> = sequence.clone();
    let identity = NoiseChannelIdentity::generate()?;
    let started = tokio::time::Instant::now();
    let error = ExecServerClient::open_initial_noise_rendezvous_connection(
        &provider,
        &identity,
        codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    )
    .await
    .err()
    .expect("offline retries must end");

    assert!(crate::client::is_environment_offline_error(&error));
    let requested_keys = sequence.requested_keys.lock().unwrap();
    assert!((4..=INITIAL_REGISTRY_MAX_RETRIES as usize + 1).contains(&requested_keys.len()));
    assert_eq!(
        *requested_keys,
        vec![identity.public_key(); requested_keys.len()]
    );
    assert!(started.elapsed() <= INITIAL_REGISTRY_OPERATION_TIMEOUT);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn initial_noise_connection_bounds_a_stalled_retry_request() -> Result<()> {
    let sequence = Arc::new(SequenceNoiseConnectProvider::new(vec![Err(
        registry_error(http::StatusCode::CONFLICT, "environment_offline"),
    )]));
    sequence
        .bundles
        .lock()
        .unwrap()
        .extend((0..INITIAL_REGISTRY_MAX_RETRIES).map(|_| {
            futures::future::pending::<Result<NoiseRendezvousConnectBundle, ExecServerError>>()
                .boxed()
        }));
    let provider: Arc<dyn NoiseRendezvousConnectProvider> = sequence.clone();
    let identity = NoiseChannelIdentity::generate()?;
    let started = tokio::time::Instant::now();
    let error = ExecServerClient::open_initial_noise_rendezvous_connection(
        &provider,
        &identity,
        codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    )
    .await
    .err()
    .expect("stalled retry must time out");

    assert!(matches!(
        error,
        ExecServerError::EnvironmentRegistryRequest(error) if error.is_timeout()
    ));
    assert_eq!(started.elapsed(), INITIAL_REGISTRY_OPERATION_TIMEOUT);
    let requested_keys = sequence.requested_keys.lock().unwrap();
    assert!((2..=3).contains(&requested_keys.len()));
    assert_eq!(
        *requested_keys,
        vec![identity.public_key(); requested_keys.len()]
    );
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn initial_noise_connection_bounds_a_stalled_initial_request() -> Result<()> {
    let sequence = Arc::new(SequenceNoiseConnectProvider::new(vec![]));
    sequence
        .bundles
        .lock()
        .unwrap()
        .extend((0..=INITIAL_REGISTRY_MAX_RETRIES).map(|_| {
            futures::future::pending::<Result<NoiseRendezvousConnectBundle, ExecServerError>>()
                .boxed()
        }));
    let provider: Arc<dyn NoiseRendezvousConnectProvider> = sequence.clone();
    let identity = NoiseChannelIdentity::generate()?;
    let started = tokio::time::Instant::now();

    let error = ExecServerClient::open_initial_noise_rendezvous_connection(
        &provider,
        &identity,
        codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    )
    .await
    .err()
    .expect("stalled initial request must time out");

    assert!(matches!(
        error,
        ExecServerError::EnvironmentRegistryRequest(error) if error.is_timeout()
    ));
    assert_eq!(started.elapsed(), INITIAL_REGISTRY_OPERATION_TIMEOUT);
    let requested_keys = sequence.requested_keys.lock().unwrap();
    assert!((2..=3).contains(&requested_keys.len()));
    assert_eq!(
        *requested_keys,
        vec![identity.public_key(); requested_keys.len()]
    );
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn initial_noise_connection_retries_a_stalled_initial_request() -> Result<()> {
    let sequence = Arc::new(SequenceNoiseConnectProvider::new(vec![]));
    sequence.bundles.lock().unwrap().extend([
        futures::future::pending::<Result<NoiseRendezvousConnectBundle, ExecServerError>>().boxed(),
        futures::future::ready(Err(registry_error(
            http::StatusCode::FORBIDDEN,
            "forbidden",
        )))
        .boxed(),
    ]);
    let provider: Arc<dyn NoiseRendezvousConnectProvider> = sequence.clone();
    let identity = NoiseChannelIdentity::generate()?;
    let started = tokio::time::Instant::now();

    let error = ExecServerClient::open_initial_noise_rendezvous_connection(
        &provider,
        &identity,
        codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    )
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
    assert_eq!(
        *sequence.requested_keys.lock().unwrap(),
        vec![identity.public_key(); 2]
    );
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
        let sequence = Arc::new(SequenceNoiseConnectProvider::new(vec![
            Err(registry_error(status, "temporarily_unavailable")),
            Err(registry_error(http::StatusCode::FORBIDDEN, "forbidden")),
        ]));
        let provider: Arc<dyn NoiseRendezvousConnectProvider> = sequence.clone();
        let identity = NoiseChannelIdentity::generate()?;

        let error = ExecServerClient::open_initial_noise_rendezvous_connection(
            &provider,
            &identity,
            codex_http_client::HttpClientFactory::new(
                codex_http_client::OutboundProxyPolicy::ReqwestDefault,
            ),
        )
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
        assert_eq!(
            *sequence.requested_keys.lock().unwrap(),
            vec![identity.public_key(); 2],
            "registry status {status} should be retried"
        );
    }
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn initial_noise_connection_retries_registry_request_timeouts() -> Result<()> {
    let sequence = Arc::new(SequenceNoiseConnectProvider::new(vec![
        Err(ExecServerError::EnvironmentRegistryRequest(
            codex_http_client::RouteAwareRequestError::Timeout,
        )),
        Err(registry_error(http::StatusCode::FORBIDDEN, "forbidden")),
    ]));
    let provider: Arc<dyn NoiseRendezvousConnectProvider> = sequence.clone();
    let identity = NoiseChannelIdentity::generate()?;

    let error = ExecServerClient::open_initial_noise_rendezvous_connection(
        &provider,
        &identity,
        codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    )
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
    assert_eq!(
        *sequence.requested_keys.lock().unwrap(),
        vec![identity.public_key(); 2]
    );
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
            let mut responses = Vec::new();
            if initial_offline {
                responses.push(Err(registry_error(
                    http::StatusCode::CONFLICT,
                    "environment_offline",
                )));
            }
            responses.push(Err(registry_error(status, code)));
            let sequence = Arc::new(SequenceNoiseConnectProvider::new(responses));
            let provider: Arc<dyn NoiseRendezvousConnectProvider> = sequence.clone();
            let identity = NoiseChannelIdentity::generate()?;
            let error = ExecServerClient::open_initial_noise_rendezvous_connection(
                &provider,
                &identity,
                codex_http_client::HttpClientFactory::new(
                    codex_http_client::OutboundProxyPolicy::ReqwestDefault,
                ),
            )
            .await
            .err()
            .expect("other errors must propagate");
            assert!(
                matches!(error, ExecServerError::EnvironmentRegistryHttp { status: actual_status, code: Some(actual_code), .. } if actual_status == status && actual_code == code)
            );
            assert_eq!(
                sequence.requested_keys.lock().unwrap().len(),
                1 + usize::from(initial_offline)
            );
        }
    }
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn noise_session_resume_leaves_offline_retries_to_recovery() -> Result<()> {
    let sequence = Arc::new(SequenceNoiseConnectProvider::new(vec![Err(
        registry_error(http::StatusCode::CONFLICT, "environment_offline"),
    )]));
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
    assert_eq!(
        *sequence.requested_keys.lock().unwrap(),
        vec![identity.public_key()]
    );
    Ok(())
}

#[tokio::test]
async fn initial_noise_connection_refreshes_bundle_after_unauthorized_handshake() -> Result<()> {
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
    let sequence = Arc::new(SequenceNoiseConnectProvider::new(vec![]));
    let unauthorized_bundle = test_bundle(unauthorized_url.clone())?;
    let accepted_bundle = test_bundle(accepted_url.clone())?;
    sequence.bundles.lock().unwrap().extend([
        async {
            tokio::time::pause();
            Err(registry_error(
                http::StatusCode::CONFLICT,
                "environment_offline",
            ))
        }
        .boxed(),
        async move {
            tokio::time::resume();
            Ok(unauthorized_bundle)
        }
        .boxed(),
        async {
            tokio::time::pause();
            Err(registry_error(
                http::StatusCode::CONFLICT,
                "environment_offline",
            ))
        }
        .boxed(),
        async move {
            tokio::time::resume();
            Ok(accepted_bundle)
        }
        .boxed(),
    ]);
    let provider: Arc<dyn NoiseRendezvousConnectProvider> = sequence.clone();
    let identity = NoiseChannelIdentity::generate()?;

    let _connection = ExecServerClient::open_initial_noise_rendezvous_connection(
        &provider,
        &identity,
        codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    )
    .await?;

    assert_eq!(
        sequence.returned_urls(),
        vec![unauthorized_url, accepted_url]
    );
    assert_eq!(
        *sequence.requested_keys.lock().unwrap(),
        vec![identity.public_key(); 4]
    );
    unauthorized_server.await??;
    accepted_server.await??;
    Ok(())
}
