mod common;

#[path = "../src/proto/codex.exec_server.relay.v1.rs"]
mod relay_proto;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_api::AuthProvider;
use codex_exec_server::EnvironmentConnectionState;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecResponse;
use codex_exec_server::ExecServerClient;
use codex_exec_server::ExecServerError;
use codex_exec_server::ExecServerRuntimePaths;
use codex_exec_server::FsReadFileParams;
use codex_exec_server::NoiseChannelIdentity;
use codex_exec_server::NoiseChannelPublicKey;
use codex_exec_server::NoiseRendezvousConnectArgs;
use codex_exec_server::NoiseRendezvousConnectBundle;
use codex_exec_server::NoiseRendezvousConnectProvider;
use codex_exec_server::ProcessId;
use codex_exec_server::RemoteEnvironmentConfig;
use codex_utils_path_uri::PathUri;
use futures::SinkExt;
use futures::StreamExt;
use futures::future::BoxFuture;
use http::HeaderMap;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use prost::Message as ProstMessage;
use relay_proto::RelayMessageFrame;
use relay_proto::relay_message_frame;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const ENVIRONMENT_ID: &str = "env-noise-relay-test";
const EXECUTOR_REGISTRATION_ID: &str = "registration-1";
const HARNESS_KEY_AUTHORIZATION: &str = "harness-key-authorization";
const REGISTRY_TOKEN: &str = "registry-token";
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

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

fn static_registry_auth_provider() -> codex_api::SharedAuthProvider {
    Arc::new(StaticRegistryAuthProvider)
}

struct FreshBundleNoiseConnectProvider {
    websocket_url: String,
    executor_public_key: NoiseChannelPublicKey,
    calls: AtomicUsize,
}

impl FreshBundleNoiseConnectProvider {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }
}

impl NoiseRendezvousConnectProvider for FreshBundleNoiseConnectProvider {
    fn connect_bundle(
        &self,
        _: NoiseChannelPublicKey,
    ) -> BoxFuture<'_, Result<NoiseRendezvousConnectBundle, ExecServerError>> {
        let call = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
        let bundle = NoiseRendezvousConnectBundle {
            websocket_url: self.websocket_url.clone(),
            environment_id: ENVIRONMENT_ID.to_string(),
            executor_registration_id: EXECUTOR_REGISTRATION_ID.to_string(),
            executor_public_key: self.executor_public_key.clone(),
            harness_key_authorization: format!("{HARNESS_KEY_AUTHORIZATION}-{call}"),
        };
        Box::pin(async move { Ok(bundle) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deferred_noise_environment_connects_and_reconnects_with_fresh_bundle() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let rendezvous_url = format!("ws://{}", listener.local_addr()?);
    let registry = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/cloud/environment/{ENVIRONMENT_ID}/register"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "environment_id": ENVIRONMENT_ID,
            "url": format!("{rendezvous_url}/relay?role=environment"),
            "security_profile": "noise_hybrid_ik_v1",
            "executor_registration_id": EXECUTOR_REGISTRATION_ID,
        })))
        .expect(1)
        .mount(&registry)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/cloud/environment/{ENVIRONMENT_ID}/validate"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "valid": true,
        })))
        .expect(2)
        .mount(&registry)
        .await;

    let (codex_exe, codex_linux_sandbox_exe) = common::current_test_binary_helper_paths()?;
    let runtime_paths = ExecServerRuntimePaths::new(codex_exe, codex_linux_sandbox_exe)?;
    let config = RemoteEnvironmentConfig::new(
        registry.uri(),
        ENVIRONMENT_ID.to_string(),
        static_registry_auth_provider(),
    )?;
    let remote_environment = tokio::spawn(codex_exec_server::run_remote_environment(
        config,
        runtime_paths,
    ));

    let environment_websocket = accept_websocket(&listener, "environment").await?;
    let provider = Arc::new(FreshBundleNoiseConnectProvider {
        websocket_url: format!("{rendezvous_url}/relay?role=harness"),
        executor_public_key: registered_executor_public_key(&registry).await?,
        calls: AtomicUsize::new(0),
    });
    let manager = EnvironmentManager::without_environments();
    let registration = manager
        .register_deferred_noise_environment(ENVIRONMENT_ID.to_string(), provider.clone())?;
    let environment = manager
        .get_environment(ENVIRONMENT_ID)
        .context("deferred Noise environment")?;
    let mut connection_state = environment
        .subscribe_connection_state()
        .context("remote environment connection state")?;

    assert_eq!(provider.calls(), 0);
    registration.complete(Ok(()))?;
    let harness_websocket = accept_websocket(&listener, "harness").await?;
    let first_relay = tokio::spawn(proxy_relay_frames(
        environment_websocket,
        harness_websocket,
        Arc::new(Mutex::new(Vec::new())),
    ));
    let initial_info = timeout(TEST_TIMEOUT, environment.info())
        .await
        .context("deferred Noise environment should become ready")??;
    assert_eq!(provider.calls(), 1);
    assert_eq!(
        next_connection_state(&mut connection_state).await?,
        EnvironmentConnectionState::Connected
    );

    first_relay.abort();
    let _ = first_relay.await;
    assert_eq!(
        next_connection_state(&mut connection_state).await?,
        EnvironmentConnectionState::Disconnected
    );
    let first_reconnected_websocket = accept_websocket(&listener, "reconnected peer").await?;
    let second_reconnected_websocket = accept_websocket(&listener, "reconnected peer").await?;
    let second_relay = tokio::spawn(proxy_relay_frames(
        first_reconnected_websocket,
        second_reconnected_websocket,
        Arc::new(Mutex::new(Vec::new())),
    ));
    let recovered_info = timeout(TEST_TIMEOUT, environment.info())
        .await
        .context("deferred Noise environment should reconnect")??;

    assert_eq!(recovered_info, initial_info);
    assert_eq!(provider.calls(), 2);
    assert_eq!(
        next_connection_state(&mut connection_state).await?,
        EnvironmentConnectionState::Connected
    );
    registry.verify().await;

    second_relay.abort();
    remote_environment.abort();
    let _ = second_relay.await;
    let _ = remote_environment.await;
    Ok(())
}

async fn next_connection_state(
    state: &mut watch::Receiver<EnvironmentConnectionState>,
) -> Result<EnvironmentConnectionState> {
    timeout(TEST_TIMEOUT, state.changed()).await??;
    Ok(*state.borrow_and_update())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn remote_environment_routes_encrypted_exec_server_rpc() -> Result<()> {
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

    let (codex_exe, codex_linux_sandbox_exe) = common::current_test_binary_helper_paths()?;
    let runtime_paths = ExecServerRuntimePaths::new(codex_exe, codex_linux_sandbox_exe)?;
    let config = RemoteEnvironmentConfig::new(
        registry.uri(),
        ENVIRONMENT_ID.to_string(),
        static_registry_auth_provider(),
    )?;
    let remote_environment = tokio::spawn(codex_exec_server::run_remote_environment(
        config,
        runtime_paths,
    ));

    let environment_websocket = accept_websocket(&listener, "environment").await?;
    let executor_public_key = registered_executor_public_key(&registry).await?;
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
    };
    let client_task =
        tokio::spawn(async move { ExecServerClient::connect_noise_rendezvous(client_args).await });
    let harness_websocket = accept_websocket(&listener, "harness").await?;
    let captured_frames = Arc::new(Mutex::new(Vec::new()));
    let relay_task = tokio::spawn(proxy_relay_frames(
        environment_websocket,
        harness_websocket,
        Arc::clone(&captured_frames),
    ));
    let client = timeout(TEST_TIMEOUT, client_task)
        .await
        .context("Noise harness client should connect")???;

    let response = client
        .exec(ExecParams {
            process_id: ProcessId::from("proc-1"),
            argv: vec!["true".to_string()],
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            env_policy: None,
            env: HashMap::new(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
        })
        .await?;
    assert_eq!(
        response,
        ExecResponse {
            process_id: ProcessId::from("proc-1"),
        }
    );

    let temp_dir = TempDir::new()?;
    let large_file_path = temp_dir.path().join("large-response.bin");
    let large_file_contents = vec![0x5a; 128 * 1024];
    std::fs::write(&large_file_path, &large_file_contents)?;
    let read_response = client
        .fs_read_file(FsReadFileParams {
            path: PathUri::from_host_native_path(large_file_path)?,
            sandbox: None,
        })
        .await?;
    assert_eq!(
        STANDARD.decode(read_response.data_base64)?,
        large_file_contents
    );

    assert_relay_data_is_encrypted(&captured_frames)?;

    drop(client);
    relay_task.abort();
    remote_environment.abort();
    let _ = relay_task.await;
    let _ = remote_environment.await;
    Ok(())
}

async fn accept_websocket(
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

async fn registered_executor_public_key(registry: &MockServer) -> Result<NoiseChannelPublicKey> {
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

async fn proxy_relay_frames(
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

fn assert_relay_data_is_encrypted(captured_frames: &Mutex<Vec<Vec<u8>>>) -> Result<()> {
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
