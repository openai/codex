#[path = "../../src/proto/codex.exec_server.relay.v1.rs"]
mod relay_proto;

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_api::AuthProvider;
use codex_exec_server::ExecServerClient;
use codex_exec_server::NoiseChannelIdentity;
use codex_exec_server::NoiseChannelPublicKey;
use codex_exec_server::NoiseRendezvousConnectArgs;
use codex_exec_server::NoiseRendezvousConnectBundle;
use codex_exec_server::RemoteEnvironmentConfig;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderMap;
use http::HeaderValue;
use prost::Message as ProstMessage;
use relay_proto::RelayMessageFrame;
use relay_proto::relay_message_frame;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::task::AbortOnDropHandle;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

pub(crate) const ENVIRONMENT_ID: &str = "env-noise-relay-test";
pub(crate) const EXECUTOR_REGISTRATION_ID: &str = "registration-1";
pub(crate) const HARNESS_KEY_AUTHORIZATION: &str = "harness-key-authorization";
pub(crate) const REGISTRY_TOKEN: &str = "registry-token";
pub(crate) const TEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
struct StaticRegistryAuthProvider;

impl AuthProvider for StaticRegistryAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        let _ = headers.insert(
            http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer registry-token"),
        );
    }
}

pub(crate) fn static_registry_auth_provider() -> codex_api::SharedAuthProvider {
    Arc::new(StaticRegistryAuthProvider)
}

pub(crate) struct RelayTest {
    registry: MockServer,
    listener: TcpListener,
}

pub(crate) struct RelayConnection {
    pub(crate) client: ExecServerClient,
    captured_frames: Arc<Mutex<Vec<Vec<u8>>>>,
    relay_task: AbortOnDropHandle<Result<()>>,
}

impl RelayTest {
    pub(crate) async fn new() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let rendezvous_url = format!("ws://{}", listener.local_addr()?);
        let registry = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(format!(
                "/cloud/environment/{ENVIRONMENT_ID}/register"
            )))
            .and(header("authorization", format!("Bearer {REGISTRY_TOKEN}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "environment_id": ENVIRONMENT_ID,
                "url": format!("{rendezvous_url}/relay?role=environment"),
                "security_profile": "noise_hybrid_ik_v1",
                "executor_registration_id": EXECUTOR_REGISTRATION_ID,
            })))
            .mount(&registry)
            .await;
        Mock::given(method("POST"))
            .and(path(format!(
                "/cloud/environment/{ENVIRONMENT_ID}/validate"
            )))
            .and(header("authorization", format!("Bearer {REGISTRY_TOKEN}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "valid": true,
            })))
            .mount(&registry)
            .await;
        Ok(Self { registry, listener })
    }

    pub(crate) fn config(&self) -> Result<RemoteEnvironmentConfig> {
        Ok(RemoteEnvironmentConfig::new(
            self.registry.uri(),
            ENVIRONMENT_ID.to_string(),
            static_registry_auth_provider(),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )?)
    }

    pub(crate) async fn connect(&self) -> Result<RelayConnection> {
        let rendezvous_url = format!("ws://{}", self.listener.local_addr()?);
        let environment_websocket = accept_websocket(&self.listener, "environment").await?;
        let executor_public_key = registered_executor_public_key(&self.registry).await?;
        let harness_identity = NoiseChannelIdentity::generate()?;
        let client_args = NoiseRendezvousConnectArgs {
            bundle: NoiseRendezvousConnectBundle {
                websocket_url: format!("{rendezvous_url}/relay?role=harness"),
                environment_id: ENVIRONMENT_ID.to_string(),
                executor_registration_id: EXECUTOR_REGISTRATION_ID.to_string(),
                executor_public_key,
                harness_key_authorization: HARNESS_KEY_AUTHORIZATION.to_string(),
            },
            harness_identity,
            client_name: "noise-relay-test".to_string(),
            connect_timeout: TEST_TIMEOUT,
            initialize_timeout: TEST_TIMEOUT,
            resume_session_id: None,
            http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        };
        let client_task =
            tokio::spawn(
                async move { ExecServerClient::connect_noise_rendezvous(client_args).await },
            );
        let harness_websocket = accept_websocket(&self.listener, "harness").await?;
        let captured_frames = Arc::new(Mutex::new(Vec::new()));
        let relay_task = AbortOnDropHandle::new(tokio::spawn(proxy_relay_frames(
            environment_websocket,
            harness_websocket,
            Arc::clone(&captured_frames),
        )));
        let client = timeout(TEST_TIMEOUT, client_task)
            .await
            .context("Noise harness client should connect")???;
        Ok(RelayConnection {
            client,
            captured_frames,
            relay_task,
        })
    }
}

impl RelayConnection {
    pub(crate) fn assert_encrypted(&self) -> Result<()> {
        assert_relay_data_is_encrypted(&self.captured_frames)
    }

    pub(crate) async fn close(self) {
        drop(self.client);
        self.relay_task.abort();
        let _ = self.relay_task.await;
    }
}

pub(crate) async fn accept_websocket(
    listener: &TcpListener,
    role: &str,
) -> Result<WebSocketStream<TcpStream>> {
    let (socket, _peer_addr) = timeout(TEST_TIMEOUT, listener.accept())
        .await
        .with_context(|| format!("remote {role} should connect to fake rendezvous"))??;
    timeout(TEST_TIMEOUT, accept_async(socket))
        .await
        .with_context(|| format!("fake rendezvous should accept {role} websocket"))?
        .map_err(Into::into)
}

pub(crate) async fn registered_executor_public_key(
    registry: &MockServer,
) -> Result<NoiseChannelPublicKey> {
    let requests = registry
        .received_requests()
        .await
        .context("wiremock should retain requests")?;
    let request = requests
        .iter()
        .find(|request| request.url.path().ends_with("/register"))
        .context("exec-server should register before connecting")?;
    let body: serde_json::Value = serde_json::from_slice(&request.body)?;
    let key = serde_json::from_value(body["executor_public_key"].clone())?;
    Ok(key)
}

pub(crate) async fn proxy_relay_frames(
    mut environment: WebSocketStream<TcpStream>,
    mut harness: WebSocketStream<TcpStream>,
    captured_frames: Arc<Mutex<Vec<Vec<u8>>>>,
) -> Result<()> {
    loop {
        tokio::select! {
            message = environment.next() => {
                let Some(message) = message else {
                    break;
                };
                let message = message?;
                capture_binary_frame(&captured_frames, &message);
                harness.send(message).await?;
            }
            message = harness.next() => {
                let Some(message) = message else {
                    break;
                };
                let message = message?;
                capture_binary_frame(&captured_frames, &message);
                environment.send(message).await?;
            }
        }
    }
    Ok(())
}

fn capture_binary_frame(captured_frames: &Mutex<Vec<Vec<u8>>>, message: &Message) {
    if let Message::Binary(bytes) = message {
        captured_frames
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(bytes.to_vec());
    }
}

pub(crate) fn assert_relay_data_is_encrypted(captured_frames: &Mutex<Vec<Vec<u8>>>) -> Result<()> {
    let captured_frames = captured_frames
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut data_frames = 0;
    for encoded in captured_frames.iter() {
        let frame = RelayMessageFrame::decode(encoded.as_slice())?;
        let Some(relay_message_frame::Body::Data(data)) = frame.body else {
            continue;
        };
        data_frames += 1;
        let payload = String::from_utf8_lossy(&data.payload);
        assert!(!payload.contains("initialize"));
        assert!(!payload.contains("process/start"));
        assert!(!payload.contains("noise-relay-test"));
    }
    assert!(
        data_frames >= 4,
        "expected encrypted request and response frames"
    );
    Ok(())
}
