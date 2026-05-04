use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SkillsChangedNotification;
use codex_app_server_protocol::SkillsListExtraRootsForCwd;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::SkillsListResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_config::types::AuthCredentialsStoreMode;
use codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const WATCHER_TIMEOUT: Duration = Duration::from_secs(20);

fn write_skill(root: &TempDir, name: &str) -> Result<()> {
    let skill_dir = root.path().join("skills").join(name);
    std::fs::create_dir_all(&skill_dir)?;
    let content = format!("---\nname: {name}\ndescription: {name} description\n---\n\n# Body\n");
    std::fs::write(skill_dir.join("SKILL.md"), content)?;
    Ok(())
}

fn write_plugins_enabled_config_with_base_url(
    codex_home: &std::path::Path,
    base_url: &str,
) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"chatgpt_base_url = "{base_url}"

[features]
plugins = true
"#,
        ),
    )
}

fn write_remote_plugins_enabled_config_with_base_url(
    codex_home: &std::path::Path,
    base_url: &str,
) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"chatgpt_base_url = "{base_url}"

[features]
plugins = true
remote_plugin = true
"#,
        ),
    )
}

fn write_plugin_with_skill(
    repo_root: &std::path::Path,
    plugin_name: &str,
    skill_name: &str,
) -> Result<()> {
    std::fs::create_dir_all(repo_root.join(".git"))?;
    std::fs::create_dir_all(repo_root.join(".agents/plugins"))?;
    std::fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        format!(
            r#"{{
  "name": "local-marketplace",
  "plugins": [
    {{
      "name": "{plugin_name}",
      "source": {{
        "source": "local",
        "path": "./{plugin_name}"
      }}
    }}
  ]
}}"#
        ),
    )?;

    let plugin_root = repo_root.join(plugin_name);
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        format!(r#"{{"name":"{plugin_name}"}}"#),
    )?;

    let skill_dir = plugin_root.join("skills").join(skill_name);
    std::fs::create_dir_all(&skill_dir)?;
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {skill_name}\ndescription: {skill_name} description\n---\n\n# Body\n"),
    )?;
    Ok(())
}

fn write_cached_remote_plugin_with_skill(
    codex_home: &std::path::Path,
) -> Result<std::path::PathBuf> {
    let plugin_root = codex_home.join("plugins/cache/chatgpt-global/linear/local");
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"linear"}"#,
    )?;

    let skill_dir = plugin_root.join("skills/triage-issues");
    std::fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    std::fs::write(
        &skill_path,
        "---\nname: triage-issues\ndescription: Triage Linear issues\n---\n\n# Body\n",
    )?;
    Ok(skill_path)
}

#[tokio::test]
async fn skills_list_includes_skills_from_per_cwd_extra_user_roots() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    let extra_root = TempDir::new()?;
    write_skill(&extra_root, "extra-skill")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_skills_list_request(SkillsListParams {
            cwds: vec![cwd.path().to_path_buf()],
            force_reload: true,
            per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                cwd: cwd.path().to_path_buf(),
                extra_user_roots: vec![extra_root.path().to_path_buf()],
            }]),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let SkillsListResponse { data } = to_response(response)?;
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].cwd.as_path(), cwd.path());
    assert!(
        data[0]
            .skills
            .iter()
            .any(|skill| skill.name == "extra-skill")
    );
    Ok(())
}

#[tokio::test]
async fn skills_list_loads_remote_installed_plugin_skills_from_cache() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    let server = MockServer::start().await;
    let expected_skill_path =
        std::fs::canonicalize(write_cached_remote_plugin_with_skill(codex_home.path())?)?;
    write_remote_plugins_enabled_config_with_base_url(
        codex_home.path(),
        &format!("{}/backend-api/", server.uri()),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let global_directory_body = r#"{
  "plugins": [
    {
      "id": "plugins~Plugin_linear",
      "name": "linear",
      "scope": "GLOBAL",
      "installation_policy": "AVAILABLE",
      "authentication_policy": "ON_USE",
      "release": {
        "display_name": "Linear",
        "description": "Track work in Linear",
        "app_ids": [],
        "interface": {},
        "skills": []
      }
    }
  ],
  "pagination": {
    "limit": 50,
    "next_page_token": null
  }
}"#;
    let global_installed_body = r#"{
  "plugins": [
    {
      "id": "plugins~Plugin_linear",
      "name": "linear",
      "scope": "GLOBAL",
      "installation_policy": "AVAILABLE",
      "authentication_policy": "ON_USE",
      "release": {
        "display_name": "Linear",
        "description": "Track work in Linear",
        "app_ids": [],
        "interface": {},
        "skills": []
      },
      "enabled": true,
      "disabled_skill_names": []
    }
  ],
  "pagination": {
    "limit": 50,
    "next_page_token": null
  }
}"#;
    let empty_page_body = r#"{
  "plugins": [],
  "pagination": {
    "limit": 50,
    "next_page_token": null
  }
}"#;

    for (scope, body) in [
        ("GLOBAL", global_directory_body),
        ("WORKSPACE", empty_page_body),
    ] {
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/list"))
            .and(query_param("scope", scope))
            .and(query_param("limit", "200"))
            .and(header("authorization", "Bearer chatgpt-token"))
            .and(header("chatgpt-account-id", "account-123"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let stale_skills_list_request_id = mcp
        .send_skills_list_request(SkillsListParams {
            cwds: vec![cwd.path().to_path_buf()],
            force_reload: true,
            per_cwd_extra_user_roots: None,
        })
        .await?;
    let stale_skills_list_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(stale_skills_list_request_id)),
    )
    .await??;
    let SkillsListResponse { data } = to_response(stale_skills_list_response)?;
    assert_eq!(data.len(), 1);
    assert!(
        data[0]
            .skills
            .iter()
            .all(|skill| skill.name != "linear:triage-issues"),
        "remote installed plugin cache has not been refreshed yet"
    );

    for (scope, body) in [
        ("GLOBAL", global_installed_body),
        ("WORKSPACE", empty_page_body),
    ] {
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/installed"))
            .and(query_param("scope", scope))
            .and(header("authorization", "Bearer chatgpt-token"))
            .and(header("chatgpt-account-id", "account-123"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }

    let plugin_list_request_id = mcp
        .send_plugin_list_request(PluginListParams { cwds: None })
        .await?;
    let plugin_list_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(plugin_list_request_id)),
    )
    .await??;
    let _: PluginListResponse = to_response(plugin_list_response)?;

    let SkillsListResponse { data } = timeout(DEFAULT_TIMEOUT, async {
        loop {
            let skills_list_request_id = mcp
                .send_skills_list_request(SkillsListParams {
                    cwds: vec![cwd.path().to_path_buf()],
                    force_reload: false,
                    per_cwd_extra_user_roots: None,
                })
                .await?;
            let skills_list_response: JSONRPCResponse = timeout(
                DEFAULT_TIMEOUT,
                mcp.read_stream_until_response_message(RequestId::Integer(skills_list_request_id)),
            )
            .await??;
            let response: SkillsListResponse = to_response(skills_list_response)?;
            if response.data.iter().any(|entry| {
                entry
                    .skills
                    .iter()
                    .any(|skill| skill.name == "linear:triage-issues")
            }) {
                break Ok::<SkillsListResponse, anyhow::Error>(response);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await??;

    assert_eq!(data.len(), 1);
    assert_eq!(data[0].errors, Vec::new());
    let skill = data[0]
        .skills
        .iter()
        .find(|skill| skill.name == "linear:triage-issues")
        .expect("expected skill from cached remote plugin");
    assert_eq!(
        std::fs::canonicalize(skill.path.as_path())?,
        expected_skill_path
    );
    assert_eq!(skill.enabled, true);
    Ok(())
}

#[tokio::test]
async fn skills_list_excludes_plugin_skills_when_workspace_codex_plugins_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let server = MockServer::start().await;
    write_skill(&codex_home, "home-skill")?;
    write_plugin_with_skill(repo_root.path(), "demo-plugin", "plugin-skill")?;
    write_plugins_enabled_config_with_base_url(
        codex_home.path(),
        &format!("{}/backend-api/", server.uri()),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123")
            .plan_type("team"),
        AuthCredentialsStoreMode::File,
    )?;
    Mock::given(method("GET"))
        .and(path("/backend-api/accounts/account-123/settings"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"beta_settings":{"enable_plugins":false}}"#),
        )
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_skills_list_request(SkillsListParams {
            cwds: vec![repo_root.path().to_path_buf()],
            force_reload: true,
            per_cwd_extra_user_roots: None,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let SkillsListResponse { data } = to_response(response)?;
    assert_eq!(data.len(), 1);
    assert!(
        data[0]
            .skills
            .iter()
            .any(|skill| skill.name == "home-skill"),
        "non-plugin skills should remain available"
    );
    assert!(
        data[0]
            .skills
            .iter()
            .all(|skill| skill.name != "demo-plugin:plugin-skill"),
        "plugin skills should be hidden when workspace Codex plugins are disabled"
    );
    Ok(())
}

#[tokio::test]
async fn skills_list_skips_cwd_roots_when_environment_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    let extra_root = TempDir::new()?;
    write_skill(&codex_home, "home-skill")?;
    write_skill(&extra_root, "extra-skill")?;

    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[(CODEX_EXEC_SERVER_URL_ENV_VAR, Some("none"))],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_skills_list_request(SkillsListParams {
            cwds: vec![cwd.path().to_path_buf()],
            force_reload: true,
            per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                cwd: cwd.path().to_path_buf(),
                extra_user_roots: vec![extra_root.path().to_path_buf()],
            }]),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let SkillsListResponse { data } = to_response(response)?;
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].cwd, cwd.path().to_path_buf());
    assert_eq!(data[0].errors, Vec::new());
    assert!(
        data[0]
            .skills
            .iter()
            .any(|skill| skill.name == "home-skill")
    );
    assert!(
        data[0]
            .skills
            .iter()
            .all(|skill| skill.name != "extra-skill")
    );
    Ok(())
}

#[tokio::test]
async fn skills_list_rejects_relative_extra_user_roots() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_skills_list_request(SkillsListParams {
            cwds: vec![cwd.path().to_path_buf()],
            force_reload: true,
            per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                cwd: cwd.path().to_path_buf(),
                extra_user_roots: vec![std::path::PathBuf::from("relative/skills")],
            }]),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert!(
        err.error
            .message
            .contains("perCwdExtraUserRoots extraUserRoots paths must be absolute"),
        "unexpected error: {}",
        err.error.message
    );
    Ok(())
}

#[tokio::test]
async fn skills_list_accepts_relative_cwds() -> Result<()> {
    let codex_home = TempDir::new()?;
    let relative_cwd = std::path::PathBuf::from("relative-cwd");
    std::fs::create_dir_all(codex_home.path().join(&relative_cwd))?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_skills_list_request(SkillsListParams {
            cwds: vec![relative_cwd.clone()],
            force_reload: true,
            per_cwd_extra_user_roots: None,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let SkillsListResponse { data } = to_response(response)?;
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].cwd, relative_cwd);
    assert_eq!(data[0].errors, Vec::new());
    Ok(())
}

#[tokio::test]
async fn skills_list_ignores_per_cwd_extra_roots_for_unknown_cwd() -> Result<()> {
    let codex_home = TempDir::new()?;
    let requested_cwd = TempDir::new()?;
    let unknown_cwd = TempDir::new()?;
    let extra_root = TempDir::new()?;
    write_skill(&extra_root, "ignored-extra-skill")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_skills_list_request(SkillsListParams {
            cwds: vec![requested_cwd.path().to_path_buf()],
            force_reload: true,
            per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                cwd: unknown_cwd.path().to_path_buf(),
                extra_user_roots: vec![extra_root.path().to_path_buf()],
            }]),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let SkillsListResponse { data } = to_response(response)?;
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].cwd.as_path(), requested_cwd.path());
    assert!(
        data[0]
            .skills
            .iter()
            .all(|skill| skill.name != "ignored-extra-skill")
    );
    Ok(())
}

#[tokio::test]
async fn skills_list_uses_cached_result_until_force_reload() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    let extra_root = TempDir::new()?;
    write_skill(&extra_root, "late-extra-skill")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    // Seed the cwd cache first without extra roots.
    let first_request_id = mcp
        .send_skills_list_request(SkillsListParams {
            cwds: vec![cwd.path().to_path_buf()],
            force_reload: false,
            per_cwd_extra_user_roots: None,
        })
        .await?;
    let first_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_request_id)),
    )
    .await??;
    let SkillsListResponse { data: first_data } = to_response(first_response)?;
    assert_eq!(first_data.len(), 1);
    assert!(
        first_data[0]
            .skills
            .iter()
            .all(|skill| skill.name != "late-extra-skill")
    );

    let second_request_id = mcp
        .send_skills_list_request(SkillsListParams {
            cwds: vec![cwd.path().to_path_buf()],
            force_reload: false,
            per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                cwd: cwd.path().to_path_buf(),
                extra_user_roots: vec![extra_root.path().to_path_buf()],
            }]),
        })
        .await?;
    let second_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_request_id)),
    )
    .await??;
    let SkillsListResponse { data: second_data } = to_response(second_response)?;
    assert_eq!(second_data.len(), 1);
    assert!(
        second_data[0]
            .skills
            .iter()
            .all(|skill| skill.name != "late-extra-skill")
    );

    let third_request_id = mcp
        .send_skills_list_request(SkillsListParams {
            cwds: vec![cwd.path().to_path_buf()],
            force_reload: true,
            per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                cwd: cwd.path().to_path_buf(),
                extra_user_roots: vec![extra_root.path().to_path_buf()],
            }]),
        })
        .await?;
    let third_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(third_request_id)),
    )
    .await??;
    let SkillsListResponse { data: third_data } = to_response(third_response)?;
    assert_eq!(third_data.len(), 1);
    assert!(
        third_data[0]
            .skills
            .iter()
            .any(|skill| skill.name == "late-extra-skill")
    );
    Ok(())
}

#[tokio::test]
async fn skills_changed_notification_is_emitted_after_skill_change() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_skill(&codex_home, "demo")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let thread_start_request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: None,
            model_provider: None,
            service_tier: None,
            cwd: None,
            approval_policy: None,
            approvals_reviewer: None,
            sandbox: None,
            permissions: None,
            config: None,
            service_name: None,
            base_instructions: None,
            developer_instructions: None,
            personality: None,
            ephemeral: None,
            session_start_source: None,
            dynamic_tools: None,
            environments: None,
            mock_experimental_field: None,
            experimental_raw_events: false,
            persist_extended_history: false,
        })
        .await?;
    let _: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_request_id)),
    )
    .await??;

    let skill_path = codex_home
        .path()
        .join("skills")
        .join("demo")
        .join("SKILL.md");
    std::fs::write(
        &skill_path,
        "---\nname: demo\ndescription: updated\n---\n\n# Updated\n",
    )?;

    let notification = timeout(
        WATCHER_TIMEOUT,
        mcp.read_stream_until_notification_message("skills/changed"),
    )
    .await??;
    let params = notification
        .params
        .context("skills/changed params must be present")?;
    let notification: SkillsChangedNotification = serde_json::from_value(params)?;

    assert_eq!(notification, SkillsChangedNotification {});
    Ok(())
}
