use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::start_analytics_events_server;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportCompletedNotification;
use codex_app_server_protocol::ExternalAgentConfigImportHistoriesReadResponse;
use codex_app_server_protocol::ExternalAgentConfigImportProgressNotification;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::JSONRPCResponse;
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
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
#[cfg(unix)]
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

use super::analytics::wait_for_analytics_event;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

fn external_agent_home(codex_home: &Path) -> PathBuf {
    codex_home.join(concat!(".", "cl", "aude"))
}

fn assert_import_response(response: ExternalAgentConfigImportResponse) -> String {
    assert!(!response.import_id.is_empty());
    response.import_id
}

#[tokio::test]
async fn external_agent_config_import_sends_completion_notification_for_sync_only_import()
-> Result<()> {
    let codex_home = TempDir::new()?;
    let sqlite_home = TempDir::new()?;
    let home_dir = codex_home.path().display().to_string();
    let sqlite_home_dir = sqlite_home.path().display().to_string();
    let mut mcp = TestAppServer::new_with_env(
        codex_home.path(),
        &[
            ("HOME", Some(home_dir.as_str())),
            ("CODEX_SQLITE_HOME", Some(sqlite_home_dir.as_str())),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

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

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportResponse = to_response(response)?;
    let import_id = assert_import_response(response);
    let progress = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/progress"),
    )
    .await??;
    assert_eq!(progress.method, "externalAgentConfig/import/progress");
    let progress: ExternalAgentConfigImportProgressNotification =
        serde_json::from_value(progress.params.expect("progress params"))?;
    assert_eq!(progress.import_id, import_id);
    assert_eq!(progress.item_type_results.len(), 1);
    assert_eq!(
        progress.item_type_results[0].item_type,
        ExternalAgentConfigMigrationItemType::Config
    );

    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");
    let completed: ExternalAgentConfigImportCompletedNotification =
        serde_json::from_value(notification.params.expect("completed params"))?;
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
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportHistoriesReadResponse = to_response(response)?;
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
    let mut mcp = TestAppServer::new_with_env(
        codex_home.path(),
        &[
            ("HOME", Some(home_dir.as_str())),
            (
                "CODEX_ANALYTICS_EVENTS_CAPTURE_FILE",
                Some(analytics_capture_file.as_str()),
            ),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "source": "test_import",
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

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportResponse = to_response(response)?;
    let import_id = assert_import_response(response);

    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    let completed: ExternalAgentConfigImportCompletedNotification =
        serde_json::from_value(notification.params.expect("completed params"))?;
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
    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))])
            .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "source": "test_import",
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
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportResponse = to_response(response)?;
    let import_id = assert_import_response(response);

    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    let completed: ExternalAgentConfigImportCompletedNotification =
        serde_json::from_value(notification.params.expect("completed params"))?;
    assert_eq!(completed.import_id, import_id);
    assert_eq!(completed.item_type_results.len(), 1);
    assert_eq!(completed.item_type_results[0].successes.len(), 0);
    assert_eq!(completed.item_type_results[0].failures.len(), 1);

    let event = wait_for_analytics_event(
        &analytics_server,
        DEFAULT_TIMEOUT,
        "codex_onboarding_external_agent_import_complete",
    )
    .await?;
    let event_params = &event["event_params"];
    assert_eq!(event_params["import_id"], serde_json::json!(import_id));
    assert_eq!(event_params["source"], "test_import");
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
    assert_eq!(event_params["type"], "SESSIONS");
    assert_eq!(event_params["failure_stage"], "session_missing");
    assert_eq!(event_params["error_type"], "session_missing");
    assert!(event_params.get("raw_errors").is_none());
    assert!(event_params.get("message").is_none());

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_sends_completion_notification_for_local_plugins() -> Result<()>
{
    let codex_home = TempDir::new()?;
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
    std::fs::create_dir_all(&source_home)?;
    let settings = serde_json::json!({
        "enabledPlugins": {
            "sample@debug": true
        },
        "extraKnownMarketplaces": {
            "debug": {
                "source": "local",
                "path": marketplace_root,
            }
        }
    });
    std::fs::write(
        source_home.join("settings.json"),
        serde_json::to_string_pretty(&settings)?,
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))])
            .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

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
                            "marketplaceName": "debug",
                            "pluginNames": ["sample"]
                        }]
                    }
                }]
            })),
        )
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportResponse = to_response(response)?;

    let import_id = assert_import_response(response);
    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");
    let completed: ExternalAgentConfigImportCompletedNotification =
        serde_json::from_value(notification.params.expect("completed params"))?;
    assert_eq!(completed.import_id, import_id);

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: None,
            marketplace_kinds: None,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;
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
    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))])
            .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

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

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportResponse = to_response(response)?;
    let import_id = assert_import_response(response);
    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");
    let completed: ExternalAgentConfigImportCompletedNotification =
        serde_json::from_value(notification.params.expect("completed params"))?;
    assert_eq!(completed.import_id, import_id);

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_creates_session_rollouts() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("follow-up answer").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    let project_root = codex_home.path().join("repo");
    let recent_timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let session_dir = external_agent_home(codex_home.path()).join("projects/repo");
    let session_path = session_dir.join("session.jsonl");
    std::fs::create_dir_all(&project_root)?;
    std::fs::create_dir_all(&session_dir)?;
    std::fs::write(
        &session_path,
        [
            serde_json::json!({
                "type": "user",
                "cwd": &project_root,
                "timestamp": &recent_timestamp,
                "message": { "content": "first request" },
            })
            .to_string(),
            serde_json::json!({
                "type": "assistant",
                "cwd": &project_root,
                "timestamp": &recent_timestamp,
                "message": { "content": "first answer" },
            })
            .to_string(),
            serde_json::json!({
                "type": "custom-title",
                "customTitle": "source session title",
            })
            .to_string(),
        ]
        .join("\n"),
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))])
            .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({
                "includeHome": true,
            })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let detected: ExternalAgentConfigDetectResponse = to_response(response)?;
    assert_eq!(detected.items.len(), 1);

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({ "migrationItems": detected.items })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportResponse = to_response(response)?;
    let import_id = assert_import_response(response);
    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");
    let completed: ExternalAgentConfigImportCompletedNotification =
        serde_json::from_value(notification.params.expect("completed params"))?;
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
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ThreadListResponse = to_response(response)?;
    let thread = response
        .data
        .first()
        .expect("expected imported thread")
        .clone();
    assert_eq!(imported_thread_id, thread.id.to_string());
    assert_eq!(thread.preview, "first request");
    assert_eq!(thread.name.as_deref(), Some("source session title"));

    let request_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id.clone(),
            include_turns: true,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ThreadReadResponse = to_response(response)?;
    assert_eq!(response.thread.turns.len(), 1);
    let items = &response.thread.turns[0].items;
    assert_eq!(items.len(), 3);
    assert_eq!(
        items.last(),
        Some(&ThreadItem::AgentMessage {
            id: "item-3".into(),
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
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: ThreadResumeResponse = to_response(response)?;

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
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
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
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ThreadReadResponse = to_response(response)?;
    assert_eq!(response.thread.turns.len(), 2);
    match &response.thread.turns[1].items[1] {
        ThreadItem::AgentMessage { text, .. } => assert_eq!(text, "follow-up answer"),
        other => panic!("expected agent message item, got {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_does_not_initialize_required_mcp() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("unused").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
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
    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))])
            .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

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
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
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
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ThreadListResponse = to_response(response)?;
    assert_eq!(response.data.len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_agent_config_import_accepts_detected_session_payload_after_restart() -> Result<()>
{
    let server = create_mock_responses_server_repeating_assistant("unused").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
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
    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))])
            .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

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
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportResponse = to_response(response)?;
    let import_id = assert_import_response(response);
    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");
    let completed: ExternalAgentConfigImportCompletedNotification =
        serde_json::from_value(notification.params.expect("completed params"))?;
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
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ThreadListResponse = to_response(response)?;
    assert_eq!(response.data.len(), 1);

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_skips_already_imported_session_versions() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("unused").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
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
    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))])
            .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({ "includeHome": true })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let detected: ExternalAgentConfigDetectResponse = to_response(response)?;

    for _ in 0..2 {
        let request_id = mcp
            .send_raw_request(
                "externalAgentConfig/import",
                Some(serde_json::json!({ "migrationItems": detected.items.clone() })),
            )
            .await?;
        let response: JSONRPCResponse = timeout(
            DEFAULT_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
        )
        .await??;
        let response: ExternalAgentConfigImportResponse = to_response(response)?;
        let import_id = assert_import_response(response);
        let notification = timeout(
            DEFAULT_TIMEOUT,
            mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
        )
        .await??;
        assert_eq!(notification.method, "externalAgentConfig/import/completed");
        let completed: ExternalAgentConfigImportCompletedNotification =
            serde_json::from_value(notification.params.expect("completed params"))?;
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
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ThreadListResponse = to_response(response)?;
    assert_eq!(response.data.len(), 1);

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_agent_config_import_returns_before_background_session_import_finishes()
-> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("unused").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
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
    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))])
            .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({ "includeHome": true })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let detected: ExternalAgentConfigDetectResponse = to_response(response)?;
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
    let response: JSONRPCResponse = timeout(
        Duration::from_secs(5),
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportResponse = to_response(response)?;
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
    let response: JSONRPCResponse = timeout(
        Duration::from_secs(5),
        mcp.read_stream_until_response_message(RequestId::Integer(duplicate_request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportResponse = to_response(response)?;
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

        let notification = timeout(
            DEFAULT_TIMEOUT,
            mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
        )
        .await??;
        assert_eq!(notification.method, "externalAgentConfig/import/completed");
        let completed: ExternalAgentConfigImportCompletedNotification =
            serde_json::from_value(notification.params.expect("completed params"))?;
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
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ThreadListResponse = to_response(response)?;
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
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::default(),
        /*auto_compact_limit*/ 200,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "Summarize the conversation.",
    )?;

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
    let mut mcp =
        TestAppServer::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))])
            .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({
                "includeHome": true,
            })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let detected: ExternalAgentConfigDetectResponse = to_response(response)?;
    assert_eq!(detected.items.len(), 1);

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({ "migrationItems": detected.items })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportResponse = to_response(response)?;
    let import_id = assert_import_response(response);
    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");
    let completed: ExternalAgentConfigImportCompletedNotification =
        serde_json::from_value(notification.params.expect("completed params"))?;
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
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ThreadListResponse = to_response(response)?;
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
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: ThreadResumeResponse = to_response(response)?;

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
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
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

fn create_config_toml(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}

fn write_analytics_config(codex_home: &std::path::Path, base_url: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!("chatgpt_base_url = \"{base_url}\"\n"),
    )
}
