mod common;

use std::time::Duration;

use anyhow::Context;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_exec_server::SelectedCapabilityRootsStatus;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::cache_system_proxy_route_for_test;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::time::sleep;
use tokio::time::timeout;
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
