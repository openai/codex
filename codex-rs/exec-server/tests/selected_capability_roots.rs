#![cfg(unix)]

mod common;

use std::collections::HashMap;

use codex_exec_server::EnvironmentManager;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_capability_roots_follow_captured_availability() -> anyhow::Result<()> {
    let mut executor = exec_server().await?;
    let manager = EnvironmentManager::without_environments();
    let selected_root = SelectedCapabilityRoot {
        id: "demo@1".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "tools".to_string(),
            path: PathUri::parse("file:///plugins/demo")?,
        },
    };

    manager.upsert_environment(
        "tools".to_string(),
        executor.websocket_url().to_string(),
        /*connect_timeout*/ None,
    )?;
    let environment = manager
        .get_environment("tools")
        .expect("executor should be registered");
    environment.wait_until_ready().await?;

    let unavailable = manager
        .resolve_selected_capability_roots(
            std::slice::from_ref(&selected_root),
            &HashMap::from([("tools".to_string(), false)]),
        )
        .await;
    assert!(unavailable.is_empty());

    let available = manager
        .resolve_selected_capability_roots(
            std::slice::from_ref(&selected_root),
            &HashMap::from([("tools".to_string(), true)]),
        )
        .await;
    let [resolved] = available.as_slice() else {
        anyhow::bail!("selected root should resolve through its stable environment");
    };

    assert_eq!(resolved.selected_root(), &selected_root);
    assert!(std::sync::Arc::ptr_eq(resolved.environment(), &environment));

    executor.shutdown().await?;
    Ok(())
}
