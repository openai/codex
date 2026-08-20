mod common;

#[cfg(unix)]
#[path = "relay/version_skew.rs"]
mod version_skew;

#[path = "common/relay.rs"]
mod relay_support;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_exec_server::EnvironmentConnectionState;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::EnvironmentReadyInfo;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecResponse;
use codex_exec_server::ExecServerError;
use codex_exec_server::ExecServerRuntimePaths;
use codex_exec_server::FsReadFileParams;
use codex_exec_server::NoiseChannelPublicKey;
use codex_exec_server::NoiseRendezvousConnectBundle;
use codex_exec_server::NoiseRendezvousConnectProvider;
use codex_exec_server::ProcessId;
use codex_exec_server::RemoteEnvironmentConfig;
use codex_exec_server_protocol::ProcessSandboxType;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::cache_system_proxy_route_for_test;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use relay_support::ENVIRONMENT_ID;
use relay_support::EXECUTOR_REGISTRATION_ID;
use relay_support::HARNESS_KEY_AUTHORIZATION;
use relay_support::RelayTest;
use relay_support::TEST_TIMEOUT;
use relay_support::accept_websocket;
use relay_support::proxy_relay_frames;
use relay_support::registered_executor_public_key;
use relay_support::static_registry_auth_provider;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_util::task::AbortOnDropHandle;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

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
#[serial_test::serial]
async fn pending_noise_environment_connects_and_reconnects_after_ready_report() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let rendezvous_address = listener.local_addr()?;
    let environment_rendezvous_url =
        "ws://environment-noise-relay-system-proxy.invalid:8765/relay?role=environment";
    let harness_rendezvous_url =
        "ws://harness-noise-relay-system-proxy.invalid:8765/relay?role=harness";
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await?;
    let proxy_url = format!("http://{}", proxy_listener.local_addr()?);
    for rendezvous_url in [environment_rendezvous_url, harness_rendezvous_url] {
        let proxy_resolution_url = rendezvous_url.replacen("ws://", "http://", /*count*/ 1);
        cache_system_proxy_route_for_test(&proxy_resolution_url, proxy_url.clone());
    }
    let (proxy_request_tx, mut proxy_request_rx) = mpsc::unbounded_channel();
    let _proxy_task = AbortOnDropHandle::new(tokio::spawn(async move {
        let mut proxy_connections = JoinSet::new();
        while let Ok((mut client, _)) = proxy_listener.accept().await {
            let proxy_request_tx = proxy_request_tx.clone();
            proxy_connections.spawn(async move {
                let mut request = Vec::new();
                let mut byte = [0_u8; 1];
                while !request.ends_with(b"\r\n\r\n") {
                    client.read_exact(&mut byte).await?;
                    request.push(byte[0]);
                }
                let request_line = String::from_utf8(request)?
                    .lines()
                    .next()
                    .context("system proxy should receive a CONNECT request")?
                    .to_string();
                proxy_request_tx
                    .send(request_line)
                    .map_err(|_| anyhow::anyhow!("system proxy request receiver was dropped"))?;
                let mut target = TcpStream::connect(rendezvous_address).await?;
                client
                    .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                    .await?;
                tokio::io::copy_bidirectional(&mut client, &mut target).await?;
                Ok::<(), anyhow::Error>(())
            });
        }
    }));
    let registry = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/cloud/environment/{ENVIRONMENT_ID}/register"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "environment_id": ENVIRONMENT_ID,
            "url": environment_rendezvous_url,
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
    let http_client_factory = HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy);
    let config = RemoteEnvironmentConfig::new(
        registry.uri(),
        ENVIRONMENT_ID.to_string(),
        static_registry_auth_provider(),
        http_client_factory.clone(),
    )?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let remote_environment = AbortOnDropHandle::new(tokio::spawn(
        codex_exec_server::run_remote_environment_until_shutdown(
            config,
            runtime_paths,
            async move {
                let _ = shutdown_rx.await;
            },
        ),
    ));

    let environment_websocket = accept_websocket(&listener, "environment").await?;
    let environment_proxy_request =
        "CONNECT environment-noise-relay-system-proxy.invalid:8765 HTTP/1.1";
    let harness_proxy_request = "CONNECT harness-noise-relay-system-proxy.invalid:8765 HTTP/1.1";
    assert_eq!(
        timeout(TEST_TIMEOUT, proxy_request_rx.recv()).await?,
        Some(environment_proxy_request.to_string())
    );
    let provider = Arc::new(FreshBundleNoiseConnectProvider {
        websocket_url: harness_rendezvous_url.to_string(),
        executor_public_key: registered_executor_public_key(&registry).await?,
        calls: AtomicUsize::new(0),
    });
    let manager = EnvironmentManager::without_environments(http_client_factory);
    let environment = manager
        .materialize_pending_noise_environment(ENVIRONMENT_ID.to_string(), provider.clone())?;
    let mut connection_state = environment
        .subscribe_connection_state()
        .context("remote environment connection state")?;

    assert_eq!(provider.calls(), 0);
    let selected_capability_roots = vec![SelectedCapabilityRoot {
        id: "executor-plugin".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: ENVIRONMENT_ID.to_string(),
            path: PathUri::parse("file:///plugins/executor-plugin")?,
        },
    }];
    let reported = manager
        .report_environment_provisioning_status(
            ENVIRONMENT_ID.to_string(),
            Ok(EnvironmentReadyInfo {
                selected_capability_roots: selected_capability_roots.clone(),
            }),
            provider.clone(),
        )?
        .context("ready report should apply to the pending environment")?;
    assert!(Arc::ptr_eq(&environment, &reported));
    assert_eq!(provider.calls(), 0);
    let initial_info = tokio::spawn({
        let environment = Arc::clone(&environment);
        async move { environment.info().await }
    });
    let harness_websocket = accept_websocket(&listener, "harness").await?;
    assert_eq!(
        timeout(TEST_TIMEOUT, proxy_request_rx.recv()).await?,
        Some(harness_proxy_request.to_string())
    );
    let first_relay = tokio::spawn(proxy_relay_frames(
        environment_websocket,
        harness_websocket,
        Arc::new(Mutex::new(Vec::new())),
    ));
    let initial_info = timeout(TEST_TIMEOUT, initial_info)
        .await
        .context("pending Noise environment should become ready")???;
    assert_eq!(
        environment.selected_capability_roots(),
        selected_capability_roots
    );
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
    let mut reconnect_proxy_requests = vec![
        timeout(TEST_TIMEOUT, proxy_request_rx.recv())
            .await?
            .context("first reconnected peer should use the system proxy")?,
        timeout(TEST_TIMEOUT, proxy_request_rx.recv())
            .await?
            .context("second reconnected peer should use the system proxy")?,
    ];
    reconnect_proxy_requests.sort();
    assert_eq!(
        reconnect_proxy_requests,
        vec![
            environment_proxy_request.to_string(),
            harness_proxy_request.to_string(),
        ]
    );
    let second_relay = tokio::spawn(proxy_relay_frames(
        first_reconnected_websocket,
        second_reconnected_websocket,
        Arc::new(Mutex::new(Vec::new())),
    ));
    let recovered_info = timeout(TEST_TIMEOUT, environment.info())
        .await
        .context("pending Noise environment should reconnect")??;

    assert_eq!(recovered_info, initial_info);
    assert_eq!(
        environment.selected_capability_roots(),
        selected_capability_roots
    );
    assert_eq!(provider.calls(), 2);
    assert_eq!(
        next_connection_state(&mut connection_state).await?,
        EnvironmentConnectionState::Connected
    );
    registry.verify().await;

    second_relay.abort();
    let _ = second_relay.await;
    let _ = shutdown_tx.send(());
    timeout(TEST_TIMEOUT, remote_environment).await???;
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
    let relay = RelayTest::new().await?;
    let (codex_exe, codex_linux_sandbox_exe) = common::current_test_binary_helper_paths()?;
    let runtime_paths = ExecServerRuntimePaths::new(codex_exe, codex_linux_sandbox_exe)?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let remote_environment = AbortOnDropHandle::new(tokio::spawn(
        codex_exec_server::run_remote_environment_until_shutdown(
            relay.config()?,
            runtime_paths,
            async move {
                let _ = shutdown_rx.await;
            },
        ),
    ));
    let connection = relay.connect().await?;
    let client = &connection.client;

    let exec_params = ExecParams {
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
        network_proxy: None,
    };
    let response = client.exec(exec_params).await?;
    assert_eq!(
        response,
        ExecResponse {
            process_id: ProcessId::from("proc-1"),
            sandbox_type: Some(ProcessSandboxType::None),
        }
    );

    let temp_dir = TempDir::new()?;
    let large_file_path = temp_dir.path().join("large-response.bin");
    let large_file_contents = vec![0x5a; 128 * 1024];
    std::fs::write(&large_file_path, &large_file_contents)?;
    let read_response = client
        .fs_read_file(FsReadFileParams {
            path: PathUri::from_host_native_path(large_file_path)?,
            follow_symlinks: None,
            sandbox: None,
        })
        .await?;
    assert_eq!(
        STANDARD.decode(read_response.data_base64)?,
        large_file_contents
    );
    connection.assert_encrypted()?;
    connection.close().await;
    let _ = shutdown_tx.send(());
    timeout(TEST_TIMEOUT, remote_environment).await???;
    Ok(())
}
