#![cfg(unix)]

mod common;

use std::collections::HashMap;
use std::sync::Arc;

use codex_exec_server_test_support::environment_manager_without_environments;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_capability_roots_use_captured_handle_after_replacement() -> anyhow::Result<()> {
    let mut executor = exec_server().await?;
    let manager = environment_manager_without_environments();
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
    let environment_a = manager
        .get_environment("tools")
        .expect("executor A should be registered");
    environment_a.wait_until_ready().await?;

    let unavailable = manager
        .resolve_selected_capability_roots(
            std::slice::from_ref(&selected_root),
            &HashMap::from([("tools".to_string(), None)]),
        )
        .await;
    assert!(unavailable.is_empty());

    let captured_environments =
        HashMap::from([("tools".to_string(), Some(Arc::clone(&environment_a)))]);
    // Replace only the process-local handle; the stable environment ID and executor stay the same.
    manager.upsert_environment(
        "tools".to_string(),
        executor.websocket_url().to_string(),
        /*connect_timeout*/ None,
    )?;

    let available = manager
        .resolve_selected_capability_roots(
            std::slice::from_ref(&selected_root),
            &captured_environments,
        )
        .await;
    let [resolved] = available.as_slice() else {
        anyhow::bail!("selected root should resolve through its stable environment");
    };

    assert_eq!(resolved.selected_root(), &selected_root);
    assert!(Arc::ptr_eq(resolved.environment(), &environment_a));

    executor.shutdown().await?;
    Ok(())
}
