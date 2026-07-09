mod common;

use std::time::Duration;

use anyhow::Context;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_exec_server::SelectedCapabilityRootsStatus;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;
use tokio::time::sleep;
use tokio::time::timeout;

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
