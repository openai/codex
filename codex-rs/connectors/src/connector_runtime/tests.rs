use super::persistence::CODEX_APPS_TOOLS_CACHE_MAX_BYTES;
use super::persistence::CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION;
use super::persistence::read_cached_codex_apps_tools;
use super::persistence::write_cached_codex_apps_tools;
use super::persistence::write_cached_codex_apps_tools_for_test;
use super::*;
use codex_protocol::mcp::McpServerInfo;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde::Serialize;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

const CODEX_APPS_MCP_SERVER_NAME: &str = "codex_apps";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TestTool {
    server_name: String,
    callable_name: String,
    connector_id: Option<String>,
    connector_name: Option<String>,
}

fn create_test_tool(server_name: &str, tool_name: &str) -> TestTool {
    TestTool {
        server_name: server_name.to_string(),
        callable_name: tool_name.to_string(),
        connector_id: None,
        connector_name: None,
    }
}

fn create_test_tool_with_connector(
    server_name: &str,
    tool_name: &str,
    connector_id: &str,
    connector_name: Option<&str>,
) -> TestTool {
    let mut tool = create_test_tool(server_name, tool_name);
    tool.connector_id = Some(connector_id.to_string());
    tool.connector_name = connector_name.map(ToOwned::to_owned);
    tool
}

fn create_codex_apps_tools_cache_context(
    codex_home: PathBuf,
    account_id: Option<&str>,
    chatgpt_user_id: Option<&str>,
) -> ConnectorRuntimeContext<TestTool> {
    ConnectorRuntimeManager::<TestTool>::default().context(
        codex_home,
        ConnectorRuntimeContextKey {
            account_id: account_id.map(ToOwned::to_owned),
            chatgpt_user_id: chatgpt_user_id.map(ToOwned::to_owned),
            is_workspace_account: false,
        },
    )
}

fn create_test_server_info(title: &str) -> McpServerInfo {
    McpServerInfo {
        name: "codex-apps".to_string(),
        title: Some(title.to_string()),
        version: "1.0.0".to_string(),
        description: None,
        icons: None,
        website_url: None,
    }
}

#[test]
fn codex_apps_tools_cache_is_overwritten_by_last_write() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let tools_gateway_1 = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "one")];
    let tools_gateway_2 = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "two")];

    write_cached_codex_apps_tools(&cache_context, &tools_gateway_1).expect("write first cache");
    let cached_gateway_1 =
        read_cached_codex_apps_tools(&cache_context).expect("cache entry exists for first write");
    assert_eq!(cached_gateway_1[0].callable_name, "one");

    write_cached_codex_apps_tools(&cache_context, &tools_gateway_2).expect("write second cache");
    let cached_gateway_2 =
        read_cached_codex_apps_tools(&cache_context).expect("cache entry exists for second write");
    assert_eq!(cached_gateway_2[0].callable_name, "two");
}

#[test]
fn codex_apps_tools_cache_is_scoped_per_user() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context_user_1 = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cache_context_user_2 = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-two"),
        Some("user-two"),
    );
    let tools_user_1 = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "one")];
    let tools_user_2 = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "two")];

    write_cached_codex_apps_tools(&cache_context_user_1, &tools_user_1)
        .expect("write user one cache");
    write_cached_codex_apps_tools(&cache_context_user_2, &tools_user_2)
        .expect("write user two cache");

    let read_user_1 =
        read_cached_codex_apps_tools(&cache_context_user_1).expect("cache entry for user one");
    let read_user_2 =
        read_cached_codex_apps_tools(&cache_context_user_2).expect("cache entry for user two");

    assert_eq!(read_user_1[0].callable_name, "one");
    assert_eq!(read_user_2[0].callable_name, "two");
    assert_ne!(
        cache_context_user_1.tools_cache_path(),
        cache_context_user_2.tools_cache_path(),
        "each user should get an isolated cache file"
    );
}

#[test]
fn codex_apps_tools_cache_preserves_formerly_disallowed_connectors() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let tools = vec![
        create_test_tool_with_connector(
            CODEX_APPS_MCP_SERVER_NAME,
            "formerly_blocked_tool",
            "connector_2b0a9009c9c64bf9933a3dae3f2b1254",
            Some("Formerly Blocked"),
        ),
        create_test_tool_with_connector(
            CODEX_APPS_MCP_SERVER_NAME,
            "calendar_tool",
            "calendar",
            Some("Calendar"),
        ),
    ];

    write_cached_codex_apps_tools(&cache_context, &tools).expect("write cache");
    let cached = read_cached_codex_apps_tools(&cache_context).expect("cache entry exists for user");

    assert_eq!(
        cached
            .iter()
            .map(|tool| (tool.callable_name.as_str(), tool.connector_id.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (
                "formerly_blocked_tool",
                Some("connector_2b0a9009c9c64bf9933a3dae3f2b1254")
            ),
            ("calendar_tool", Some("calendar")),
        ]
    );
}

#[test]
fn codex_apps_tools_cache_is_ignored_when_schema_version_mismatches() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cache_path = cache_context.tools_cache_path();
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION + 1,
        "tools": [create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "one")],
    }))
    .expect("serialize");
    std::fs::write(cache_path, bytes).expect("write");

    assert!(read_cached_codex_apps_tools(&cache_context).is_none());
}

#[test]
fn codex_apps_tools_cache_is_ignored_when_json_is_invalid() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cache_path = cache_context.tools_cache_path();
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(cache_path, b"{not json").expect("write");

    assert!(read_cached_codex_apps_tools(&cache_context).is_none());
}

#[test]
fn startup_cached_codex_apps_tools_loads_from_disk_cache() {
    let codex_home = tempdir().expect("tempdir");
    let writer_cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cached_tools = vec![create_test_tool(
        CODEX_APPS_MCP_SERVER_NAME,
        "calendar_search",
    )];
    let server_info = create_test_server_info("Codex Apps");
    write_cached_codex_apps_tools_for_test(&writer_cache_context, &server_info, &cached_tools);
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );

    let startup_tools = cache_context
        .current_tools()
        .expect("expected startup snapshot to load from cache");
    let cached_server_info = cache_context.cached_server_info();

    assert_eq!(startup_tools.len(), 1);
    assert_eq!(startup_tools[0].server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(startup_tools[0].callable_name, "calendar_search");
    assert_eq!(cached_server_info, Some(server_info));
}

#[test]
fn startup_cached_codex_apps_tools_loads_without_server_info_cache() {
    let codex_home = tempdir().expect("tempdir");
    let writer_cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cache_path = writer_cache_context.tools_cache_path();
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION,
        "tools": [create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "calendar_search")],
    }))
    .expect("serialize");
    std::fs::write(cache_path, bytes).expect("write");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );

    let startup_tools = cache_context
        .current_tools()
        .expect("legacy startup snapshot should remain available");
    let cached_server_info = cache_context.cached_server_info();

    assert_eq!(startup_tools.len(), 1);
    assert_eq!(startup_tools[0].callable_name, "calendar_search");
    assert_eq!(cached_server_info, None);
}

#[test]
fn codex_apps_server_info_cache_survives_legacy_tools_cache_write() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let server_info = create_test_server_info("Codex Apps");
    write_cached_codex_apps_tools_for_test(
        &cache_context,
        &server_info,
        &[create_test_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "calendar_search",
        )],
    );

    let cache_path = cache_context.tools_cache_path();
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION - 1,
        "tools": [create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "calendar_search")],
    }))
    .expect("serialize");
    std::fs::write(cache_path, bytes).expect("write legacy tools cache");
    let startup_cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );

    assert_eq!(
        startup_cache_context.cached_server_info(),
        Some(server_info)
    );
    assert!(startup_cache_context.current_tools().is_none());
}

#[test]
fn codex_apps_tools_cache_context_does_not_reread_disk_after_creation() {
    let codex_home = tempdir().expect("tempdir");
    let writer_cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cached_tools = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "cached")];
    write_cached_codex_apps_tools(&writer_cache_context, &cached_tools).expect("write cache");
    let reader_cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let updated_tools = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "updated")];
    write_cached_codex_apps_tools(&writer_cache_context, &updated_tools).expect("rewrite cache");

    assert_eq!(
        reader_cache_context
            .current_tools()
            .expect("in-memory tools")[0]
            .callable_name,
        "cached"
    );
    assert_eq!(
        read_cached_codex_apps_tools(&writer_cache_context).expect("disk tools")[0].callable_name,
        "updated"
    );
}

#[test]
fn codex_apps_tools_cache_publishes_newest_shared_snapshot() {
    let codex_home = tempdir().expect("tempdir");
    let cache = ConnectorRuntimeManager::<TestTool>::default();
    let cache_context_1 = cache.context(
        codex_home.path().to_path_buf(),
        ConnectorRuntimeContextKey {
            account_id: Some("account-one".to_string()),
            chatgpt_user_id: Some("user-one".to_string()),
            is_workspace_account: false,
        },
    );
    let cache_context_2 = cache.context(
        codex_home.path().to_path_buf(),
        ConnectorRuntimeContextKey {
            account_id: Some("account-one".to_string()),
            chatgpt_user_id: Some("user-one".to_string()),
            is_workspace_account: false,
        },
    );
    let older_ticket = cache_context_1.begin_fetch(ConnectorRuntimeFetchSource::Startup);
    let newer_ticket = cache_context_2.begin_fetch(ConnectorRuntimeFetchSource::HardRefresh);
    let server_info = create_test_server_info("Codex Apps");
    let newer_tools = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "newer")];
    let older_tools = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "older")];

    let published_tools =
        cache_context_2.publish_if_newest_accepted(newer_ticket, &server_info, newer_tools);
    assert_eq!(cache_context_1.current_tools(), Some(published_tools));
    let current_tools =
        cache_context_1.publish_if_newest_accepted(older_ticket, &server_info, older_tools);

    assert_eq!(current_tools[0].callable_name, "newer");
    assert_eq!(
        cache_context_2.current_tools().expect("shared snapshot")[0].callable_name,
        "newer"
    );
    assert_eq!(
        read_cached_codex_apps_tools(&cache_context_1).expect("persisted snapshot")[0]
            .callable_name,
        "newer"
    );
}

#[test]
fn codex_apps_tools_cache_keeps_live_publish_when_disk_persistence_fails() {
    let codex_home = tempdir().expect("tempdir");
    let codex_home_file = codex_home.path().join("not-a-directory");
    std::fs::write(&codex_home_file, b"occupied").expect("create codex home file");
    let cache_context = ConnectorRuntimeManager::<TestTool>::default().context(
        codex_home_file,
        ConnectorRuntimeContextKey {
            account_id: Some("account-one".to_string()),
            chatgpt_user_id: Some("user-one".to_string()),
            is_workspace_account: false,
        },
    );
    let tools = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "live")];
    let published_tools = cache_context.publish_if_newest_accepted(
        cache_context.begin_fetch(ConnectorRuntimeFetchSource::HardRefresh),
        &create_test_server_info("Codex Apps"),
        tools.clone(),
    );

    assert_eq!(published_tools, tools);
    assert_eq!(cache_context.current_tools(), Some(tools));
}

#[test]
fn connector_runtime_without_cache_ignores_disk_state() {
    let codex_home = tempdir().expect("tempdir");
    let writer = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let tools = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "cached")];
    let server_info = create_test_server_info("Codex Apps");
    write_cached_codex_apps_tools_for_test(&writer, &server_info, &tools);
    let context = ConnectorRuntimeManager::<TestTool>::new_without_cache().context(
        codex_home.path().to_path_buf(),
        ConnectorRuntimeContextKey {
            account_id: Some("account-one".to_string()),
            chatgpt_user_id: Some("user-one".to_string()),
            is_workspace_account: false,
        },
    );

    assert_eq!(context.current_tools(), None);
    assert_eq!(context.cached_server_info(), None);
}

#[test]
fn connector_runtime_without_cache_publishes_without_writing() {
    let temp_dir = tempdir().expect("tempdir");
    let codex_home = temp_dir.path().join("codex-home");
    let context = ConnectorRuntimeManager::<TestTool>::new_without_cache().context(
        codex_home.clone(),
        ConnectorRuntimeContextKey {
            account_id: Some("account-one".to_string()),
            chatgpt_user_id: Some("user-one".to_string()),
            is_workspace_account: false,
        },
    );
    let tools = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "live")];
    let published_tools = context.publish_if_newest_accepted(
        context.begin_fetch(ConnectorRuntimeFetchSource::HardRefresh),
        &create_test_server_info("Codex Apps"),
        tools.clone(),
    );

    assert_eq!(published_tools, tools);
    assert_eq!(context.current_tools(), Some(tools));
    assert!(!codex_home.exists());
}

#[cfg(unix)]
#[test]
fn codex_apps_tools_cache_scopes_non_utf8_home_disk_paths() {
    let codex_home = PathBuf::from(std::ffi::OsString::from_vec(
        b"/tmp/codex-home-\xff".to_vec(),
    ));
    let cache = ConnectorRuntimeManager::<TestTool>::default();
    let user_one_context = cache.context(
        codex_home.clone(),
        ConnectorRuntimeContextKey {
            account_id: Some("account-one".to_string()),
            chatgpt_user_id: Some("user-one".to_string()),
            is_workspace_account: false,
        },
    );
    let user_two_context = cache.context(
        codex_home,
        ConnectorRuntimeContextKey {
            account_id: Some("account-two".to_string()),
            chatgpt_user_id: Some("user-two".to_string()),
            is_workspace_account: false,
        },
    );
    let cache_paths = [
        user_one_context.tools_cache_path(),
        user_two_context.tools_cache_path(),
    ];

    assert_ne!(cache_paths[0], cache_paths[1]);
}

#[test]
fn contexts_for_different_identities_keep_isolated_snapshots() {
    let codex_home = tempdir().expect("tempdir");
    let manager = ConnectorRuntimeManager::<TestTool>::default();
    let context_a = manager.context(
        codex_home.path().to_path_buf(),
        ConnectorRuntimeContextKey {
            account_id: Some("account-a".to_string()),
            chatgpt_user_id: Some("user-a".to_string()),
            is_workspace_account: false,
        },
    );
    let tools_a = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "tool-a")];
    let snapshot_a = context_a.publish_runtime_if_newest_accepted(
        context_a.begin_fetch(ConnectorRuntimeFetchSource::HardRefresh),
        &create_test_server_info("Codex Apps"),
        tools_a.clone(),
    );
    let older_ticket_a = context_a.begin_fetch(ConnectorRuntimeFetchSource::Startup);
    let context_b = manager.context(
        codex_home.path().to_path_buf(),
        ConnectorRuntimeContextKey {
            account_id: Some("account-b".to_string()),
            chatgpt_user_id: Some("user-b".to_string()),
            is_workspace_account: false,
        },
    );
    let same_context_a = manager.context(
        codex_home.path().to_path_buf(),
        ConnectorRuntimeContextKey {
            account_id: Some("account-a".to_string()),
            chatgpt_user_id: Some("user-a".to_string()),
            is_workspace_account: false,
        },
    );

    assert!(Arc::ptr_eq(
        &snapshot_a,
        &same_context_a
            .current_snapshot()
            .expect("context A snapshot")
    ));
    assert!(context_b.current_snapshot().is_none());

    let tools_b = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "tool-b")];
    let snapshot_b = context_b.publish_runtime_if_newest_accepted(
        context_b.begin_fetch(ConnectorRuntimeFetchSource::HardRefresh),
        &create_test_server_info("Codex Apps"),
        tools_b.clone(),
    );
    let newer_tools_a = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "newer-a")];
    let newer_snapshot_a = same_context_a.publish_runtime_if_newest_accepted(
        same_context_a.begin_fetch(ConnectorRuntimeFetchSource::HardRefresh),
        &create_test_server_info("Codex Apps"),
        newer_tools_a.clone(),
    );
    let stale_snapshot_a = context_a.publish_runtime_if_newest_accepted(
        older_ticket_a,
        &create_test_server_info("Codex Apps"),
        vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "stale-a")],
    );

    assert_eq!(snapshot_a.tools(), &tools_a);
    assert_eq!(snapshot_b.tools(), &tools_b);
    assert_eq!(newer_snapshot_a.tools(), &newer_tools_a);
    assert!(Arc::ptr_eq(&newer_snapshot_a, &stale_snapshot_a));
    assert!(Arc::ptr_eq(
        &newer_snapshot_a,
        &context_a.current_snapshot().expect("context A snapshot")
    ));
    assert!(Arc::ptr_eq(
        &snapshot_b,
        &context_b.current_snapshot().expect("context B snapshot")
    ));
}

#[test]
fn oversized_tools_cache_is_ignored_during_initial_load() {
    let codex_home = tempdir().expect("tempdir");
    let context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cache_path = context.tools_cache_path();
    std::fs::create_dir_all(cache_path.parent().expect("cache parent"))
        .expect("create cache parent");
    let file = std::fs::File::create(cache_path).expect("create oversized cache");
    file.set_len(CODEX_APPS_TOOLS_CACHE_MAX_BYTES + 1)
        .expect("size oversized cache");

    let reloaded = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );

    assert!(reloaded.current_snapshot().is_none());
}

#[test]
fn cold_loaded_snapshot_uses_cache_modification_time() {
    let codex_home = tempdir().expect("tempdir");
    let writer = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let tools = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "cached")];
    write_cached_codex_apps_tools(&writer, &tools).expect("write tools cache");
    let modified_at = std::fs::metadata(writer.tools_cache_path())
        .and_then(|metadata| metadata.modified())
        .expect("cache modification time");

    let reloaded = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let snapshot = reloaded.current_snapshot().expect("cold-loaded snapshot");

    assert_eq!(snapshot.tools(), &tools);
    assert_eq!(snapshot.refreshed_at(), modified_at);
}
#[test]
fn accepted_generations_finish_persistence_in_order() {
    let codex_home = tempdir().expect("tempdir");
    let context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let older_ticket = context.begin_fetch(ConnectorRuntimeFetchSource::Startup);
    let newer_ticket = context.begin_fetch(ConnectorRuntimeFetchSource::HardRefresh);
    let (older_persisting_tx, older_persisting_rx) = std::sync::mpsc::channel();
    let (release_older_tx, release_older_rx) = std::sync::mpsc::channel();
    let older_context = context.clone();
    let older_publish = std::thread::spawn(move || {
        older_context.publish_runtime_if_newest_accepted_with(
            older_ticket,
            &create_test_server_info("Codex Apps"),
            vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "older")],
            move |_, _, _| {
                older_persisting_tx
                    .send(())
                    .expect("signal older persistence");
                release_older_rx.recv().expect("release older persistence");
            },
        )
    });
    older_persisting_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("older generation should enter persistence");

    let (newer_persisting_tx, newer_persisting_rx) = std::sync::mpsc::channel();
    let newer_context = context;
    let newer_publish = std::thread::spawn(move || {
        newer_context.publish_runtime_if_newest_accepted_with(
            newer_ticket,
            &create_test_server_info("Codex Apps"),
            vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "newer")],
            move |_, _, _| {
                newer_persisting_tx
                    .send(())
                    .expect("signal newer persistence");
            },
        )
    });

    assert!(
        newer_persisting_rx
            .recv_timeout(Duration::from_millis(20))
            .is_err()
    );
    release_older_tx
        .send(())
        .expect("allow older persistence to finish");
    newer_persisting_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("newer generation should persist after older generation");

    older_publish.join().expect("join older publish");
    let newer_snapshot = newer_publish.join().expect("join newer publish");
    assert_eq!(newer_snapshot.tools()[0].callable_name, "newer");
}

#[test]
fn personal_and_workspace_contexts_are_distinct_even_with_matching_ids() {
    let codex_home = tempdir().expect("tempdir");
    let manager = ConnectorRuntimeManager::<TestTool>::default();
    let personal_context = manager.context(
        codex_home.path().to_path_buf(),
        ConnectorRuntimeContextKey {
            account_id: Some("account".to_string()),
            chatgpt_user_id: Some("user".to_string()),
            is_workspace_account: false,
        },
    );
    let personal_tools = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "personal")];
    let _ = personal_context.publish_runtime_if_newest_accepted(
        personal_context.begin_fetch(ConnectorRuntimeFetchSource::Startup),
        &create_test_server_info("Codex Apps"),
        personal_tools.clone(),
    );

    let workspace_context = manager.context(
        codex_home.path().to_path_buf(),
        ConnectorRuntimeContextKey {
            account_id: Some("account".to_string()),
            chatgpt_user_id: Some("user".to_string()),
            is_workspace_account: true,
        },
    );

    let workspace_tools = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "workspace")];
    let _ = workspace_context.publish_runtime_if_newest_accepted(
        workspace_context.begin_fetch(ConnectorRuntimeFetchSource::Startup),
        &create_test_server_info("Codex Apps"),
        workspace_tools.clone(),
    );

    assert_eq!(personal_context.current_tools(), Some(personal_tools));
    assert_eq!(workspace_context.current_tools(), Some(workspace_tools));
    assert_ne!(
        personal_context.tools_cache_path(),
        workspace_context.tools_cache_path()
    );
}

#[test]
fn live_publish_sets_timestamp_and_stale_publish_preserves_it() {
    let codex_home = tempdir().expect("tempdir");
    let context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let stale_ticket = context.begin_fetch(ConnectorRuntimeFetchSource::Startup);
    let current_ticket = context.begin_fetch(ConnectorRuntimeFetchSource::HardRefresh);
    let before = SystemTime::now();
    let current = context.publish_runtime_if_newest_accepted(
        current_ticket,
        &create_test_server_info("Codex Apps"),
        vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "current")],
    );
    let after = SystemTime::now();

    assert!(current.refreshed_at() >= before);
    assert!(current.refreshed_at() <= after);

    let stale = context.publish_runtime_if_newest_accepted(
        stale_ticket,
        &create_test_server_info("Codex Apps"),
        vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "stale")],
    );
    assert!(Arc::ptr_eq(&current, &stale));
    assert_eq!(stale.refreshed_at(), current.refreshed_at());
}
