use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::start_analytics_events_server;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportCompletedNotification;
use codex_app_server_protocol::ExternalAgentConfigImportHistoriesReadResponse;
use codex_app_server_protocol::ExternalAgentConfigImportProgressNotification;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::ExternalAgentImportedConnectorCandidate;
use codex_app_server_protocol::ExternalAgentImportedConnectorSource;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
#[cfg(unix)]
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

use super::analytics::wait_for_analytics_event;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const SECONDARY_MIGRATION_SOURCE: &str = concat!("cur", "sor");

fn external_agent_home(codex_home: &Path) -> PathBuf {
    codex_home.join(concat!(".", "cla", "ude"))
}

fn connector_metadata_root(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Claude")
    }
    #[cfg(target_os = "windows")]
    {
        home.join("AppData/Roaming/Claude")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        home.join(".config/Claude")
    }
}

fn secondary_external_agent_home(codex_home: &Path) -> PathBuf {
    codex_home.join(concat!(".", "cur", "sor"))
}

fn assert_import_response(response: ExternalAgentConfigImportResponse) -> String {
    assert!(!response.import_id.is_empty());
    response.import_id
}

#[tokio::test]
async fn external_agent_config_detect_accepts_migration_source_and_defaults_unknown_values()
-> Result<()> {
    let codex_home = TempDir::new()?;
    let source_home = external_agent_home(codex_home.path());
    std::fs::create_dir_all(&source_home)?;
    std::fs::write(source_home.join("CLAUDE.md"), "project instructions")?;
    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let mut responses = Vec::new();
    for params in [
        serde_json::json!({ "includeHome": true }),
        serde_json::json!({
            "includeHome": true,
            "migrationSource": "claude-code",
        }),
        serde_json::json!({
            "includeHome": true,
            "migrationSource": "unknown-source",
        }),
        serde_json::json!({
            "includeHome": true,
            "source": SECONDARY_MIGRATION_SOURCE,
        }),
    ] {
        let request_id = mcp
            .send_raw_request("externalAgentConfig/detect", Some(params))
            .await?;
        responses.push(
            timeout(
                DEFAULT_TIMEOUT,
                mcp.read_response::<ExternalAgentConfigDetectResponse>(request_id),
            )
            .await??,
        );
    }

    assert_eq!(responses[0].items.len(), 1);
    assert_eq!(
        responses[0].items[0].item_type,
        ExternalAgentConfigMigrationItemType::AgentsMd
    );
    let expected = responses[0].clone();
    assert_eq!(responses, vec![expected; 4]);

    Ok(())
}

#[tokio::test]
async fn external_agent_config_migration_source_drives_detect_and_import() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source_home = secondary_external_agent_home(codex_home.path());
    std::fs::create_dir_all(&source_home)?;
    std::fs::write(source_home.join("sandbox.json"), r#"{"type":"read_only"}"#)?;
    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({
                "includeHome": true,
                "migrationSource": SECONDARY_MIGRATION_SOURCE,
            })),
        )
        .await?;
    let detected: ExternalAgentConfigDetectResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(detected.items.len(), 1);
    assert_eq!(
        detected.items[0].item_type,
        ExternalAgentConfigMigrationItemType::Config
    );

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "migrationSource": SECONDARY_MIGRATION_SOURCE,
                "migrationItems": detected.items,
            })),
        )
        .await?;
    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);
    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);
    assert_eq!(completed.item_type_results.len(), 1);
    assert_eq!(completed.item_type_results[0].successes.len(), 1);
    assert_eq!(completed.item_type_results[0].failures, Vec::new());
    assert!(
        std::fs::read_to_string(codex_home.path().join("config.toml"))?
            .contains("sandbox_mode = \"read-only\"")
    );

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_source_remains_attribution_only() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source_home = external_agent_home(codex_home.path());
    std::fs::create_dir_all(&source_home)?;
    std::fs::write(source_home.join("CLAUDE.md"), "Claude guidance")?;
    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({ "includeHome": true })),
        )
        .await?;
    let detected: ExternalAgentConfigDetectResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(detected.items.len(), 1);
    assert_eq!(
        detected.items[0].item_type,
        ExternalAgentConfigMigrationItemType::AgentsMd
    );

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "source": SECONDARY_MIGRATION_SOURCE,
                "migrationItems": detected.items,
            })),
        )
        .await?;
    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);
    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);
    assert_eq!(completed.item_type_results.len(), 1);
    assert_eq!(completed.item_type_results[0].successes.len(), 1);
    assert_eq!(completed.item_type_results[0].failures, Vec::new());
    assert_eq!(
        std::fs::read_to_string(codex_home.path().join("AGENTS.md"))?,
        "Codex guidance"
    );

    Ok(())
}

#[tokio::test]
async fn external_agent_config_secondary_source_imports_session_and_plugin_end_to_end() -> Result<()>
{
    let codex_home = TempDir::new()?;
    let source_home = secondary_external_agent_home(codex_home.path());
    let project_root = codex_home.path().join("workspace with.dots_and-dashes");
    std::fs::create_dir_all(&project_root)?;

    let encoded_project = project_root
        .to_string_lossy()
        .trim_start_matches(['/', '\\'])
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    #[cfg(windows)]
    let encoded_project = encoded_project.replacen("--", "-", /*count*/ 1);
    let session_path = source_home
        .join("projects")
        .join(encoded_project)
        .join("agent-transcripts/session-1/session-1.jsonl");
    std::fs::create_dir_all(session_path.parent().expect("session parent"))?;
    std::fs::write(
        &session_path,
        [
            serde_json::json!({
                "role": "user",
                "message": {
                    "content": [{
                        "type": "text",
                        "text": "<user_query>first request</user_query>"
                    }]
                }
            })
            .to_string(),
            serde_json::json!({
                "role": "assistant",
                "message": {
                    "content": [{"type": "text", "text": "first answer"}]
                }
            })
            .to_string(),
        ]
        .join("\n"),
    )?;

    let marketplace_root = source_home.join("plugins/marketplaces/debug");
    let plugin_root = marketplace_root.join("plugins/sample");
    let configured_marketplace_root = codex_home.path().join("configured-marketplace");
    let configured_marketplace_manifest =
        configured_marketplace_root.join(".agents/plugins/marketplace.json");
    let configured_plugin_root = configured_marketplace_root.join("plugins/sample");
    std::fs::create_dir_all(marketplace_root.join(".cursor-plugin"))?;
    std::fs::create_dir_all(plugin_root.join(".cursor-plugin"))?;
    std::fs::create_dir_all(source_home.join("plugins/cache/debug/sample"))?;
    std::fs::create_dir_all(
        configured_marketplace_manifest
            .parent()
            .expect("configured marketplace manifest parent"),
    )?;
    std::fs::create_dir_all(configured_plugin_root.join(".codex-plugin"))?;
    std::fs::write(
        marketplace_root.join(".cursor-plugin/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [{"name": "sample", "source": "plugins/sample"}]
}"#,
    )?;
    std::fs::write(
        plugin_root.join(".cursor-plugin/plugin.json"),
        r#"{"name":"sample","version":"0.2.0"}"#,
    )?;
    std::fs::write(
        &configured_marketplace_manifest,
        r#"{
  "name": "debug",
  "plugins": [{
    "name": "sample",
    "source": {"source": "local", "path": "./plugins/sample"}
  }]
}"#,
    )?;
    std::fs::write(
        configured_plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample","version":"0.1.0"}"#,
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"[marketplaces.debug]
source_type = "local"
source = {:?}
"#,
            configured_marketplace_root.display().to_string()
        ),
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({
                "includeHome": true,
                "migrationSource": SECONDARY_MIGRATION_SOURCE,
            })),
        )
        .await?;
    let detected: ExternalAgentConfigDetectResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(detected.items.len(), 2);
    assert!(
        detected
            .items
            .iter()
            .any(|item| item.item_type == ExternalAgentConfigMigrationItemType::Sessions)
    );
    assert!(
        detected
            .items
            .iter()
            .any(|item| item.item_type == ExternalAgentConfigMigrationItemType::Plugins)
    );

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "migrationSource": SECONDARY_MIGRATION_SOURCE,
                "migrationItems": detected.items,
            })),
        )
        .await?;
    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);
    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);
    assert_eq!(completed.item_type_results.len(), 2);
    assert!(
        completed
            .item_type_results
            .iter()
            .all(|result| result.failures.is_empty())
    );

    let request_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: None,
            sort_key: None,
            sort_direction: None,
            model_providers: None,
            source_kinds: None,
            archived: None,
            cwd: None,
            use_state_db_only: false,
            search_term: None,
            parent_thread_id: None,
            ancestor_thread_id: None,
        })
        .await?;
    let response: ThreadListResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let thread = response.data.first().expect("imported session");
    assert_eq!(thread.cwd.as_path(), project_root);
    assert_eq!(thread.preview, "first request");
    assert_eq!(thread.name, None);

    let request_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id.clone(),
            include_turns: true,
        })
        .await?;
    let response: ThreadReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(response.thread.turns.len(), 1);
    let imported_items = &response.thread.turns[0].items;
    assert_eq!(imported_items.len(), 3);
    match &imported_items[0] {
        ThreadItem::UserMessage { content, .. } => assert_eq!(
            content,
            &vec![UserInput::Text {
                text: "first request".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected user message item, got {other:?}"),
    }
    match &imported_items[1] {
        ThreadItem::AgentMessage { text, .. } => assert_eq!(text, "first answer"),
        other => panic!("expected agent message item, got {other:?}"),
    }

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: None,
            marketplace_kinds: None,
            force_refetch: false,
        })
        .await?;
    let response: PluginListResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let marketplace = response
        .marketplaces
        .iter()
        .find(|marketplace| marketplace.name == "debug")
        .expect("configured marketplace");
    assert_eq!(
        marketplace
            .path
            .as_ref()
            .map(codex_config::AbsolutePathBuf::as_path),
        Some(configured_marketplace_manifest.as_path())
    );
    let plugin = marketplace
        .plugins
        .iter()
        .find(|plugin| plugin.name == "sample")
        .expect("imported plugin");
    assert_eq!(plugin.local_version.as_deref(), Some("0.1.0"));
    assert!(plugin.installed);
    assert!(plugin.enabled);

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_sends_completion_notification_for_sync_only_import()
-> Result<()> {
    let codex_home = TempDir::new()?;
    let sqlite_home = TempDir::new()?;
    let home_dir = codex_home.path().display().to_string();
    let sqlite_home_dir = sqlite_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[
            ("HOME", Some(home_dir.as_str())),
            ("CODEX_SQLITE_HOME", Some(sqlite_home_dir.as_str())),
        ])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "migrationItems": [{
                    "itemType": "CONFIG",
                    "description": "Import config",
                    "cwd": null
                }]
            })),
        )
        .await?;

    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);
    let progress: ExternalAgentConfigImportProgressNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/progress"),
    )
    .await??;
    assert_eq!(progress.import_id, import_id);
    assert_eq!(progress.item_type_results.len(), 1);
    assert_eq!(
        progress.item_type_results[0].item_type,
        ExternalAgentConfigMigrationItemType::Config
    );

    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);
    let state_db =
        codex_state::StateRuntime::init(sqlite_home.path().to_path_buf(), "mock_provider".into())
            .await?;
    let details_record = state_db
        .external_agent_config_import_details_record(&import_id)
        .await?
        .expect("completed import details should be recorded by import id");
    let expected_successes = completed
        .item_type_results
        .iter()
        .flat_map(|type_result| type_result.successes.iter())
        .collect::<Vec<_>>();
    let expected_failures = completed
        .item_type_results
        .iter()
        .flat_map(|type_result| type_result.failures.iter())
        .collect::<Vec<_>>();
    assert_eq!(
        serde_json::to_value(&details_record.successes)?,
        serde_json::to_value(&expected_successes)?
    );
    assert_eq!(
        serde_json::to_value(&details_record.failures)?,
        serde_json::to_value(&expected_failures)?
    );

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import/readHistories",
            /*params*/ None,
        )
        .await?;
    let response: ExternalAgentConfigImportHistoriesReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(response.connectors, Vec::new());
    let entry = response
        .data
        .iter()
        .find(|entry| entry.import_id == import_id)
        .expect("import history entry should be available");
    assert!(entry.completed_at_ms > 0);
    assert_eq!(
        serde_json::to_value(&entry.successes)?,
        serde_json::to_value(&expected_successes)?
    );
    assert_eq!(
        serde_json::to_value(&entry.failures)?,
        serde_json::to_value(&expected_failures)?
    );

    Ok(())
}

#[tokio::test]
async fn external_agent_memory_import_requires_feature_config() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source_home = external_agent_home(codex_home.path());
    let source_memory = source_home.join("projects/project-a/memory");
    std::fs::create_dir_all(&source_memory)?;
    let source_file = source_memory.join("MEMORY.md");
    std::fs::write(&source_file, "project A memory")?;
    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({ "includeHome": true })),
        )
        .await?;
    let detected: ExternalAgentConfigDetectResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(detected.items, Vec::new());

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "migrationItems": [{
                    "itemType": "MEMORY",
                    "description": "Import memory",
                    "cwd": null,
                    "details": {
                        "memory": ["project-a"]
                    }
                }]
            })),
        )
        .await?;
    let error = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        error.error.message,
        "external agent memory import is disabled"
    );
    assert!(
        !codex_home
            .path()
            .join("memories/extensions/external_agent_import")
            .exists()
    );

    Ok(())
}

#[tokio::test]
async fn external_agent_config_detects_non_memory_items_when_config_reload_fails() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source_home = external_agent_home(codex_home.path());
    std::fs::create_dir_all(&source_home)?;
    std::fs::write(source_home.join("CLAUDE.md"), "project instructions")?;
    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "this is not valid = [toml",
    )?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({ "includeHome": true })),
        )
        .await?;
    let detected: ExternalAgentConfigDetectResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(
        detected
            .items
            .iter()
            .map(|item| item.item_type)
            .collect::<Vec<_>>(),
        vec![ExternalAgentConfigMigrationItemType::AgentsMd]
    );

    Ok(())
}

#[tokio::test]
async fn external_agent_config_detects_and_imports_project_memory_files() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source_home = external_agent_home(codex_home.path());
    let source_project = source_home.join("projects/project-a");
    let source_memory = source_project.join("memory");
    let project_cwd = codex_home.path().join("project-a");
    std::fs::create_dir_all(&source_memory)?;
    std::fs::create_dir_all(&project_cwd)?;
    let project_cwd = std::fs::canonicalize(project_cwd)?;
    let source_file = source_memory.join("MEMORY.md");
    let source_topic = source_memory.join("release-process.md");
    std::fs::write(&source_file, "project A memory")?;
    std::fs::write(&source_topic, "project A release process")?;
    std::fs::write(
        source_project.join("session.jsonl"),
        serde_json::json!({
            "type": "user",
            "cwd": &project_cwd,
            "timestamp": "2026-07-13T00:00:00Z",
            "message": { "content": "remember this" },
        })
        .to_string(),
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "[features]\nexternal_agent_memory_import = true\n",
    )?;
    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    for details in [serde_json::json!({}), serde_json::json!({ "memory": [] })] {
        let request_id = mcp
            .send_raw_request(
                "externalAgentConfig/import",
                Some(serde_json::json!({
                    "migrationItems": [{
                        "itemType": "MEMORY",
                        "description": "Import memory",
                        "cwd": null,
                        "details": details,
                    }]
                })),
            )
            .await?;
        let error = timeout(
            DEFAULT_TIMEOUT,
            mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
        )
        .await??;
        assert_eq!(
            error.error.message,
            "memory import requires at least one selected memory"
        );
    }

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({ "includeHome": true })),
        )
        .await?;
    let mut detected: ExternalAgentConfigDetectResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    detected
        .items
        .retain(|item| item.item_type == ExternalAgentConfigMigrationItemType::Memory);
    assert_eq!(detected.items.len(), 1);
    let memory_item = &detected.items[0];
    assert_eq!(
        memory_item.item_type,
        ExternalAgentConfigMigrationItemType::Memory
    );
    assert_eq!(memory_item.cwd, None);
    assert_eq!(
        memory_item
            .details
            .as_ref()
            .expect("memory details")
            .memory
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["project-a"]
    );
    detected.items[0]
        .details
        .as_mut()
        .expect("memory details")
        .memory
        .push("missing-project".to_string());

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({ "migrationItems": detected.items })),
        )
        .await?;
    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);
    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);
    assert_eq!(completed.item_type_results.len(), 1);
    let memory_result = &completed.item_type_results[0];
    assert_eq!(
        memory_result.item_type,
        ExternalAgentConfigMigrationItemType::Memory
    );
    assert_eq!(memory_result.failures.len(), 1);
    assert_eq!(
        memory_result.failures[0].source.as_deref(),
        Some("missing-project")
    );
    assert_eq!(memory_result.failures[0].failure_stage, "memory_import");
    assert_eq!(memory_result.successes.len(), 1);
    assert_eq!(
        memory_result.successes[0].source.as_deref(),
        Some("project-a")
    );

    let imported_resources_root = PathBuf::from(
        memory_result.successes[0]
            .target
            .as_deref()
            .expect("memory target"),
    );
    let expected_resources_root = codex_home
        .path()
        .join("memories/extensions/external_agent_import/resources");
    assert_eq!(
        std::fs::canonicalize(&imported_resources_root)?,
        std::fs::canonicalize(expected_resources_root)?,
    );
    let imported_files = [
        imported_resources_root.join("project-a/MEMORY.md"),
        imported_resources_root.join("project-a/release-process.md"),
    ];
    assert_eq!(
        std::fs::read_to_string(&imported_files[0])?,
        "project A memory"
    );
    assert_eq!(
        std::fs::read_to_string(&imported_files[1])?,
        "project A release process"
    );
    let imported_scope: serde_json::Value = serde_json::from_slice(&std::fs::read(
        imported_resources_root.join("project-a/scope.json"),
    )?)?;
    assert_eq!(imported_scope, serde_json::json!({ "cwd": project_cwd }));
    let memory_root = codex_home.path().join("memories");
    let memory_diff = codex_git_utils::diff_since_latest_init(&memory_root).await?;
    for relative_path in [
        "extensions/external_agent_import/resources/project-a/MEMORY.md",
        "extensions/external_agent_import/resources/project-a/release-process.md",
        "extensions/external_agent_import/resources/project-a/scope.json",
    ] {
        assert!(
            memory_diff
                .changes
                .iter()
                .any(|change| change.path == relative_path)
        );
    }

    codex_memories_write::workspace::reset_memory_workspace_baseline(&memory_root).await?;
    std::fs::remove_dir_all(&source_project)?;
    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({ "includeHome": true })),
        )
        .await?;
    let mut detected: ExternalAgentConfigDetectResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    detected
        .items
        .retain(|item| item.item_type == ExternalAgentConfigMigrationItemType::Memory);
    assert_eq!(detected.items.len(), 1);
    assert_eq!(
        detected.items[0]
            .details
            .as_ref()
            .expect("memory details")
            .memory,
        vec!["project-a"]
    );

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({ "migrationItems": detected.items })),
        )
        .await?;
    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);
    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);
    assert_eq!(completed.item_type_results.len(), 1);
    assert_eq!(completed.item_type_results[0].failures, Vec::new());
    assert_eq!(completed.item_type_results[0].successes.len(), 1);
    assert_eq!(
        completed.item_type_results[0].successes[0]
            .source
            .as_deref(),
        Some("project-a")
    );
    assert!(!imported_resources_root.join("project-a").exists());

    let memory_diff = codex_git_utils::diff_since_latest_init(&memory_root).await?;
    assert_eq!(
        memory_diff
            .changes
            .iter()
            .map(|change| (change.status, change.path.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (
                codex_git_utils::GitBaselineChangeStatus::Deleted,
                "extensions/external_agent_import/resources/project-a/MEMORY.md",
            ),
            (
                codex_git_utils::GitBaselineChangeStatus::Deleted,
                "extensions/external_agent_import/resources/project-a/release-process.md",
            ),
            (
                codex_git_utils::GitBaselineChangeStatus::Deleted,
                "extensions/external_agent_import/resources/project-a/scope.json",
            ),
        ]
    );

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_reports_failed_sync_import_in_completion() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    let source_home = external_agent_home(codex_home.path());
    std::fs::create_dir_all(&source_home)?;
    std::fs::write(
        source_home.join("settings.json"),
        r#"{"env":{"FOO":"bar"}}"#,
    )?;
    std::fs::write(codex_home.path().join("config.toml"), "invalid = [")?;
    let home_dir = codex_home.path().display().to_string();
    let analytics_capture_file = codex_home.path().join("analytics-events.jsonl");
    let analytics_capture_file = analytics_capture_file.display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[
            ("HOME", Some(home_dir.as_str())),
            (
                "CODEX_ANALYTICS_EVENTS_CAPTURE_FILE",
                Some(analytics_capture_file.as_str()),
            ),
        ])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "source": "test_import",
                "providerId": "test-provider-42",
                "migrationItems": [
                    {
                        "itemType": "CONFIG",
                        "description": "Import config",
                        "cwd": null
                    },
                    {
                        "itemType": "COMMANDS",
                        "description": "Import commands",
                        "cwd": null
                    }
                ]
            })),
        )
        .await?;

    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);

    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);
    let config_result = completed
        .item_type_results
        .iter()
        .find(|result| result.item_type == ExternalAgentConfigMigrationItemType::Config)
        .expect("config result");
    assert!(config_result.successes.is_empty());
    assert_eq!(config_result.failures.len(), 1);
    let config_failure = &config_result.failures[0];
    assert_eq!(
        config_failure.error_type.as_deref(),
        Some("invalid_existing_config")
    );
    assert_eq!(config_failure.failure_stage, "import_request_failed");
    assert!(
        config_failure
            .message
            .contains("invalid existing config.toml"),
        "unexpected failure: {config_failure:?}"
    );
    let commands_result = completed
        .item_type_results
        .iter()
        .find(|result| result.item_type == ExternalAgentConfigMigrationItemType::Commands)
        .expect("commands result");
    assert!(commands_result.successes.is_empty());
    assert!(commands_result.failures.is_empty());

    let events = timeout(DEFAULT_TIMEOUT, async {
        loop {
            let contents = match std::fs::read_to_string(&analytics_capture_file) {
                Ok(contents) => contents,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    continue;
                }
                Err(err) => return Err(err.into()),
            };
            let mut captured_events = Vec::new();
            for line in contents.lines() {
                let payload: serde_json::Value = serde_json::from_str(line)?;
                let Some(events) = payload["events"].as_array() else {
                    continue;
                };
                captured_events.extend(events.iter().cloned());
            }
            if captured_events.iter().any(|event| {
                event["event_type"] == "codex_onboarding_external_agent_import_complete"
                    && event["event_params"]["type"] == "COMMANDS"
            }) {
                return Ok::<Vec<serde_json::Value>, anyhow::Error>(captured_events);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await??;
    let event = events
        .iter()
        .find(|event| {
            event["event_type"] == "codex_onboarding_external_agent_import_failure"
                && event["event_params"]["type"] == "CONFIG"
        })
        .expect("config failure analytics event");
    let event_params = &event["event_params"];
    assert_eq!(event_params["import_id"], import_id);
    assert_eq!(event_params["source"], "test_import");
    assert_eq!(event_params["provider_id"], "test-provider-42");
    assert_eq!(event_params["type"], "CONFIG");
    assert_eq!(event_params["failure_stage"], "import_request_failed");
    assert_eq!(event_params["error_type"], "invalid_existing_config");
    assert!(event_params.get("raw_errors").is_none());
    assert!(event_params.get("message").is_none());
    assert!(!events.iter().any(|event| {
        event["event_type"] == "codex_onboarding_external_agent_import_failure"
            && event["event_params"]["type"] == "COMMANDS"
    }));

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_completed_tracks_analytics_event() -> Result<()> {
    let analytics_server = start_analytics_events_server().await?;
    let codex_home = TempDir::new()?;
    write_analytics_config(codex_home.path(), &analytics_server.uri())?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let missing_session_path =
        external_agent_home(codex_home.path()).join("projects/repo/missing.jsonl");
    let project_root = codex_home.path().join("repo");
    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "source": "test_import",
                "providerId": "test-provider-42",
                "migrationSource": SECONDARY_MIGRATION_SOURCE,
                "migrationItems": [{
                    "itemType": "SESSIONS",
                    "description": "Migrate recent sessions",
                    "cwd": null,
                    "details": {
                        "sessions": [{
                            "path": missing_session_path,
                            "cwd": project_root,
                            "title": "missing session"
                        }]
                    }
                }]
            })),
        )
        .await?;
    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);

    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);
    assert_eq!(completed.item_type_results.len(), 1);
    assert_eq!(completed.item_type_results[0].successes.len(), 0);
    assert_eq!(completed.item_type_results[0].failures.len(), 1);
    assert_eq!(
        completed.item_type_results[0].failures[0]
            .sub_error_type
            .as_deref(),
        Some("session_not_detected")
    );

    let event = wait_for_analytics_event(
        &analytics_server,
        DEFAULT_TIMEOUT,
        "codex_onboarding_external_agent_import_complete",
    )
    .await?;
    let event_params = &event["event_params"];
    assert_eq!(event_params["import_id"], serde_json::json!(import_id));
    assert_eq!(event_params["source"], "test_import");
    assert_eq!(event_params["provider_id"], "test-provider-42");
    assert_eq!(event_params["type"], "SESSIONS");
    assert_eq!(event_params["success_count"], 0);
    assert_eq!(event_params["failed_count"], 1);
    assert!(event_params.get("raw_errors").is_none());

    let event = wait_for_analytics_event(
        &analytics_server,
        DEFAULT_TIMEOUT,
        "codex_onboarding_external_agent_import_failure",
    )
    .await?;
    let event_params = &event["event_params"];
    assert_eq!(event_params["import_id"], serde_json::json!(import_id));
    assert_eq!(event_params["source"], "test_import");
    assert_eq!(event_params["provider_id"], "test-provider-42");
    assert_eq!(event_params["type"], "SESSIONS");
    assert_eq!(event_params["failure_stage"], "session_missing");
    assert_eq!(event_params["error_type"], "session_missing");
    assert_eq!(event_params["sub_error_type"], "session_not_detected");
    assert!(event_params.get("raw_errors").is_none());
    assert!(event_params.get("message").is_none());

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_reinstalls_plugins_from_known_marketplaces() -> Result<()> {
    let codex_home = TempDir::new()?;
    let analytics_server = start_analytics_events_server().await?;
    write_analytics_config(codex_home.path(), &analytics_server.uri())?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    let marketplace_root = codex_home.path().join("marketplace");
    let plugin_root = marketplace_root.join("plugins").join("sample");
    std::fs::create_dir_all(marketplace_root.join(".agents/plugins"))?;
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::write(
        marketplace_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample",
      "source": {
        "source": "local",
        "path": "./plugins/sample"
      }
    }
  ]
}"#,
    )?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample","version":"0.1.0"}"#,
    )?;
    let source_home = external_agent_home(codex_home.path());
    std::fs::create_dir_all(source_home.join("plugins"))?;
    let settings = serde_json::json!({
        "enabledPlugins": {
            "missing@debug": true,
            "sample@debug": true,
        },
        "extraKnownMarketplaces": {
            "debug": {
                "source": {
                    "source": "file",
                    "path": marketplace_root.join(".agents/plugins/marketplace.json"),
                }
            }
        }
    });
    std::fs::write(
        source_home.join("settings.json"),
        serde_json::to_string_pretty(&settings)?,
    )?;
    std::fs::write(
        source_home.join("plugins/known_marketplaces.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "debug": {
                "source": {
                    "source": "file",
                    "path": marketplace_root.join(".agents/plugins/marketplace.json"),
                },
                "installLocation": marketplace_root,
                "lastUpdated": "2026-07-09T00:16:23.611Z",
            }
        }))?,
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({ "includeHome": true })),
        )
        .await?;
    let detected: ExternalAgentConfigDetectResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(detected.items.len(), 1);
    assert_eq!(
        detected.items[0].item_type,
        ExternalAgentConfigMigrationItemType::Plugins
    );
    assert_eq!(
        detected.items[0]
            .details
            .as_ref()
            .map(|details| details.plugins.clone()),
        Some(vec![codex_app_server_protocol::PluginsMigration {
            marketplace_name: "debug".to_string(),
            plugin_names: vec!["missing".to_string(), "sample".to_string()],
        }])
    );

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({ "migrationItems": detected.items })),
        )
        .await?;
    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    let import_id = assert_import_response(response);
    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);
    assert_eq!(completed.item_type_results.len(), 1);
    let plugin_result = &completed.item_type_results[0];
    assert_eq!(
        plugin_result.item_type,
        ExternalAgentConfigMigrationItemType::Plugins
    );
    assert_eq!(plugin_result.successes.len(), 1);
    assert_eq!(
        plugin_result.successes[0].source.as_deref(),
        Some("sample@debug")
    );
    assert_eq!(plugin_result.failures.len(), 1);
    assert_eq!(
        plugin_result.failures[0].source.as_deref(),
        Some("missing@debug")
    );
    assert_eq!(
        plugin_result.failures[0].error_type.as_deref(),
        Some("plugin_not_found")
    );
    assert_eq!(plugin_result.failures[0].failure_stage, "plugin_import");
    assert_eq!(
        plugin_result.failures[0].message,
        "plugin `missing` was not found in marketplace `debug`"
    );

    let event = wait_for_analytics_event(
        &analytics_server,
        DEFAULT_TIMEOUT,
        "codex_plugin_install_failed",
    )
    .await?;
    let event_params = &event["event_params"];
    assert_eq!(event_params["plugin_id"], "missing@debug");
    assert_eq!(event_params["plugin_name"], "missing");
    assert_eq!(event_params["marketplace_name"], "debug");
    assert_eq!(event_params["source"], "external_agent_migration");
    assert_eq!(event_params["error_type"], "plugin_not_found");

    let event = wait_for_analytics_event(
        &analytics_server,
        DEFAULT_TIMEOUT,
        "codex_onboarding_external_agent_import_failure",
    )
    .await?;
    let event_params = &event["event_params"];
    assert_eq!(event_params["type"], "PLUGINS");
    assert_eq!(event_params["failure_stage"], "plugin_import");
    assert_eq!(event_params["error_type"], "plugin_not_found");

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: None,
            marketplace_kinds: None,
            force_refetch: false,
        })
        .await?;
    let response: PluginListResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let plugin = response
        .marketplaces
        .iter()
        .find(|marketplace| marketplace.name == "debug")
        .and_then(|marketplace| {
            marketplace
                .plugins
                .iter()
                .find(|plugin| plugin.name == "sample")
        })
        .expect("expected imported plugin to be listed");
    assert!(plugin.installed);
    assert!(plugin.enabled);
    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_sends_completion_notification_after_pending_plugins_finish()
-> Result<()> {
    let codex_home = TempDir::new()?;
    let source_home = external_agent_home(codex_home.path());
    std::fs::create_dir_all(&source_home)?;
    // This test only needs a pending non-local plugin import. Use an invalid
    // source so the background completion path cannot make a real network clone.
    std::fs::write(
        source_home.join("settings.json"),
        r#"{
  "enabledPlugins": {
    "formatter@acme-tools": true
  },
  "extraKnownMarketplaces": {
    "acme-tools": {
      "source": "not a valid marketplace source"
    }
  }
}"#,
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "migrationItems": [{
                    "itemType": "PLUGINS",
                    "description": "Import plugins",
                    "cwd": null,
                    "details": {
                        "plugins": [{
                            "marketplaceName": "acme-tools",
                            "pluginNames": ["formatter"]
                        }]
                    }
                }]
            })),
        )
        .await?;

    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);
    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_creates_session_rollouts() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("follow-up answer").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let project_root = codex_home.path().join("repo");
    let recent_timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let session_dir = external_agent_home(codex_home.path()).join("projects/repo");
    let session_path = session_dir.join("session.jsonl");
    let manifest_dir = connector_metadata_root(codex_home.path())
        .join("claude-code-sessions/account/organization");
    let control_request = "<ide_selection>src/auth.rs:1-5</ide_selection>";
    let first_request = "Fix auth flow";
    std::fs::create_dir_all(&project_root)?;
    std::fs::create_dir_all(&session_dir)?;
    std::fs::create_dir_all(&manifest_dir)?;
    std::fs::write(
        manifest_dir.join("session.json"),
        serde_json::json!({
            "cliSessionId": "session",
            "remoteMcpServersConfig": [
                { "name": "Gmail", "uuid": "gmail-server" },
                { "name": "Slack", "uuid": "slack-server" },
            ],
        })
        .to_string(),
    )?;
    std::fs::write(
        &session_path,
        [
            serde_json::json!({
                "type": "user",
                "cwd": &project_root,
                "timestamp": &recent_timestamp,
                "message": { "content": control_request },
            })
            .to_string(),
            serde_json::json!({
                "type": "user",
                "cwd": &project_root,
                "timestamp": &recent_timestamp,
                "message": { "content": first_request },
            })
            .to_string(),
            serde_json::json!({
                "type": "assistant",
                "cwd": &project_root,
                "timestamp": &recent_timestamp,
                "attributionMcpServer": "gmail-server",
                "message": { "content": "first answer" },
            })
            .to_string(),
        ]
        .join("\n"),
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({
                "includeHome": true,
            })),
        )
        .await?;
    let detected: ExternalAgentConfigDetectResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(detected.items.len(), 1);
    assert_eq!(
        detected.items[0]
            .details
            .as_ref()
            .and_then(|details| details.sessions.first())
            .and_then(|session| session.title.as_deref()),
        Some("Fix auth flow")
    );

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({ "migrationItems": detected.items })),
        )
        .await?;
    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);
    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);
    assert_eq!(completed.item_type_results.len(), 1);
    let session_result = &completed.item_type_results[0];
    assert_eq!(
        session_result.item_type,
        ExternalAgentConfigMigrationItemType::Sessions
    );
    assert_eq!(session_result.failures, Vec::new());
    assert_eq!(session_result.successes.len(), 1);
    let session_success = &session_result.successes[0];
    assert_eq!(
        session_success.item_type,
        ExternalAgentConfigMigrationItemType::Sessions
    );
    assert_eq!(session_success.cwd, None);
    let session_source = std::fs::canonicalize(&session_path)?.display().to_string();
    assert_eq!(
        session_success.source.as_deref(),
        Some(session_source.as_str())
    );
    let imported_thread_id = session_success
        .target
        .as_deref()
        .expect("session success should include imported thread id")
        .to_string();

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import/readHistories",
            /*params*/ None,
        )
        .await?;
    let response: ExternalAgentConfigImportHistoriesReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(
        response.connectors,
        vec![ExternalAgentImportedConnectorCandidate {
            name: "Gmail".to_string(),
            session_count: 1,
            source: ExternalAgentImportedConnectorSource::RemoteMcpServersConfig,
        }]
    );

    let request_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: None,
            sort_key: None,
            sort_direction: None,
            model_providers: None,
            source_kinds: None,
            archived: None,
            cwd: None,
            use_state_db_only: false,
            search_term: None,
            parent_thread_id: None,
            ancestor_thread_id: None,
        })
        .await?;
    let response: ThreadListResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let thread = response
        .data
        .first()
        .expect("expected imported thread")
        .clone();
    assert_eq!(imported_thread_id, thread.id.to_string());
    assert_eq!(thread.preview, control_request);
    assert_eq!(thread.name.as_deref(), Some("Fix auth flow"));

    let request_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id.clone(),
            include_turns: true,
        })
        .await?;
    let response: ThreadReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(response.thread.turns.len(), 2);
    let control_items = &response.thread.turns[0].items;
    assert_eq!(control_items.len(), 1);
    match &control_items[0] {
        ThreadItem::UserMessage { content, .. } => {
            assert_eq!(
                content,
                &vec![UserInput::Text {
                    text: control_request.to_string(),
                    text_elements: Vec::new(),
                }]
            );
        }
        other => panic!("expected user message item, got {other:?}"),
    }
    let imported_items = &response.thread.turns[1].items;
    assert_eq!(imported_items.len(), 3);
    match &imported_items[0] {
        ThreadItem::UserMessage { content, .. } => {
            assert_eq!(
                content,
                &vec![UserInput::Text {
                    text: first_request.to_string(),
                    text_elements: Vec::new(),
                }]
            );
        }
        other => panic!("expected user message item, got {other:?}"),
    }
    assert_eq!(
        imported_items.last(),
        Some(&ThreadItem::AgentMessage {
            id: "item-4".into(),
            text: "<EXTERNAL SESSION IMPORTED>".into(),
            phase: None,
            memory_citation: None,
        })
    );

    let request_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let _: ThreadResumeResponse = timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    let request_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "follow up".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id,
            include_turns: true,
        })
        .await?;
    let response: ThreadReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(response.thread.turns.len(), 3);
    match &response.thread.turns[2].items[1] {
        ThreadItem::AgentMessage { text, .. } => assert_eq!(text, "follow-up answer"),
        other => panic!("expected agent message item, got {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_does_not_initialize_required_mcp() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("unused").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let mut config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    config.push_str(
        r#"
[mcp_servers.required_broken]
command = "this-command-does-not-exist"
required = true
"#,
    );
    std::fs::write(codex_home.path().join("config.toml"), config)?;
    let project_root = codex_home.path().join("repo");
    let recent_timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let session_dir = external_agent_home(codex_home.path()).join("projects/repo");
    let session_path = session_dir.join("session.jsonl");
    std::fs::create_dir_all(&project_root)?;
    std::fs::create_dir_all(&session_dir)?;
    std::fs::write(
        &session_path,
        serde_json::json!({
            "type": "user",
            "cwd": &project_root,
            "timestamp": &recent_timestamp,
            "message": { "content": "first request" },
        })
        .to_string(),
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "migrationItems": [{
                    "itemType": "SESSIONS",
                    "description": "Migrate recent sessions",
                    "cwd": null,
                    "details": {
                        "sessions": [{
                            "path": session_path,
                            "cwd": project_root,
                            "title": "first request"
                        }]
                    }
                }]
            })),
        )
        .await?;
    let _: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;

    let request_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: None,
            sort_key: None,
            sort_direction: None,
            model_providers: None,
            source_kinds: None,
            archived: None,
            cwd: None,
            use_state_db_only: false,
            search_term: None,
            parent_thread_id: None,
            ancestor_thread_id: None,
        })
        .await?;
    let response: ThreadListResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(response.data.len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_agent_config_import_accepts_detected_session_payload_after_restart() -> Result<()>
{
    let server = create_mock_responses_server_repeating_assistant("unused").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let project_root = codex_home.path().join("repo");
    let recent_timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let session_dir = external_agent_home(codex_home.path()).join("projects/repo");
    let session_path = session_dir.join("session.jsonl");
    std::fs::create_dir_all(&project_root)?;
    std::fs::create_dir_all(&session_dir)?;
    std::fs::write(
        &session_path,
        serde_json::json!({
            "type": "user",
            "cwd": &project_root,
            "timestamp": &recent_timestamp,
            "message": { "content": "first request" },
        })
        .to_string(),
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "migrationItems": [{
                    "itemType": "SESSIONS",
                    "description": "Migrate recent sessions",
                    "cwd": null,
                    "details": {
                        "sessions": [{
                            "path": session_path,
                            "cwd": project_root,
                            "title": "first request"
                        }]
                    }
                }]
            })),
        )
        .await?;
    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);
    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);

    let request_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: None,
            sort_key: None,
            sort_direction: None,
            model_providers: None,
            source_kinds: None,
            archived: None,
            cwd: None,
            use_state_db_only: false,
            search_term: None,
            parent_thread_id: None,
            ancestor_thread_id: None,
        })
        .await?;
    let response: ThreadListResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(response.data.len(), 1);

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_skips_already_imported_session_versions() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("unused").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let project_root = codex_home.path().join("repo");
    let recent_timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let session_dir = external_agent_home(codex_home.path()).join("projects/repo");
    let session_path = session_dir.join("session.jsonl");
    std::fs::create_dir_all(&project_root)?;
    std::fs::create_dir_all(&session_dir)?;
    std::fs::write(
        &session_path,
        serde_json::json!({
            "type": "user",
            "cwd": &project_root,
            "timestamp": &recent_timestamp,
            "message": { "content": "first request" },
        })
        .to_string(),
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({ "includeHome": true })),
        )
        .await?;
    let detected: ExternalAgentConfigDetectResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    for _ in 0..2 {
        let request_id = mcp
            .send_raw_request(
                "externalAgentConfig/import",
                Some(serde_json::json!({ "migrationItems": detected.items.clone() })),
            )
            .await?;
        let response: ExternalAgentConfigImportResponse =
            timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
        let import_id = assert_import_response(response);
        let completed: ExternalAgentConfigImportCompletedNotification = timeout(
            DEFAULT_TIMEOUT,
            mcp.read_notification("externalAgentConfig/import/completed"),
        )
        .await??;
        assert_eq!(completed.import_id, import_id);
    }

    let request_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: None,
            sort_key: None,
            sort_direction: None,
            model_providers: None,
            source_kinds: None,
            archived: None,
            cwd: None,
            use_state_db_only: false,
            search_term: None,
            parent_thread_id: None,
            ancestor_thread_id: None,
        })
        .await?;
    let response: ThreadListResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(response.data.len(), 1);

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_agent_config_import_returns_before_background_session_import_finishes()
-> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("unused").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let project_root = codex_home.path().join("repo");
    let recent_timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let session_dir = external_agent_home(codex_home.path()).join("projects/repo");
    let session_path = session_dir.join("session.jsonl");
    std::fs::create_dir_all(&project_root)?;
    std::fs::create_dir_all(&session_dir)?;
    let session_contents = serde_json::json!({
        "type": "user",
        "cwd": &project_root,
        "timestamp": &recent_timestamp,
        "message": { "content": "first request" },
    })
    .to_string();
    std::fs::write(&session_path, &session_contents)?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({ "includeHome": true })),
        )
        .await?;
    let detected: ExternalAgentConfigDetectResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(detected.items.len(), 1);
    let detected_items = detected.items;

    std::fs::remove_file(&session_path)?;
    let status = std::process::Command::new("mkfifo")
        .arg(&session_path)
        .status()?;
    assert!(status.success());

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({ "migrationItems": detected_items.clone() })),
        )
        .await?;
    let response: ExternalAgentConfigImportResponse =
        timeout(Duration::from_secs(5), mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);

    assert!(
        timeout(
            Duration::from_millis(200),
            mcp.read_stream_until_notification_message("externalAgentConfig/import/completed")
        )
        .await
        .is_err(),
        "session import completed before the blocked background import was unblocked"
    );

    let duplicate_request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({ "migrationItems": detected_items })),
        )
        .await?;
    let response: ExternalAgentConfigImportResponse = timeout(
        Duration::from_secs(5),
        mcp.read_response(duplicate_request_id),
    )
    .await??;
    let duplicate_import_id = assert_import_response(response);

    let mut completed_import_ids = Vec::new();
    for _ in 0..2 {
        timeout(DEFAULT_TIMEOUT, async {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .open(&session_path)
                .await?;
            file.write_all(session_contents.as_bytes()).await
        })
        .await??;

        let completed: ExternalAgentConfigImportCompletedNotification = timeout(
            DEFAULT_TIMEOUT,
            mcp.read_notification("externalAgentConfig/import/completed"),
        )
        .await??;
        completed_import_ids.push(completed.import_id);
    }
    completed_import_ids.sort();
    let mut expected_import_ids = vec![import_id, duplicate_import_id];
    expected_import_ids.sort();
    assert_eq!(completed_import_ids, expected_import_ids);

    let request_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: None,
            sort_key: None,
            sort_direction: None,
            model_providers: None,
            source_kinds: None,
            archived: None,
            cwd: None,
            use_state_db_only: false,
            search_term: None,
            parent_thread_id: None,
            ancestor_thread_id: None,
        })
        .await?;
    let response: ThreadListResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(response.data.len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_agent_config_import_compacts_huge_session_before_first_follow_up() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_log = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_assistant_message("m1", "LOCAL_SUMMARY"),
                responses::ev_completed_with_tokens("r1", /*total_tokens*/ 120),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m2", "follow-up answer"),
                responses::ev_completed_with_tokens("r2", /*total_tokens*/ 80),
            ]),
        ],
    )
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_root_config(
            "compact_prompt = \"Summarize the conversation.\"\nmodel_auto_compact_token_limit = 200",
        )
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;

    let project_root = codex_home.path().join("repo");
    let recent_timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let session_dir = external_agent_home(codex_home.path()).join("projects/repo");
    let session_path = session_dir.join("session.jsonl");
    std::fs::create_dir_all(&project_root)?;
    std::fs::create_dir_all(&session_dir)?;
    let huge_user = "u".repeat(20_000);
    let huge_assistant = "a".repeat(20_000);
    std::fs::write(
        &session_path,
        [
            serde_json::json!({
                "type": "user",
                "cwd": &project_root,
                "timestamp": &recent_timestamp,
                "message": { "content": &huge_user },
            })
            .to_string(),
            serde_json::json!({
                "type": "assistant",
                "cwd": &project_root,
                "timestamp": &recent_timestamp,
                "message": { "content": &huge_assistant },
            })
            .to_string(),
        ]
        .join("\n"),
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({
                "includeHome": true,
            })),
        )
        .await?;
    let detected: ExternalAgentConfigDetectResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(detected.items.len(), 1);

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({ "migrationItems": detected.items })),
        )
        .await?;
    let response: ExternalAgentConfigImportResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let import_id = assert_import_response(response);
    let completed: ExternalAgentConfigImportCompletedNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_notification("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(completed.import_id, import_id);

    let request_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: None,
            sort_key: None,
            sort_direction: None,
            model_providers: None,
            source_kinds: None,
            archived: None,
            cwd: None,
            use_state_db_only: false,
            search_term: None,
            parent_thread_id: None,
            ancestor_thread_id: None,
        })
        .await?;
    let response: ThreadListResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let thread = response
        .data
        .first()
        .expect("expected imported thread")
        .clone();

    let request_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let _: ThreadResumeResponse = timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    let request_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "follow up".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = response_log.requests();
    assert_eq!(requests.len(), 2);
    let first = requests[0].body_json().to_string();
    let second = requests[1].body_json().to_string();
    assert!(first.contains("Summarize the conversation."));
    assert!(!first.contains("follow up"));
    assert!(second.contains("follow up"));
    assert!(second.contains("LOCAL_SUMMARY"));
    Ok(())
}

fn write_analytics_config(codex_home: &std::path::Path, base_url: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!("chatgpt_base_url = \"{base_url}\"\n"),
    )
}
