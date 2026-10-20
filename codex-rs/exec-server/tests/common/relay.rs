use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Context;
use anyhow::Result;
use codex_api::AuthProvider;
use codex_exec_server::ExecServerClient;
use codex_exec_server::NoiseChannelIdentity;
use codex_exec_server::NoiseRendezvousConnectArgs;
use codex_exec_server::NoiseRendezvousConnectBundle;
use codex_exec_server::RemoteEnvironmentConfig;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use http::HeaderMap;
use http::HeaderValue;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_util::task::AbortOnDropHandle;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

pub(crate) use codex_exec_server_test_support::relay::TEST_TIMEOUT;
pub(crate) use codex_exec_server_test_support::relay::accept_websocket;
pub(crate) use codex_exec_server_test_support::relay::assert_relay_data_is_encrypted;
pub(crate) use codex_exec_server_test_support::relay::proxy_relay_frames;
pub(crate) use codex_exec_server_test_support::relay::registered_executor_public_key;

pub(crate) const ENVIRONMENT_ID: &str = "env-noise-relay-test";
pub(crate) const EXECUTOR_REGISTRATION_ID: &str = "registration-1";
pub(crate) const HARNESS_KEY_AUTHORIZATION: &str = "harness-key-authorization";
pub(crate) const REGISTRY_TOKEN: &str = "registry-token";

#[derive(Debug)]
struct StaticRegistryAuthProvider;

impl AuthProvider for StaticRegistryAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), codex_api::AuthError> {
        let _ = headers.insert(
            http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer registry-token"),
        );
        Ok(())
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
