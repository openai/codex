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
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecResponse;
use codex_exec_server::ExecServerClient;
use codex_exec_server::ExecServerError;
use codex_exec_server::ExecServerRuntimePaths;
use codex_exec_server::FsReadFileParams;
use codex_exec_server::ProcessId;
use codex_exec_server::RemoteEnvironmentConfig;
use codex_exec_server::RemoteRelaySecurity;
use codex_exec_server::SecureChannelIdentity;
use codex_exec_server::SecureChannelPublicKey;
use codex_exec_server::SecureRendezvousConnectArgs;
use codex_exec_server::SecureRendezvousConnectBundle;
use codex_exec_server::SecureRendezvousConnectProvider;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::FutureExt;
use futures::SinkExt;
use futures::StreamExt;
use futures::future::BoxFuture;
use http::HeaderMap;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use prost::Message as ProstMessage;
use relay_proto::RelayData;
use relay_proto::RelayMessageFrame;
use relay_proto::relay_message_frame;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
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

const ENVIRONMENT_ID: &str = "env-secure-relay-test";
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

struct FailingSecureConnectProvider {
    attempts: Arc<AtomicUsize>,
}

impl SecureRendezvousConnectProvider for FailingSecureConnectProvider {
    fn environment_id(&self) -> &str {
        ENVIRONMENT_ID
    }

    fn connect_args(&self) -> BoxFuture<'_, Result<SecureRendezvousConnectArgs, ExecServerError>> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        async {
            Err(ExecServerError::Protocol(
                "test registry connect failure".to_string(),
            ))
        }
        .boxed()
    }
}

struct WrongEnvironmentSecureConnectProvider {
    harness_identity: SecureChannelIdentity,
    executor_public_key: SecureChannelPublicKey,
}

impl SecureRendezvousConnectProvider for WrongEnvironmentSecureConnectProvider {
    fn environment_id(&self) -> &str {
        ENVIRONMENT_ID
    }

    fn connect_args(&self) -> BoxFuture<'_, Result<SecureRendezvousConnectArgs, ExecServerError>> {
        async move {
            Ok(SecureRendezvousConnectArgs::new(
                SecureRendezvousConnectBundle {
                    websocket_url: "ws://127.0.0.1:1".to_string(),
                    environment_id: "wrong-environment".to_string(),
                    executor_registration_id: EXECUTOR_REGISTRATION_ID.to_string(),
                    executor_public_key: self.executor_public_key.clone(),
                    harness_key_authorization: HARNESS_KEY_AUTHORIZATION.to_string(),
                },
                self.harness_identity.clone(),
                "secure-relay-test".to_string(),
            ))
        }
        .boxed()
    }
}

fn static_registry_auth_provider() -> codex_api::SharedAuthProvider {
    Arc::new(StaticRegistryAuthProvider)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_environment_uses_legacy_relay_by_default() -> Result<()> {
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
            "url": rendezvous_url,
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
    assert_eq!(config.relay_security, RemoteRelaySecurity::Legacy);
    let remote_environment = tokio::spawn(codex_exec_server::run_remote_environment(
        config,
        runtime_paths,
    ));

    let mut websocket = accept_websocket(&listener, "legacy environment").await?;
    let initialize = serde_json::json!({
        "id": 1,
        "method": "initialize",
        "params": {
            "clientName": "legacy-default-test",
            "resumeSessionId": null,
        },
    });
    websocket
        .send(Message::Binary(
            RelayMessageFrame {
                version: 1,
                stream_id: "legacy-stream".to_string(),
                ack: 0,
                ack_bits: 0,
                body: Some(relay_message_frame::Body::Data(RelayData {
                    seq: 0,
                    segment_index: 0,
                    segment_count: 1,
                    payload: serde_json::to_vec(&initialize)?,
                })),
            }
            .encode_to_vec()
            .into(),
        ))
        .await?;

    let response = timeout(TEST_TIMEOUT, websocket.next())
        .await
        .context("legacy relay should return initialize response")?
        .context("legacy relay websocket should remain open")??;
    let Message::Binary(response) = response else {
        anyhow::bail!("legacy relay should return a binary protobuf frame");
    };
    let response = RelayMessageFrame::decode(response.as_ref())?;
    let Some(relay_message_frame::Body::Data(response)) = response.body else {
        anyhow::bail!("legacy relay should return a data frame");
    };
    let response: serde_json::Value = serde_json::from_slice(&response.payload)?;
    assert_eq!(response["id"], 1);
    assert!(response["result"].is_object());

    remote_environment.abort();
    let _ = remote_environment.await;
    Ok(())
}

#[tokio::test]
async fn secure_environment_refreshes_bundle_for_each_connection_attempt() -> Result<()> {
    let attempts = Arc::new(AtomicUsize::new(0));
    let manager = EnvironmentManager::without_environments();
    manager.upsert_secure_environment(
        ENVIRONMENT_ID.to_string(),
        Arc::new(FailingSecureConnectProvider {
            attempts: Arc::clone(&attempts),
        }),
    )?;
    let backend = manager
        .get_environment(ENVIRONMENT_ID)
        .context("secure environment should be materialized")?
        .get_exec_backend();

    for attempt in 1..=2 {
        let result = backend
            .start(ExecParams {
                process_id: ProcessId::new(format!("proc-{attempt}")),
                argv: vec!["true".to_string()],
                cwd: std::env::current_dir()?,
                env_policy: None,
                env: HashMap::new(),
                tty: false,
                pipe_stdin: false,
                arg0: None,
            })
            .await;
        assert!(matches!(
            result,
            Err(ExecServerError::Protocol(ref message))
                if message == "test registry connect failure"
        ));
    }

    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn secure_environment_rejects_provider_bundle_for_another_environment() -> Result<()> {
    let manager = EnvironmentManager::without_environments();
    manager.upsert_secure_environment(
        ENVIRONMENT_ID.to_string(),
        Arc::new(WrongEnvironmentSecureConnectProvider {
            harness_identity: SecureChannelIdentity::generate()?,
            executor_public_key: SecureChannelIdentity::generate()?.public_key(),
        }),
    )?;
    let backend = manager
        .get_environment(ENVIRONMENT_ID)
        .context("secure environment should be materialized")?
        .get_exec_backend();

    let result = backend
        .start(ExecParams {
            process_id: ProcessId::from("proc-wrong-environment"),
            argv: vec!["true".to_string()],
            cwd: std::env::current_dir()?,
            env_policy: None,
            env: HashMap::new(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
        })
        .await;

    assert!(matches!(
        result,
        Err(ExecServerError::Protocol(ref message))
            if message == "secure rendezvous provider returned a different environment id"
    ));
    Ok(())
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
            "/cloud/environment/{ENVIRONMENT_ID}/validate-harness-key"
        )))
        .and(header("authorization", format!("Bearer {REGISTRY_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&registry)
        .await;

    let (codex_exe, codex_linux_sandbox_exe) = common::current_test_binary_helper_paths()?;
    let runtime_paths = ExecServerRuntimePaths::new(codex_exe, codex_linux_sandbox_exe)?;
    let mut config = RemoteEnvironmentConfig::new(
        registry.uri(),
        ENVIRONMENT_ID.to_string(),
        static_registry_auth_provider(),
    )?;
    config.relay_security = RemoteRelaySecurity::Noise;
    let remote_environment = tokio::spawn(codex_exec_server::run_remote_environment(
        config,
        runtime_paths,
    ));

    let environment_websocket = accept_websocket(&listener, "environment").await?;
    let executor_public_key = registered_executor_public_key(&registry).await?;
    let harness_identity = SecureChannelIdentity::generate()?;
    let client_args = SecureRendezvousConnectArgs::new(
        SecureRendezvousConnectBundle {
            websocket_url: format!("{rendezvous_url}/relay?role=harness"),
            environment_id: ENVIRONMENT_ID.to_string(),
            executor_registration_id: EXECUTOR_REGISTRATION_ID.to_string(),
            executor_public_key,
            harness_key_authorization: HARNESS_KEY_AUTHORIZATION.to_string(),
        },
        harness_identity,
        "secure-relay-test".to_string(),
    );
    let client_task =
        tokio::spawn(async move { ExecServerClient::connect_secure_rendezvous(client_args).await });
    let harness_websocket = accept_websocket(&listener, "harness").await?;
    let captured_frames = Arc::new(Mutex::new(Vec::new()));
    let relay_task = tokio::spawn(proxy_relay_frames(
        environment_websocket,
        harness_websocket,
        Arc::clone(&captured_frames),
    ));
    let client = timeout(TEST_TIMEOUT, client_task)
        .await
        .context("secure harness client should connect")???;

    let response = client
        .exec(ExecParams {
            process_id: ProcessId::from("proc-1"),
            argv: vec!["true".to_string()],
            cwd: std::env::current_dir()?,
            env_policy: None,
            env: HashMap::new(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
        })
        .await?;
    assert_eq!(
        response,
        ExecResponse {
            process_id: ProcessId::from("proc-1")
        }
    );

    let temp_dir = TempDir::new()?;
    let large_file_path = temp_dir.path().join("large-response.bin");
    let large_file_contents = vec![0x5a; 128 * 1024];
    std::fs::write(&large_file_path, &large_file_contents)?;
    let read_response = client
        .fs_read_file(FsReadFileParams {
            path: AbsolutePathBuf::try_from(large_file_path)?,
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

async fn registered_executor_public_key(registry: &MockServer) -> Result<SecureChannelPublicKey> {
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
        assert!(!payload.contains("secure-relay-test"));
    }
    assert!(
        data_frames >= 4,
        "expected encrypted request and response frames"
    );
    Ok(())
}
