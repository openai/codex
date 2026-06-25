#![cfg(unix)]

mod common;

use std::collections::HashMap;
use std::sync::Arc;

use codex_exec_server::EnvironmentManager;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_capability_roots_follow_same_id_environment_replacement() -> anyhow::Result<()> {
    let mut executor_a = exec_server().await?;
    let mut executor_b = exec_server().await?;
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
        executor_a.websocket_url().to_string(),
        /*connect_timeout*/ None,
    )?;
    let environment_a = manager
        .get_environment("tools")
        .expect("executor A should be registered");
    environment_a.wait_until_ready().await?;
    let resolved_a = manager
        .resolve_selected_capability_roots(std::slice::from_ref(&selected_root), &HashMap::new())
        .await;
    let [resolved_a] = resolved_a.as_slice() else {
        anyhow::bail!("selected root should resolve through executor A");
    };

    manager.upsert_environment(
        "tools".to_string(),
        executor_b.websocket_url().to_string(),
        /*connect_timeout*/ None,
    )?;
    let environment_b = manager
        .get_environment("tools")
        .expect("executor B should be registered");
    environment_b.wait_until_ready().await?;
    let resolved_b = manager
        .resolve_selected_capability_roots(std::slice::from_ref(&selected_root), &HashMap::new())
        .await;
    let [resolved_b] = resolved_b.as_slice() else {
        anyhow::bail!("selected root should resolve through executor B");
    };

    assert_eq!(resolved_b.selected_root(), &selected_root);
    assert!(Arc::ptr_eq(resolved_a.environment(), &environment_a));
    assert!(Arc::ptr_eq(resolved_b.environment(), &environment_b));
    assert!(!Arc::ptr_eq(
        resolved_a.environment(),
        resolved_b.environment()
    ));

    executor_a.shutdown().await?;
    executor_b.shutdown().await?;
    Ok(())
}
