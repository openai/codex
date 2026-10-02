//! Pre-write registration failures retry without replacing the Noise identity or session handler.

use std::time::Duration;

use codex_exec_server::ExecServerClient;
use codex_exec_server::NoiseChannelIdentity;
use codex_exec_server::NoiseRendezvousConnectArgs;
use pretty_assertions::assert_eq;
use tokio::sync::oneshot;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;

use super::*;

type RemoteTask = AbortOnDropHandle<Result<(), ExecServerError>>;
type RelayTask = AbortOnDropHandle<Result<()>>;

struct RegistryFixture {
    registry: MockServer,
    listener: TcpListener,
    requests: mpsc::UnboundedReceiver<usize>,
}

impl RegistryFixture {
    async fn new(statuses: Vec<u16>, response_delay: Duration) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let rendezvous_url = format!("ws://{}", listener.local_addr()?);
        let registry = MockServer::start().await;
        let (request_tx, requests) = mpsc::unbounded_channel();
        let attempts = AtomicUsize::new(0);
        Mock::given(method("POST"))
            .and(path(format!("/cloud/environment/{ENVIRONMENT_ID}/register")))
            .respond_with(move |_: &wiremock::Request| {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                let _ = request_tx.send(attempt);
                let status = statuses[(attempt - 1).min(statuses.len() - 1)];
                let mut response = ResponseTemplate::new(status).set_delay(response_delay);
                if status == 200 {
                    response = response.set_body_json(serde_json::json!({
                        "environment_id": ENVIRONMENT_ID,
                        "url": format!("{rendezvous_url}/relay?role=environment&registration={attempt}"),
                        "security_profile": "noise_hybrid_ik_v1",
                        "executor_registration_id": format!("registration-{attempt}"),
                    }));
                } else if status == 502 {
                    response = response.set_body_json(serde_json::json!({
                        "detail": {"code": "authentication_service_unavailable", "message": "Authentication service unavailable"},
                    }));
                } else {
                    response = response.set_body_json(serde_json::json!({
                        "error": {"code": "registration_conflict", "message": "registration conflicted"},
                    }));
                }
                response
            })
            .mount(&registry)
            .await;
        Mock::given(method("POST"))
            .and(path(format!(
                "/cloud/environment/{ENVIRONMENT_ID}/validate"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "valid": true,
            })))
            .mount(&registry)
            .await;
        Ok(Self {
            registry,
            listener,
            requests,
        })
    }

    fn start(&self) -> Result<(oneshot::Sender<()>, RemoteTask)> {
        let config = RemoteEnvironmentConfig::new(
            self.registry.uri(),
            ENVIRONMENT_ID.to_string(),
            static_registry_auth_provider(),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )?;
        let (codex_exe, sandbox_exe) = common::current_test_binary_helper_paths()?;
        let runtime_paths = ExecServerRuntimeOptions::new(codex_exe, sandbox_exe)?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = AbortOnDropHandle::new(tokio::spawn(
            codex_exec_server::run_remote_environment_until_shutdown(
                config,
                runtime_paths,
                async move {
                    let _ = shutdown_rx.await;
                },
            ),
        ));
        Ok((shutdown_tx, task))
    }

    async fn connect(
        &self,
        registration_id: &str,
        harness_identity: &NoiseChannelIdentity,
        resume_session_id: Option<String>,
    ) -> Result<(ExecServerClient, RelayTask)> {
        let environment = accept_websocket(&self.listener, "environment").await?;
        let args = NoiseRendezvousConnectArgs {
            bundle: NoiseRendezvousConnectBundle {
                websocket_url: format!("ws://{}/relay?role=harness", self.listener.local_addr()?),
                environment_id: ENVIRONMENT_ID.to_string(),
                executor_registration_id: registration_id.to_string(),
                executor_public_key: registered_executor_public_key(&self.registry).await?,
                harness_key_authorization: HARNESS_KEY_AUTHORIZATION.to_string(),
            },
            harness_identity: harness_identity.clone(),
            client_name: "registration-retry-test".to_string(),
            connect_timeout: TEST_TIMEOUT,
            initialize_timeout: TEST_TIMEOUT,
            resume_session_id,
            http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        };
        let client = AbortOnDropHandle::new(tokio::spawn(async move {
            ExecServerClient::connect_noise_rendezvous(args).await
        }));
        let harness = accept_websocket(&self.listener, "harness").await?;
        let relay = AbortOnDropHandle::new(tokio::spawn(proxy_relay_frames(
            environment,
            harness,
            Arc::new(Mutex::new(Vec::new())),
        )));
        Ok((timeout(TEST_TIMEOUT, client).await???, relay))
    }

    async fn registered_keys(&self) -> Result<Vec<NoiseChannelPublicKey>> {
        self.registry
            .received_requests()
            .await
            .context("registry should retain requests")?
            .iter()
            .filter(|request| request.url.path().ends_with("/register"))
            .map(|request| {
                let body: serde_json::Value = serde_json::from_slice(&request.body)?;
                Ok(serde_json::from_value(body["executor_public_key"].clone())?)
            })
            .collect()
    }
}

#[test_case::test_case(503; "registration_conflict")]
#[test_case::test_case(502; "authentication_service_unavailable")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn registration_retries_preserve_noise_identity_and_initialized_session(
    retry_status: u16,
) -> Result<()> {
    let fixture = RegistryFixture::new(
        vec![retry_status, 200, retry_status, retry_status, 200],
        Duration::ZERO,
    )
    .await?;
    let (shutdown, remote) = fixture.start()?;
    let harness_identity = NoiseChannelIdentity::generate()?;
    let (client, first_relay) = fixture
        .connect(
            "registration-2",
            &harness_identity,
            /*resume_session_id*/ None,
        )
        .await?;
    let session_id = client.session_id().context("initialized session ID")?;
    let environment_info = client.force_environment_info().await?;
    let key = registered_executor_public_key(&fixture.registry).await?;
    assert_eq!(fixture.registered_keys().await?, vec![key.clone(); 2]);

    first_relay.abort();
    let _ = first_relay.await;
    drop(client);
    let (socket, _) = timeout(TEST_TIMEOUT, fixture.listener.accept()).await??;
    let rejected = http::Response::builder()
        .status(http::StatusCode::UNAUTHORIZED)
        .body(Some("expired registration".to_string()))?;
    let stale_url = accept_hdr_async(socket, |request: &Request, _: Response| {
        assert_eq!(
            request
                .uri()
                .path_and_query()
                .map(http::uri::PathAndQuery::as_str),
            Some("/relay?role=environment&registration=2")
        );
        Err(rejected)
    });
    assert!(timeout(TEST_TIMEOUT, stale_url).await?.is_err());

    let (resumed, second_relay) = fixture
        .connect(
            "registration-5",
            &harness_identity,
            Some(session_id.clone()),
        )
        .await?;
    assert_eq!(resumed.session_id(), Some(session_id));
    assert_eq!(resumed.force_environment_info().await?, environment_info);
    assert_eq!(fixture.registered_keys().await?, vec![key; 5]);

    assert!(shutdown.send(()).is_ok(), "remote task exited");
    timeout(TEST_TIMEOUT, remote).await???;
    drop(resumed);
    second_relay.abort();
    let _ = second_relay.await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_interrupts_in_flight_registration() -> Result<()> {
    let mut fixture = RegistryFixture::new(vec![503], Duration::from_secs(60)).await?;
    let (shutdown, remote) = fixture.start()?;
    assert_eq!(
        timeout(TEST_TIMEOUT, fixture.requests.recv()).await?,
        Some(1)
    );
    assert!(shutdown.send(()).is_ok(), "remote task exited");
    timeout(Duration::from_secs(1), remote)
        .await
        .context("shutdown must not wait for the registration request timeout")???;
    Ok(())
}
