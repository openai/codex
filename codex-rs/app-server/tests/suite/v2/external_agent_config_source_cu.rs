use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn imports_cursor_config_plugins_hooks_sessions_and_capitalized_text() -> Result<()> {
    let root = TempDir::new()?;
    let home = root.path().join("home");
    let codex_home = home.join(".codex");
    let source_dir = home.join(".cursor");
    let project_root = root.path().join("workspace-with-dashes");
    let writable_root = root.path().join("shared-cache");
    std::fs::create_dir_all(&codex_home)?;
    std::fs::create_dir_all(&source_dir)?;
    std::fs::create_dir_all(&project_root)?;
    std::fs::create_dir_all(project_root.join(".git"))?;
    std::fs::create_dir_all(&writable_root)?;

    std::fs::write(
        source_dir.join("cli-config.json"),
        r#"{"permissions":{"allow":["Shell(git)"]}}"#,
    )?;
    std::fs::write(
        source_dir.join("sandbox.json"),
        serde_json::to_string(&serde_json::json!({
            "type": "workspace_readwrite",
            "additionalReadwritePaths": [writable_root],
            "disableTmpWrite": true,
            "networkPolicy": {"default": "allow"}
        }))?,
    )?;

    std::fs::create_dir_all(source_dir.join("skills/release"))?;
    std::fs::write(
        source_dir.join("skills/release/SKILL.md"),
        concat!(
            "---\nname: release\ndescription: Release with Cursor\n---\n\n",
            "Use Cursor to release. Keep the database cursor and CURSOR acronym unchanged.\n"
        ),
    )?;

    std::fs::create_dir_all(source_dir.join("hooks"))?;
    std::fs::write(
        source_dir.join("hooks/session-start.sh"),
        "#!/bin/sh\nexit 0\n",
    )?;
    std::fs::write(
        source_dir.join("hooks.json"),
        r#"{
          "version": 1,
          "hooks": {
            "sessionStart": [{
              "command": "sh .cursor/hooks/session-start.sh",
              "statusMessage": "Starting Cursor context"
            }],
            "preToolUse": [{
              "command": "sh .cursor/hooks/session-start.sh",
              "matcher": "Shell"
            }],
            "afterAgentResponse": [{"command": "echo unsupported"}]
          }
        }"#,
    )?;

    let marketplace_root = source_dir.join("plugins/marketplaces/team-tools");
    let plugin_root = marketplace_root.join("release-helper");
    std::fs::create_dir_all(marketplace_root.join(".cursor-plugin"))?;
    std::fs::create_dir_all(plugin_root.join(".cursor-plugin"))?;
    std::fs::create_dir_all(plugin_root.join("skills/plugin-release"))?;
    std::fs::write(
        marketplace_root.join(".cursor-plugin/marketplace.json"),
        r#"{
          "name": "team-tools",
          "plugins": [{"name": "release-helper", "source": "release-helper"}]
        }"#,
    )?;
    std::fs::write(
        plugin_root.join(".cursor-plugin/plugin.json"),
        r#"{
          "name": "release-helper",
          "version": "1.0.0",
          "skills": "skills"
        }"#,
    )?;
    std::fs::write(
        plugin_root.join("skills/plugin-release/SKILL.md"),
        "---\nname: plugin-release\ndescription: Release helper\n---\n",
    )?;
    std::fs::create_dir_all(source_dir.join("plugins/cache/team-tools/release-helper/revision"))?;

    let session_id = "11111111-1111-1111-1111-111111111111";
    let session_path = source_dir
        .join("projects")
        .join(encoded_cursor_project_path(&project_root))
        .join("agent-transcripts")
        .join(session_id)
        .join(format!("{session_id}.jsonl"));
    std::fs::create_dir_all(session_path.parent().expect("session parent"))?;
    std::fs::write(
        &session_path,
        concat!(
            "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"review the release\"}]}}\n",
            "{\"role\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"release looks good\"}]}}\n"
        ),
    )?;

    let home_dir = home.display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(&codex_home)
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build()
        .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({
                "includeHome": true,
                "cwds": [project_root],
                "source": "cursor"
            })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let detected: ExternalAgentConfigDetectResponse = to_response(response)?;
    let detected_types = detected
        .items
        .iter()
        .map(|item| item.item_type)
        .collect::<Vec<_>>();
    for expected in [
        ExternalAgentConfigMigrationItemType::Config,
        ExternalAgentConfigMigrationItemType::Hooks,
        ExternalAgentConfigMigrationItemType::Skills,
        ExternalAgentConfigMigrationItemType::Plugins,
        ExternalAgentConfigMigrationItemType::Sessions,
    ] {
        assert!(detected_types.contains(&expected), "missing {expected:?}");
    }
    assert_eq!(
        detected_types
            .iter()
            .filter(|item_type| **item_type == ExternalAgentConfigMigrationItemType::Plugins)
            .count(),
        1
    );

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "migrationItems": detected.items,
                "source": "cursor"
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
    assert!(
        completed
            .item_type_results
            .iter()
            .all(|result| result.failures.is_empty()),
        "{:#?}",
        completed.item_type_results
    );
    let session_result = completed
        .item_type_results
        .iter()
        .find(|result| result.item_type == ExternalAgentConfigMigrationItemType::Sessions)
        .expect("session result");
    assert_eq!(session_result.successes.len(), 1);

    let config = std::fs::read_to_string(codex_home.join("config.toml"))?;
    assert!(config.contains("sandbox_mode = \"workspace-write\""));
    assert!(config.contains("network_access = true"));
    assert!(config.contains("exclude_slash_tmp = true"));
    assert!(config.contains(writable_root.to_string_lossy().as_ref()));
    assert!(config.contains(r#"[plugins."release-helper@team-tools"]"#));

    let skill = std::fs::read_to_string(home.join(".agents/skills/release/SKILL.md"))?;
    assert!(skill.contains("Release with Codex"));
    assert!(skill.contains("Use Codex to release"));
    assert!(skill.contains("database cursor"));
    assert!(skill.contains("CURSOR acronym"));

    let hooks = std::fs::read_to_string(codex_home.join("hooks.json"))?;
    assert!(hooks.contains("SessionStart"));
    assert!(hooks.contains("PreToolUse"));
    assert!(hooks.contains("Shell"));
    assert!(hooks.contains("Starting Codex context"));
    assert!(!hooks.contains("afterAgentResponse"));
    assert!(codex_home.join("hooks/session-start.sh").is_file());

    Ok(())
}

#[cfg(not(windows))]
fn encoded_cursor_project_path(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches('/')
        .replace('/', "-")
}

#[cfg(windows)]
fn encoded_cursor_project_path(path: &Path) -> String {
    path.to_string_lossy().replace([':', '\\'], "-")
}
