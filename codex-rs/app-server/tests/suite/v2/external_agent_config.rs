use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server::INVALID_PARAMS_ERROR_CODE;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::JSONRPCError;
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
use core_test_support::responses;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tempfile::TempDir;
#[cfg(unix)]
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

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
    std::fs::create_dir_all(codex_home.path().join(".claude"))?;
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
        codex_home.path().join(".claude").join("settings.json"),
        serde_json::to_string_pretty(&settings)?,
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp =
        McpProcess::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))]).await?;
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

    assert_eq!(response, ExternalAgentConfigImportResponse {});
    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");

    let request_id = mcp
        .send_plugin_list_request(PluginListParams { cwds: None })
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
    std::fs::create_dir_all(codex_home.path().join(".claude"))?;
    // This test only needs a pending non-local plugin import. Use an invalid
    // source so the background completion path cannot make a real network clone.
    std::fs::write(
        codex_home.path().join(".claude").join("settings.json"),
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
        McpProcess::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))]).await?;
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
    assert_eq!(response, ExternalAgentConfigImportResponse {});
    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");

    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_creates_session_rollouts() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("follow-up answer").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    let project_root = codex_home.path().join("repo");
    let recent_timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let session_dir = codex_home.path().join(".claude/projects/repo");
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
        McpProcess::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))]).await?;
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
    assert_eq!(response, ExternalAgentConfigImportResponse {});
    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_agent_config_import_accepts_detected_session_payload_after_restart() -> Result<()>
{
    let server = create_mock_responses_server_repeating_assistant("unused").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    let project_root = codex_home.path().join("repo");
    let recent_timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let session_dir = codex_home.path().join(".claude/projects/repo");
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
        McpProcess::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))]).await?;
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
    assert_eq!(response, ExternalAgentConfigImportResponse {});
    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");

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
    let session_dir = codex_home.path().join(".claude/projects/repo");
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
        McpProcess::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))]).await?;
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
        let _: ExternalAgentConfigImportResponse = to_response(response)?;
        let notification = timeout(
            DEFAULT_TIMEOUT,
            mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
        )
        .await??;
        assert_eq!(notification.method, "externalAgentConfig/import/completed");
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
    let session_dir = codex_home.path().join(".claude/projects/repo");
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

    let project_config_dir = project_root.join(".codex");
    std::fs::create_dir_all(&project_config_dir)?;
    let project_config = project_config_dir.join("config.toml");
    let status = std::process::Command::new("mkfifo")
        .arg(&project_config)
        .status()?;
    assert!(status.success());

    let home_dir = codex_home.path().display().to_string();
    let mut mcp =
        McpProcess::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))]).await?;
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
    assert_eq!(response, ExternalAgentConfigImportResponse {});

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
    assert_eq!(response, ExternalAgentConfigImportResponse {});

    let writer = tokio::spawn(async move {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&project_config)
            .await?;
        file.write_all(b"\n").await
    });
    timeout(DEFAULT_TIMEOUT, writer).await???;

    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");

    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");

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
async fn external_agent_config_import_rejects_undetected_session_paths() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("unused").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    let project_root = codex_home.path().join("repo");
    let recent_timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let session_dir = codex_home.path().join(".claude/projects/repo");
    let detected_session_path = session_dir.join("detected.jsonl");
    let undetected_session_path = codex_home.path().join("outside.jsonl");
    std::fs::create_dir_all(&project_root)?;
    std::fs::create_dir_all(&session_dir)?;
    for path in [&detected_session_path, &undetected_session_path] {
        std::fs::write(
            path,
            format!(
                r#"{{"type":"user","cwd":"{}","timestamp":"{}","message":{{"content":"first request"}}}}"#,
                project_root.display(),
                recent_timestamp
            ),
        )?;
    }

    let home_dir = codex_home.path().display().to_string();
    let mut mcp =
        McpProcess::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))]).await?;
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
                            "path": undetected_session_path,
                            "cwd": project_root,
                            "title": "first request"
                        }]
                    }
                }]
            })),
        )
        .await?;
    let err: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(err.error.code, INVALID_PARAMS_ERROR_CODE);
    assert!(
        err.error
            .message
            .contains("external agent session was not detected for import")
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
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ThreadListResponse = to_response(response)?;
    assert_eq!(response.data, Vec::new());

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
    let session_dir = codex_home.path().join(".claude/projects/repo");
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
        McpProcess::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))]).await?;
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
    let _: ExternalAgentConfigImportResponse = to_response(response)?;
    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("externalAgentConfig/import/completed"),
    )
    .await??;
    assert_eq!(notification.method, "externalAgentConfig/import/completed");

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
