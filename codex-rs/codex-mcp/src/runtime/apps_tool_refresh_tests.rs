#![cfg(test)]
//! Verifies that an in-flight Apps refresh cannot publish after invalidation.

use super::*;
use tokio::sync::Notify;

#[tokio::test]
async fn stale_apps_refresh_does_not_publish() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let auth = CodexAuth::from_external_chatgpt_tokens(
        "header.e30.token",
        "account",
        /*chatgpt_plan_type*/ None,
    )?;
    let auth_manager =
        AuthManager::from_auth_for_testing_with_home(auth.clone(), home.path().to_path_buf());
    let cache = ConnectorRuntimeManager::<ToolInfo>::default().context(
        home.path().to_path_buf(),
        ConnectorRuntimeContextKey::personal(
            /*account_id*/ None, /*chatgpt_user_id*/ None,
        ),
    );
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let connections = crate::connection_manager::tests::create_test_manager_with_ready_apps_client(
        cache.clone(),
        "search",
        Some(started.clone()),
        Some(release.clone()),
    )
    .await?;
    connections.list_all_tools().await;
    let revision = connections
        .stable_catalog_revisions(/*required_servers*/ &[], &HashSet::new())
        .await
        .expect("Apps catalog is ready");
    let runtime = McpRuntime::empty(/*prefix_mcp_tool_names*/ true);
    runtime.current.store(Arc::new(PublishedMcpRuntime {
        connections: connections.clone(),
        config: Some(Arc::new(crate::mcp::tests::test_mcp_config(
            home.path().to_path_buf(),
        ))),
        auth_token: Some(auth.get_token()?),
        auth: Some(auth),
        auth_manager: Some(auth_manager.clone()),
        auth_owner: None,
        auth_generation: Arc::new(()),
        plugins_available: false,
        ready_selected_capability_roots: Vec::new(),
        environment_selections: Arc::default(),
        ready_environments: HashMap::new(),
        cached_binding: Mutex::new(None),
    }));

    let refresh = runtime.capture_apps_tool_refresh();
    tokio::pin!(refresh);
    tokio::select! {
        _ = started.notified() => {}
        _ = &mut refresh => panic!("refresh completed before tools/list was released"),
    }
    runtime.invalidate_apps_tool_refresh();
    release.notify_one();
    assert!(
        refresh.await?.is_none(),
        "runtime invalidation supersedes the fetch"
    );

    let refresh = runtime.capture_apps_tool_refresh();
    tokio::pin!(refresh);
    tokio::select! {
        _ = started.notified() => {}
        _ = &mut refresh => panic!("refresh completed before tools/list was released"),
    }
    auth_manager.logout().await?;
    release.notify_one();
    assert!(refresh.await?.is_none(), "logout supersedes the fetch");

    assert!(
        !cache.has_current_tools(),
        "stale tools reached the shared cache"
    );
    assert!(
        connections
            .stable_catalog_revisions(/*required_servers*/ &[], &HashSet::new())
            .await
            .as_ref()
            == Some(&revision),
        "a stale fetch invalidated the existing client catalog"
    );
    Ok(())
}
