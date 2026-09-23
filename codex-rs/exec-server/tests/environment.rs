mod common;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorCapabilityDiscoveryCache;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_exec_server::SelectedCapabilityRootsStatus;
use codex_exec_server_protocol::CAPABILITY_ROOTS_DISCOVER_METHOD;
use codex_exec_server_protocol::CapabilityRootDiscoverRequest;
use codex_exec_server_protocol::CapabilityRootsDiscoverParams;
use codex_file_system::FileSystemSandboxContext;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::cache_system_proxy_route_for_test;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_path_uri::PathUri;
use common::exec_server::exec_server;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::task::AbortOnDropHandle;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(remote_exec_server)]
async fn prepared_remote_environment_uses_configured_system_proxy() -> anyhow::Result<()> {
    let server = exec_server().await?;
    let upstream = server
        .websocket_url()
        .strip_prefix("ws://")
        .context("exec-server websocket should use ws://")?
        .to_string();
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await?;
    let proxy_url = format!("http://{}", proxy_listener.local_addr()?);
    let websocket_url = "ws://exec-server-system-proxy.invalid:8765/";
    let proxy_resolution_url = "http://exec-server-system-proxy.invalid:8765/";
    cache_system_proxy_route_for_test(proxy_resolution_url, proxy_url);

    let (request_tx, request_rx) = oneshot::channel();
    let _proxy_task = AbortOnDropHandle::new(tokio::spawn(async move {
        let (mut client, _) = proxy_listener.accept().await?;
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
        request_tx
            .send(request_line)
            .map_err(|_| anyhow::anyhow!("system proxy request receiver was dropped"))?;

        let mut target = TcpStream::connect(upstream).await?;
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        tokio::io::copy_bidirectional(&mut client, &mut target).await?;
        Ok::<(), anyhow::Error>(())
    }));

    let codex_home = tempfile::tempdir()?;
    std::fs::write(
        codex_home.path().join("environments.toml"),
        format!(
            "default = \"{REMOTE_ENVIRONMENT_ID}\"\ninclude_local = false\n\n[[environments]]\nid = \"{REMOTE_ENVIRONMENT_ID}\"\nurl = \"{websocket_url}\"\n"
        ),
    )?;

    let prepared = EnvironmentManager::prepare_from_codex_home(codex_home.path()).await?;
    assert!(prepared.default_environment_is_remote());
    let manager = prepared.build(
        /*local_runtime_paths*/ None,
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
    )?;

    let request_line = timeout(Duration::from_secs(5), request_rx)
        .await
        .context("prepared environment did not connect through the system proxy")??;
    assert_eq!(
        request_line,
        "CONNECT exec-server-system-proxy.invalid:8765 HTTP/1.1"
    );
    let environment = manager
        .default_environment()
        .context("prepared remote environment")?;
    timeout(Duration::from_secs(5), environment.info())
        .await
        .context("prepared remote environment did not initialize through the system proxy")??;

    Ok(())
}

/// Older discovery executors receive direct-read contexts unchanged but never receive restricted reads.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capability_discovery_without_sandbox_support_only_sends_full_disk_reads()
-> anyhow::Result<()> {
    let server = exec_server().await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let proxy_url = format!("ws://{}", listener.local_addr()?);
    let upstream_url = server.websocket_url().to_string();
    let (requests_tx, mut requests_rx) = tokio::sync::mpsc::unbounded_channel();
    let _proxy = AbortOnDropHandle::new(tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut downstream = accept_async(stream).await?;
        let (mut upstream, _) = connect_async(&upstream_url).await?;
        loop {
            tokio::select! {
                message = downstream.next() => {
                    let Some(message) = message.transpose()? else { break };
                    if let Message::Text(text) = &message {
                        let request: serde_json::Value = serde_json::from_str(text.as_ref())?;
                        if request.get("method").and_then(serde_json::Value::as_str)
                            == Some(CAPABILITY_ROOTS_DISCOVER_METHOD)
                        {
                            requests_tx.send(request)?;
                        }
                    }
                    upstream.send(message).await?;
                }
                message = upstream.next() => {
                    let Some(mut message) = message.transpose()? else { break };
                    if let Message::Text(text) = &message {
                        let mut response: serde_json::Value = serde_json::from_str(text.as_ref())?;
                        for pointer in [
                            "/result/environmentInfo/capabilities/capabilityDiscoverySandbox",
                            "/result/capabilities/capabilityDiscoverySandbox",
                        ] {
                            if let Some(capability) = response.pointer_mut(pointer) {
                                *capability = false.into();
                            }
                        }
                        message = Message::Text(response.to_string().into());
                    }
                    downstream.send(message).await?;
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    }));
    let manager =
        EnvironmentManager::create_for_tests(Some(proxy_url), /*local_runtime_paths*/ None).await;
    let environment = manager
        .default_environment()
        .context("remote environment")?;
    assert!(
        !environment
            .info()
            .await?
            .capabilities
            .capability_discovery_sandbox
    );

    let workspace = tempfile::tempdir()?;
    let cwd = PathUri::from_host_native_path(workspace.path())?;
    let full_read = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::read_only(),
        cwd.clone(),
    );
    let request = CapabilityRootDiscoverRequest {
        id: "root".to_string(),
        path: cwd.clone(),
        sandbox: Some(full_read),
    };
    let unrestricted = CapabilityRootDiscoverRequest {
        id: "unrestricted".to_string(),
        path: cwd.clone(),
        sandbox: Some(FileSystemSandboxContext::from_permission_profile(
            PermissionProfile::Disabled,
            cwd.clone(),
        )),
    };
    let response = environment
        .discover_capability_roots(CapabilityRootsDiscoverParams {
            roots: vec![request.clone(), unrestricted.clone()],
        })
        .await?;
    assert_eq!(
        response
            .roots
            .iter()
            .map(|root| (root.id.as_str(), &root.error))
            .collect::<Vec<_>>(),
        vec![("root", &None), ("unrestricted", &None)]
    );
    let sent = timeout(Duration::from_secs(5), requests_rx.recv())
        .await?
        .context("discovery request")?;
    let sent: CapabilityRootsDiscoverParams = serde_json::from_value(sent["params"].clone())?;
    assert_eq!(sent.roots, vec![request.clone(), unrestricted]);

    let restricted = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
                cwd.clone().into(),
                FileSystemAccessMode::Read,
            )]),
            NetworkSandboxPolicy::Restricted,
        ),
        cwd,
    );
    let error = environment
        .discover_capability_roots(CapabilityRootsDiscoverParams {
            roots: vec![
                request.clone(),
                CapabilityRootDiscoverRequest {
                    sandbox: Some(restricted),
                    ..request
                },
            ],
        })
        .await
        .expect_err("restricted reads must not fall back to old discovery");
    assert!(
        error
            .to_string()
            .contains("exec-server does not support sandboxed capability discovery")
    );
    assert_eq!(
        requests_rx.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(remote_exec_server)]
async fn selected_capability_inspection_tracks_connection_recovery() -> anyhow::Result<()> {
    let server = exec_server().await?;
    let mut proxy = server.disconnectable_websocket_proxy().await?;
    let manager = EnvironmentManager::create_for_tests(
        Some(proxy.websocket_url().to_string()),
        /*local_runtime_paths*/ None,
    )
    .await;
    let environment = manager
        .default_environment()
        .context("remote environment")?;
    environment.info().await?;

    let skill_root_path = PathUri::parse("file:///plugins/demo")?;
    let selected_root = SelectedCapabilityRoot {
        id: "demo@1".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            path: skill_root_path.clone(),
        },
    };
    assert_eq!(
        manager.inspect_selected_capability_roots(std::slice::from_ref(&selected_root)),
        SelectedCapabilityRootsStatus {
            ready_roots: vec![selected_root.clone()],
            warnings: Vec::new(),
        }
    );
    let file_system = environment.get_filesystem_without_reconnect();

    proxy.pause_and_disconnect().await?;
    assert_eq!(
        manager.inspect_selected_capability_roots(std::slice::from_ref(&selected_root)),
        SelectedCapabilityRootsStatus::default()
    );
    let read_result = timeout(
        Duration::from_secs(1),
        file_system.read_directory(&skill_root_path, /*sandbox*/ None),
    )
    .await
    .context("passive filesystem read waited for recovery")?;
    assert!(read_result.is_err());

    proxy.resume()?;
    let recovered_status = timeout(Duration::from_secs(5), async {
        loop {
            let status =
                manager.inspect_selected_capability_roots(std::slice::from_ref(&selected_root));
            if !status.ready_roots.is_empty() {
                break status;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("environment did not recover")?;
    assert_eq!(
        recovered_status,
        SelectedCapabilityRootsStatus {
            ready_roots: vec![selected_root],
            warnings: Vec::new(),
        }
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(remote_exec_server)]
async fn capability_discovery_retries_executor_disconnect_within_same_request() -> anyhow::Result<()>
{
    let server = exec_server().await?;
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await?;
    let proxy_websocket_url = format!("ws://{}", proxy_listener.local_addr()?);
    let upstream_websocket_url = server.websocket_url().to_string();
    let discovery_attempts = Arc::new(AtomicUsize::new(0));
    let proxy_discovery_attempts = Arc::clone(&discovery_attempts);
    let _proxy_task = AbortOnDropHandle::new(tokio::spawn(async move {
        while let Ok((downstream, _)) = proxy_listener.accept().await {
            let mut downstream = accept_async(downstream).await?;
            let (mut upstream, _) = connect_async(&upstream_websocket_url).await?;

            loop {
                tokio::select! {
                    message = downstream.next() => {
                        let Some(message) = message.transpose()? else {
                            break;
                        };
                        if let Message::Text(message_text) = &message {
                            let request = serde_json::from_str::<serde_json::Value>(message_text.as_ref())?;
                            if request.get("method").and_then(serde_json::Value::as_str)
                                == Some(CAPABILITY_ROOTS_DISCOVER_METHOD)
                            {
                                let attempt = proxy_discovery_attempts.fetch_add(1, Ordering::SeqCst);
                                if attempt == 0 {
                                    break;
                                }
                                sleep(Duration::from_secs(9)).await;
                            }
                        }
                        upstream.send(message).await?;
                    }
                    message = upstream.next() => {
                        let Some(message) = message.transpose()? else {
                            break;
                        };
                        downstream.send(message).await?;
                    }
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    }));
    let manager = Arc::new(
        EnvironmentManager::create_for_tests(
            Some(proxy_websocket_url),
            /*local_runtime_paths*/ None,
        )
        .await,
    );
    manager
        .default_environment()
        .context("remote environment")?
        .info()
        .await?;

    let cache = Arc::new(ExecutorCapabilityDiscoveryCache::new(Arc::clone(&manager)));
    let skill_root = tempfile::tempdir()?;
    let selected_roots = vec![SelectedCapabilityRoot {
        id: "recovering-skill".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            path: PathUri::from_host_native_path(skill_root.path())?,
        },
    }];

    let snapshot = timeout(
        Duration::from_secs(12),
        cache.snapshot(&selected_roots, &HashMap::new()),
    )
    .await
    .context("capability discovery did not retry within the same request")?;
    let discovery = snapshot.roots()[0]
        .result
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))?;

    assert_eq!(discovery.id, "recovering-skill");
    assert_eq!(
        2,
        discovery_attempts.load(Ordering::SeqCst),
        "same-request retry must issue a second capability discovery RPC"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capability_discovery_retries_after_executor_reconnects() -> anyhow::Result<()> {
    let server = exec_server().await?;
    let manager = Arc::new(EnvironmentManager::default_for_tests());
    let cache = ExecutorCapabilityDiscoveryCache::new(Arc::clone(&manager));
    let skill_root = tempfile::tempdir()?;
    let refused_listener = TcpListener::bind("127.0.0.1:0").await?;
    let refused_address = refused_listener.local_addr()?;
    drop(refused_listener);
    manager.upsert_environment(
        "recovering".to_string(),
        format!("ws://{refused_address}"),
        Some(Duration::from_millis(100)),
    )?;
    let selected_roots = vec![SelectedCapabilityRoot {
        id: "recovering-skill".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "recovering".to_string(),
            path: PathUri::from_host_native_path(skill_root.path())?,
        },
    }];

    let failed_snapshot = cache.snapshot(&selected_roots, &HashMap::new()).await;
    assert!(failed_snapshot.roots()[0].result.is_err());
    assert!(!cache.take_recovered_discovery());

    manager.upsert_environment(
        "recovering".to_string(),
        server.websocket_url().to_string(),
        /*connect_timeout*/ None,
    )?;
    manager
        .get_environment("recovering")
        .context("recovered environment")?
        .wait_until_ready()
        .await?;

    let recovered_snapshot = cache.snapshot(&selected_roots, &HashMap::new()).await;
    let discovery = recovered_snapshot.roots()[0]
        .result
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))?;

    assert_eq!(discovery.id, "recovering-skill");
    assert!(cache.take_recovered_discovery());
    assert!(!cache.take_recovered_discovery());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capability_discovery_retries_after_application_network_policy_recovers()
-> anyhow::Result<()> {
    let server = exec_server().await?;
    let controller = codex_http_client::NetworkPolicyController::default();
    let policy = controller.policy();
    let manager = Arc::new(EnvironmentManager::without_environments(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
            .with_network_policy(policy.clone()),
    ));
    manager.upsert_environment(
        "recovering".to_string(),
        server.websocket_url().to_string(),
        /*connect_timeout*/ None,
    )?;
    let cache = ExecutorCapabilityDiscoveryCache::new(manager);
    let skill_root = tempfile::tempdir()?;
    let selected_roots = vec![SelectedCapabilityRoot {
        id: "recovering-skill".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "recovering".to_string(),
            path: PathUri::from_host_native_path(skill_root.path())?,
        },
    }];

    let failed = cache.snapshot(&selected_roots, &HashMap::new()).await;
    let error = failed.roots()[0].result.as_ref().unwrap_err();
    assert!(error.contains("application network policy"), "{error}");
    assert!(!cache.take_recovered_discovery());

    controller.publish(
        policy.revision(),
        codex_http_client::DestinationPolicy::Unrestricted,
    );
    let recovered = timeout(
        Duration::from_secs(/*secs*/ 10),
        cache.snapshot(&selected_roots, &HashMap::new()),
    )
    .await?;
    let discovery = recovered.roots()[0]
        .result
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    assert_eq!(discovery.id, "recovering-skill");
    assert!(cache.take_recovered_discovery());
    assert!(!cache.take_recovered_discovery());
    Ok(())
}
