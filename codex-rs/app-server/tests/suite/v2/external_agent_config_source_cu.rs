use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn external_agent_config_imports_source_cu_repo_artifacts() -> Result<()> {
    let codex_home = TempDir::new()?;
    let project_root = codex_home.path().join("repo");
    let source_dir = project_root.join(".cursor");
    let nested_scope = project_root.join("backend");
    std::fs::create_dir_all(project_root.join(".git"))?;
    std::fs::create_dir_all(source_dir.join("skills/release"))?;
    std::fs::create_dir_all(source_dir.join("commands"))?;
    std::fs::create_dir_all(source_dir.join("agents"))?;
    std::fs::create_dir_all(source_dir.join("rules/team"))?;
    std::fs::create_dir_all(nested_scope.join(".cursor/rules"))?;
    std::fs::write(
        source_dir.join("mcp.json"),
        r#"{"mcpServers":{"docs":{"command":"docs-server","args":["--stdio"]}}}"#,
    )?;
    std::fs::write(
        source_dir.join("skills/release/SKILL.md"),
        "---\nname: release\ndescription: Release with Cursor\n---\n\nUse Cursor to release. Advance the database cursor after each page.\n",
    )?;
    std::fs::write(
        source_dir.join("commands/review.md"),
        "Review the change with Cursor. Keep the text cursor visible.\n",
    )?;
    std::fs::write(
        source_dir.join("agents/researcher.md"),
        "---\nname: researcher\ndescription: Cursor research role\n---\nResearch with Cursor.\n",
    )?;
    std::fs::write(
        project_root.join(".cursorrules"),
        "Use Cursor for repository work. Keep the text cursor visible.\n",
    )?;
    std::fs::write(
        source_dir.join("rules/always.mdc"),
        "---\ndescription: Always apply\nalwaysApply: true # YAML comments are valid\n---\nRun checks in Cursor.\n",
    )?;
    std::fs::write(
        source_dir.join("rules/team/database.mdc"),
        "---\ndescription: Database guidance\nalwaysApply: true\n---\nDocument database cursor behavior.\n",
    )?;
    std::fs::write(
        source_dir.join("rules/scoped.mdc"),
        "---\ndescription: TypeScript only\nglobs: '*.ts'\nalwaysApply: false\n---\nUse TypeScript.\n",
    )?;
    std::fs::write(
        nested_scope.join(".cursor/rules/always.mdc"),
        "---\ndescription: Backend guidance\nalwaysApply: true\n---\nUse Cursor for backend work.\n",
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build()
        .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/detect",
            Some(serde_json::json!({
                "includeHome": false,
                "cwds": [&project_root],
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
    assert_eq!(
        detected
            .items
            .iter()
            .map(|item| item.item_type)
            .collect::<Vec<_>>(),
        vec![
            ExternalAgentConfigMigrationItemType::McpServerConfig,
            ExternalAgentConfigMigrationItemType::Skills,
            ExternalAgentConfigMigrationItemType::Commands,
            ExternalAgentConfigMigrationItemType::Subagents,
            ExternalAgentConfigMigrationItemType::AgentsMd,
            ExternalAgentConfigMigrationItemType::AgentsMd,
        ]
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
            .all(|result| result.failures.is_empty())
    );

    let config = std::fs::read_to_string(project_root.join(".codex/config.toml"))?;
    assert!(config.contains("[mcp_servers.docs]"));
    assert!(config.contains("command = \"docs-server\""));
    let skill = std::fs::read_to_string(project_root.join(".agents/skills/release/SKILL.md"))?;
    assert!(skill.contains("Release with Codex"));
    assert!(skill.contains("database cursor"));
    let command = std::fs::read_to_string(
        project_root.join(".agents/skills/source-command-review/SKILL.md"),
    )?;
    assert!(command.contains("Review the change with Codex"));
    assert!(command.contains("Migrated source command `review`"));
    assert!(command.contains("text cursor"));
    let subagent = std::fs::read_to_string(project_root.join(".codex/agents/researcher.toml"))?;
    assert!(subagent.contains("Codex research role"));
    let agents_md = std::fs::read_to_string(project_root.join("AGENTS.md"))?;
    assert!(agents_md.contains("Use Codex for repository work."));
    assert!(agents_md.contains("Run checks in Codex."));
    assert!(agents_md.contains("Document database cursor behavior."));
    assert!(agents_md.contains("text cursor"));
    assert!(!agents_md.contains("Use TypeScript."));
    assert!(!agents_md.contains("alwaysApply"));
    let nested_agents_md = std::fs::read_to_string(nested_scope.join("AGENTS.md"))?;
    assert!(nested_agents_md.contains("Use Codex for backend work."));

    Ok(())
}

#[tokio::test]
async fn external_agent_config_imports_source_cu_home_artifacts() -> Result<()> {
    let root = TempDir::new()?;
    let home = root.path().join("home");
    let codex_home = home.join(".codex");
    let source_dir = home.join(".cursor");
    std::fs::create_dir_all(&codex_home)?;
    std::fs::create_dir_all(source_dir.join("skills/home-release"))?;
    std::fs::create_dir_all(source_dir.join("commands"))?;
    std::fs::create_dir_all(source_dir.join("agents"))?;
    std::fs::create_dir_all(source_dir.join("rules"))?;
    std::fs::write(
        source_dir.join("mcp.json"),
        r#"{"mcpServers":{"home-docs":{"command":"home-docs-server"}}}"#,
    )?;
    std::fs::write(
        source_dir.join("skills/home-release/SKILL.md"),
        "---\nname: home-release\ndescription: Release with Cursor\n---\n\nUse Cursor for home releases.\n",
    )?;
    std::fs::write(
        source_dir.join("commands/home-review.md"),
        "Review home changes with Cursor.\n",
    )?;
    std::fs::write(
        source_dir.join("agents/home-researcher.md"),
        "---\nname: home-researcher\ndescription: Research with Cursor\n---\nResearch with Cursor.\n",
    )?;
    std::fs::write(
        source_dir.join("rules/always.mdc"),
        "---\ndescription: Home guidance\nalwaysApply: true\n---\nUse Cursor for home work.\n",
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
                "cwds": [],
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
    assert_eq!(
        detected
            .items
            .iter()
            .map(|item| item.item_type)
            .collect::<Vec<_>>(),
        vec![
            ExternalAgentConfigMigrationItemType::McpServerConfig,
            ExternalAgentConfigMigrationItemType::Skills,
            ExternalAgentConfigMigrationItemType::Commands,
            ExternalAgentConfigMigrationItemType::Subagents,
            ExternalAgentConfigMigrationItemType::AgentsMd,
        ]
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
            .all(|result| result.failures.is_empty())
    );

    let config = std::fs::read_to_string(codex_home.join("config.toml"))?;
    assert!(config.contains("[mcp_servers.home-docs]"));
    let skill = std::fs::read_to_string(home.join(".agents/skills/home-release/SKILL.md"))?;
    assert!(skill.contains("Release with Codex"));
    assert!(skill.contains("Use Codex for home releases."));
    let command =
        std::fs::read_to_string(home.join(".agents/skills/source-command-home-review/SKILL.md"))?;
    assert!(command.contains("Review home changes with Codex."));
    let subagent = std::fs::read_to_string(codex_home.join("agents/home-researcher.toml"))?;
    assert!(subagent.contains("Research with Codex"));
    let agents_md = std::fs::read_to_string(codex_home.join("AGENTS.md"))?;
    assert!(agents_md.contains("Use Codex for home work."));

    Ok(())
}

#[tokio::test]
async fn source_cu_import_does_not_accept_source_cl_sessions() -> Result<()> {
    let codex_home = TempDir::new()?;
    let project_root = codex_home.path().join("repo");
    let session_path = external_agent_home(codex_home.path()).join("projects/repo/session.jsonl");
    std::fs::create_dir_all(session_path.parent().expect("session parent"))?;
    std::fs::write(&session_path, "{}\n")?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("HOME", Some(home_dir.as_str()))])
        .build()
        .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "source": "cursor",
                "migrationItems": [{
                    "itemType": "SESSIONS",
                    "description": "Migrate recent sessions",
                    "cwd": null,
                    "details": {
                        "sessions": [{
                            "path": session_path,
                            "cwd": project_root,
                            "title": "wrong source"
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
    let result = &completed.item_type_results[0];
    assert_eq!(
        result.item_type,
        ExternalAgentConfigMigrationItemType::Sessions
    );
    assert!(result.successes.is_empty());
    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.failures[0].failure_stage, "session_missing");

    Ok(())
}
