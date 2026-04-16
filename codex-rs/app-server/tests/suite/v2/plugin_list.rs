use std::time::Duration;

use anyhow::Result;
use anyhow::bail;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginMarketplaceEntry;
use codex_app_server_protocol::PluginSource;
use codex_app_server_protocol::PluginSummary;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::config::set_project_trust_level;
use codex_protocol::config_types::TrustLevel;
use codex_utils_absolute_path::AbsolutePathBuf;
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

// These tests start full app-server processes and can also run plugin startup warmers.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const TEST_CURATED_PLUGIN_SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const STARTUP_REMOTE_PLUGIN_SYNC_MARKER_FILE: &str = ".tmp/app-server-remote-plugin-sync-v1";

fn write_plugins_enabled_config(codex_home: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        r#"[features]
plugins = true
"#,
    )
}

#[tokio::test]
async fn plugin_list_skips_invalid_marketplace_file_and_reports_error() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(repo_root.path().join(".agents/plugins"))?;
    write_plugins_enabled_config(codex_home.path())?;
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;
    std::fs::write(marketplace_path.as_path(), "{not json")?;

    let home = codex_home.path().to_string_lossy().into_owned();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            ("HOME", Some(home.as_str())),
            ("USERPROFILE", Some(home.as_str())),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: Some(vec![AbsolutePathBuf::try_from(repo_root.path())?]),
            force_remote_sync: false,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;

    assert!(
        response
            .marketplaces
            .iter()
            .all(|marketplace| { marketplace.path != marketplace_path }),
        "invalid marketplace should be skipped"
    );
    assert_eq!(response.marketplace_load_errors.len(), 1);
    assert_eq!(
        response.marketplace_load_errors[0].marketplace_path,
        marketplace_path
    );
    assert!(
        response.marketplace_load_errors[0]
            .message
            .contains("invalid marketplace file"),
        "unexpected error: {:?}",
        response.marketplace_load_errors
    );
    Ok(())
}

#[tokio::test]
async fn plugin_list_rejects_relative_cwds() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "plugin/list",
            Some(serde_json::json!({
                "cwds": ["relative-root"],
            })),
        )
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(err.error.message.contains("Invalid request"));
    Ok(())
}

#[tokio::test]
async fn plugin_list_keeps_valid_marketplaces_when_another_marketplace_fails_to_load() -> Result<()>
{
    let codex_home = TempDir::new()?;
    let valid_repo_root = TempDir::new()?;
    let invalid_repo_root = TempDir::new()?;
    std::fs::create_dir_all(valid_repo_root.path().join(".git"))?;
    std::fs::create_dir_all(valid_repo_root.path().join(".agents/plugins"))?;
    std::fs::create_dir_all(
        valid_repo_root
            .path()
            .join("plugins/valid-plugin/.codex-plugin"),
    )?;
    std::fs::create_dir_all(invalid_repo_root.path().join(".git"))?;
    std::fs::create_dir_all(invalid_repo_root.path().join(".agents/plugins"))?;
    write_plugins_enabled_config(codex_home.path())?;

    let valid_marketplace_path = AbsolutePathBuf::try_from(
        valid_repo_root
            .path()
            .join(".agents/plugins/marketplace.json"),
    )?;
    let invalid_marketplace_path = AbsolutePathBuf::try_from(
        invalid_repo_root
            .path()
            .join(".agents/plugins/marketplace.json"),
    )?;
    let valid_plugin_path =
        AbsolutePathBuf::try_from(valid_repo_root.path().join("plugins/valid-plugin"))?;

    std::fs::write(
        valid_marketplace_path.as_path(),
        r#"{
  "name": "valid-marketplace",
  "plugins": [
    {
      "name": "valid-plugin",
      "source": {
        "source": "local",
        "path": "./plugins/valid-plugin"
      }
    }
  ]
}"#,
    )?;
    std::fs::write(
        valid_repo_root
            .path()
            .join("plugins/valid-plugin/.codex-plugin/plugin.json"),
        r#"{"name":"valid-plugin"}"#,
    )?;
    std::fs::write(invalid_marketplace_path.as_path(), "{not json")?;

    let home = codex_home.path().to_string_lossy().into_owned();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            ("HOME", Some(home.as_str())),
            ("USERPROFILE", Some(home.as_str())),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: Some(vec![
                AbsolutePathBuf::try_from(valid_repo_root.path())?,
                AbsolutePathBuf::try_from(invalid_repo_root.path())?,
            ]),
            force_remote_sync: false,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;

    assert_eq!(
        response.marketplaces,
        vec![PluginMarketplaceEntry {
            name: "valid-marketplace".to_string(),
            path: valid_marketplace_path,
            interface: None,
            plugins: vec![PluginSummary {
                id: "valid-plugin@valid-marketplace".to_string(),
                name: "valid-plugin".to_string(),
                source: PluginSource::Local {
                    path: valid_plugin_path,
                },
                installed: false,
                enabled: false,
                install_policy: PluginInstallPolicy::Available,
                auth_policy: PluginAuthPolicy::OnInstall,
                interface: None,
            }],
        }]
    );
    assert_eq!(response.marketplace_load_errors.len(), 1);
    assert_eq!(
        response.marketplace_load_errors[0].marketplace_path,
        invalid_marketplace_path
    );
    assert!(
        response.marketplace_load_errors[0]
            .message
            .contains("invalid marketplace file"),
        "unexpected error: {:?}",
        response.marketplace_load_errors
    );
    assert_eq!(response.remote_sync_error, None);
    assert!(response.featured_plugin_ids.is_empty());
    Ok(())
}

#[tokio::test]
async fn plugin_list_accepts_omitted_cwds() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::create_dir_all(codex_home.path().join(".agents/plugins"))?;
    write_plugins_enabled_config(codex_home.path())?;
    std::fs::write(
        codex_home.path().join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "home-plugin",
      "source": {
        "source": "local",
        "path": "./home-plugin"
      }
    }
  ]
}"#,
    )?;
    let home = codex_home.path().to_string_lossy().into_owned();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            ("HOME", Some(home.as_str())),
            ("USERPROFILE", Some(home.as_str())),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: None,
            force_remote_sync: false,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: PluginListResponse = to_response(response)?;
    Ok(())
}

#[tokio::test]
async fn plugin_list_includes_install_and_enabled_state_from_config() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(repo_root.path().join(".agents/plugins"))?;
    write_installed_plugin(&codex_home, "codex-curated", "enabled-plugin")?;
    write_installed_plugin(&codex_home, "codex-curated", "disabled-plugin")?;
    std::fs::write(
        repo_root.path().join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "interface": {
    "displayName": "ChatGPT Official"
  },
  "plugins": [
    {
      "name": "enabled-plugin",
      "source": {
        "source": "local",
        "path": "./enabled-plugin"
      }
    },
    {
      "name": "disabled-plugin",
      "source": {
        "source": "local",
        "path": "./disabled-plugin"
      }
    },
    {
      "name": "uninstalled-plugin",
      "source": {
        "source": "local",
        "path": "./uninstalled-plugin"
      }
    }
  ]
}"#,
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"[features]
plugins = true

[plugins."enabled-plugin@codex-curated"]
enabled = true

[plugins."disabled-plugin@codex-curated"]
enabled = false
"#,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: Some(vec![AbsolutePathBuf::try_from(repo_root.path())?]),
            force_remote_sync: false,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;

    let marketplace = response
        .marketplaces
        .into_iter()
        .find(|marketplace| {
            marketplace.path
                == AbsolutePathBuf::try_from(
                    repo_root.path().join(".agents/plugins/marketplace.json"),
                )
                .expect("absolute marketplace path")
        })
        .expect("expected repo marketplace entry");

    assert_eq!(marketplace.name, "codex-curated");
    assert_eq!(
        marketplace
            .interface
            .as_ref()
            .and_then(|interface| interface.display_name.as_deref()),
        Some("ChatGPT Official")
    );
    assert_eq!(marketplace.plugins.len(), 3);
    assert_eq!(marketplace.plugins[0].id, "enabled-plugin@codex-curated");
    assert_eq!(marketplace.plugins[0].name, "enabled-plugin");
    assert_eq!(marketplace.plugins[0].installed, true);
    assert_eq!(marketplace.plugins[0].enabled, true);
    assert_eq!(
        marketplace.plugins[0].install_policy,
        PluginInstallPolicy::Available
    );
    assert_eq!(
        marketplace.plugins[0].auth_policy,
        PluginAuthPolicy::OnInstall
    );
    assert_eq!(marketplace.plugins[1].id, "disabled-plugin@codex-curated");
    assert_eq!(marketplace.plugins[1].name, "disabled-plugin");
    assert_eq!(marketplace.plugins[1].installed, true);
    assert_eq!(marketplace.plugins[1].enabled, false);
    assert_eq!(
        marketplace.plugins[1].install_policy,
        PluginInstallPolicy::Available
    );
    assert_eq!(
        marketplace.plugins[1].auth_policy,
        PluginAuthPolicy::OnInstall
    );
    assert_eq!(
        marketplace.plugins[2].id,
        "uninstalled-plugin@codex-curated"
    );
    assert_eq!(marketplace.plugins[2].name, "uninstalled-plugin");
    assert_eq!(marketplace.plugins[2].installed, false);
    assert_eq!(marketplace.plugins[2].enabled, false);
    assert_eq!(
        marketplace.plugins[2].install_policy,
        PluginInstallPolicy::Available
    );
    assert_eq!(
        marketplace.plugins[2].auth_policy,
        PluginAuthPolicy::OnInstall
    );
    Ok(())
}

#[tokio::test]
async fn plugin_list_uses_home_config_for_enabled_state() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::create_dir_all(codex_home.path().join(".agents/plugins"))?;
    write_installed_plugin(&codex_home, "codex-curated", "shared-plugin")?;
    std::fs::write(
        codex_home.path().join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "shared-plugin",
      "source": {
        "source": "local",
        "path": "./shared-plugin"
      }
    }
  ]
}"#,
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"[features]
plugins = true

[plugins."shared-plugin@codex-curated"]
enabled = true
"#,
    )?;

    let workspace_enabled = TempDir::new()?;
    std::fs::create_dir_all(workspace_enabled.path().join(".git"))?;
    std::fs::create_dir_all(workspace_enabled.path().join(".agents/plugins"))?;
    std::fs::write(
        workspace_enabled
            .path()
            .join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "shared-plugin",
      "source": {
        "source": "local",
        "path": "./shared-plugin"
      }
    }
  ]
}"#,
    )?;
    std::fs::create_dir_all(workspace_enabled.path().join(".codex"))?;
    std::fs::write(
        workspace_enabled.path().join(".codex/config.toml"),
        r#"[plugins."shared-plugin@codex-curated"]
enabled = false
"#,
    )?;
    set_project_trust_level(
        codex_home.path(),
        workspace_enabled.path(),
        TrustLevel::Trusted,
    )?;

    let workspace_default = TempDir::new()?;
    let home = codex_home.path().to_string_lossy().into_owned();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            ("HOME", Some(home.as_str())),
            ("USERPROFILE", Some(home.as_str())),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: Some(vec![
                AbsolutePathBuf::try_from(workspace_enabled.path())?,
                AbsolutePathBuf::try_from(workspace_default.path())?,
            ]),
            force_remote_sync: false,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;

    let shared_plugin = response
        .marketplaces
        .iter()
        .flat_map(|marketplace| marketplace.plugins.iter())
        .find(|plugin| plugin.name == "shared-plugin")
        .expect("expected shared-plugin entry");
    assert_eq!(shared_plugin.id, "shared-plugin@codex-curated");
    assert_eq!(shared_plugin.installed, true);
    assert_eq!(shared_plugin.enabled, true);
    Ok(())
}

#[tokio::test]
async fn plugin_list_returns_plugin_interface_with_absolute_asset_paths() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let plugin_root = repo_root.path().join("plugins/demo-plugin");
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(repo_root.path().join(".agents/plugins"))?;
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    write_plugins_enabled_config(codex_home.path())?;
    std::fs::write(
        repo_root.path().join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "demo-plugin",
      "source": {
        "source": "local",
        "path": "./plugins/demo-plugin"
      },
      "policy": {
        "installation": "AVAILABLE",
        "authentication": "ON_INSTALL"
      },
      "category": "Design"
    }
  ]
}"#,
    )?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r##"{
  "name": "demo-plugin",
  "interface": {
    "displayName": "Plugin Display Name",
    "shortDescription": "Short description for subtitle",
    "longDescription": "Long description for details page",
    "developerName": "OpenAI",
    "category": "Productivity",
    "capabilities": ["Interactive", "Write"],
    "websiteURL": "https://openai.com/",
    "privacyPolicyURL": "https://openai.com/policies/row-privacy-policy/",
    "termsOfServiceURL": "https://openai.com/policies/row-terms-of-use/",
    "defaultPrompt": [
      "Starter prompt for trying a plugin",
      "Find my next action"
    ],
    "brandColor": "#3B82F6",
    "composerIcon": "./assets/icon.png",
    "logo": "./assets/logo.png",
    "screenshots": ["./assets/screenshot1.png", "./assets/screenshot2.png"]
  }
}"##,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: Some(vec![AbsolutePathBuf::try_from(repo_root.path())?]),
            force_remote_sync: false,
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
        .flat_map(|marketplace| marketplace.plugins.iter())
        .find(|plugin| plugin.name == "demo-plugin")
        .expect("expected demo-plugin entry");

    assert_eq!(plugin.id, "demo-plugin@codex-curated");
    assert_eq!(plugin.installed, false);
    assert_eq!(plugin.enabled, false);
    assert_eq!(plugin.install_policy, PluginInstallPolicy::Available);
    assert_eq!(plugin.auth_policy, PluginAuthPolicy::OnInstall);
    let interface = plugin
        .interface
        .as_ref()
        .expect("expected plugin interface");
    assert_eq!(
        interface.display_name.as_deref(),
        Some("Plugin Display Name")
    );
    assert_eq!(interface.category.as_deref(), Some("Design"));
    assert_eq!(
        interface.website_url.as_deref(),
        Some("https://openai.com/")
    );
    assert_eq!(
        interface.privacy_policy_url.as_deref(),
        Some("https://openai.com/policies/row-privacy-policy/")
    );
    assert_eq!(
        interface.terms_of_service_url.as_deref(),
        Some("https://openai.com/policies/row-terms-of-use/")
    );
    assert_eq!(
        interface.default_prompt,
        Some(vec![
            "Starter prompt for trying a plugin".to_string(),
            "Find my next action".to_string()
        ])
    );
    assert_eq!(
        interface.composer_icon,
        Some(AbsolutePathBuf::try_from(
            plugin_root.join("assets/icon.png")
        )?)
    );
    assert_eq!(
        interface.logo,
        Some(AbsolutePathBuf::try_from(
            plugin_root.join("assets/logo.png")
        )?)
    );
    assert_eq!(
        interface.screenshots,
        vec![
            AbsolutePathBuf::try_from(plugin_root.join("assets/screenshot1.png"))?,
            AbsolutePathBuf::try_from(plugin_root.join("assets/screenshot2.png"))?,
        ]
    );
    Ok(())
}

#[tokio::test]
async fn plugin_list_accepts_legacy_string_default_prompt() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let plugin_root = repo_root.path().join("plugins/demo-plugin");
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(repo_root.path().join(".agents/plugins"))?;
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    write_plugins_enabled_config(codex_home.path())?;
    std::fs::write(
        repo_root.path().join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "demo-plugin",
      "source": {
        "source": "local",
        "path": "./plugins/demo-plugin"
      }
    }
  ]
}"#,
    )?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r##"{
  "name": "demo-plugin",
  "interface": {
    "defaultPrompt": "Starter prompt for trying a plugin"
  }
}"##,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: Some(vec![AbsolutePathBuf::try_from(repo_root.path())?]),
            force_remote_sync: false,
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
        .flat_map(|marketplace| marketplace.plugins.iter())
        .find(|plugin| plugin.name == "demo-plugin")
        .expect("expected demo-plugin entry");
    assert_eq!(
        plugin
            .interface
            .as_ref()
            .and_then(|interface| interface.default_prompt.clone()),
        Some(vec!["Starter prompt for trying a plugin".to_string()])
    );
    Ok(())
}

#[tokio::test]
async fn plugin_list_force_remote_sync_returns_remote_sync_error_on_fail_open() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_plugin_sync_config(codex_home.path(), "https://chatgpt.com/backend-api/")?;
    write_openai_curated_marketplace(codex_home.path(), &["linear"])?;
    write_installed_plugin(&codex_home, "openai-curated", "linear")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: None,
            force_remote_sync: true,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;

    assert!(
        response
            .remote_sync_error
            .as_deref()
            .is_some_and(|message| message.contains("chatgpt authentication required"))
    );
    let curated_marketplace = response
        .marketplaces
        .into_iter()
        .find(|marketplace| marketplace.name == "openai-curated")
        .expect("expected openai-curated marketplace entry");
    assert_eq!(
        curated_marketplace
            .plugins
            .into_iter()
            .map(|plugin| (plugin.id, plugin.installed, plugin.enabled))
            .collect::<Vec<_>>(),
        vec![("linear@openai-curated".to_string(), true, false)]
    );
    Ok(())
}

#[tokio::test]
async fn plugin_list_force_remote_sync_reconciles_curated_plugin_state() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    write_plugin_sync_config(codex_home.path(), &format!("{}/backend-api/", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    write_openai_curated_marketplace(codex_home.path(), &["linear", "gmail", "calendar"])?;
    write_installed_plugin(&codex_home, "openai-curated", "linear")?;
    write_installed_plugin(&codex_home, "openai-curated", "gmail")?;
    write_installed_plugin(&codex_home, "openai-curated", "calendar")?;

    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/list"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"[
  {"id":"1","name":"linear","marketplace_name":"openai-curated","version":"1.0.0","enabled":true},
  {"id":"2","name":"gmail","marketplace_name":"openai-curated","version":"1.0.0","enabled":false}
]"#,
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/featured"))
        .and(query_param("platform", "codex"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"["linear@openai-curated","calendar@openai-curated"]"#),
        )
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: None,
            force_remote_sync: true,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;
    assert_eq!(response.remote_sync_error, None);
    assert_eq!(
        response.featured_plugin_ids,
        vec![
            "linear@openai-curated".to_string(),
            "calendar@openai-curated".to_string(),
        ]
    );

    let curated_marketplace = response
        .marketplaces
        .into_iter()
        .find(|marketplace| marketplace.name == "openai-curated")
        .expect("expected openai-curated marketplace entry");
    assert_eq!(
        curated_marketplace
            .plugins
            .into_iter()
            .map(|plugin| (plugin.id, plugin.installed, plugin.enabled))
            .collect::<Vec<_>>(),
        vec![
            ("linear@openai-curated".to_string(), true, true),
            ("gmail@openai-curated".to_string(), false, false),
            ("calendar@openai-curated".to_string(), false, false),
        ]
    );

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(config.contains(r#"[plugins."linear@openai-curated"]"#));
    assert!(!config.contains(r#"[plugins."gmail@openai-curated"]"#));
    assert!(!config.contains(r#"[plugins."calendar@openai-curated"]"#));

    assert!(
        codex_home
            .path()
            .join("plugins/cache/openai-curated/linear/local")
            .is_dir()
    );
    assert!(
        !codex_home
            .path()
            .join("plugins/cache/openai-curated/gmail")
            .exists()
    );
    assert!(
        !codex_home
            .path()
            .join("plugins/cache/openai-curated/calendar")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn app_server_startup_remote_plugin_sync_runs_once() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    write_plugin_sync_config(codex_home.path(), &format!("{}/backend-api/", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    write_openai_curated_marketplace(codex_home.path(), &["linear"])?;

    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/list"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"[
  {"id":"1","name":"linear","marketplace_name":"openai-curated","version":"1.0.0","enabled":true}
]"#,
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/featured"))
        .and(query_param("platform", "codex"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"["linear@openai-curated"]"#))
        .mount(&server)
        .await;

    let marker_path = codex_home
        .path()
        .join(STARTUP_REMOTE_PLUGIN_SYNC_MARKER_FILE);

    {
        let mut mcp = McpProcess::new(codex_home.path()).await?;
        timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

        wait_for_path_exists(&marker_path).await?;
        wait_for_remote_plugin_request_count(&server, "/plugins/list", /*expected_count*/ 1)
            .await?;
        let request_id = mcp
            .send_plugin_list_request(PluginListParams {
                cwds: None,
                force_remote_sync: false,
            })
            .await?;
        let response: JSONRPCResponse = timeout(
            DEFAULT_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
        )
        .await??;
        let response: PluginListResponse = to_response(response)?;
        let curated_marketplace = response
            .marketplaces
            .into_iter()
            .find(|marketplace| marketplace.name == "openai-curated")
            .expect("expected openai-curated marketplace entry");
        assert_eq!(
            curated_marketplace
                .plugins
                .into_iter()
                .map(|plugin| (plugin.id, plugin.installed, plugin.enabled))
                .collect::<Vec<_>>(),
            vec![("linear@openai-curated".to_string(), true, true)]
        );
        wait_for_remote_plugin_request_count(&server, "/plugins/list", /*expected_count*/ 1)
            .await?;
    }

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(config.contains(r#"[plugins."linear@openai-curated"]"#));

    {
        let mut mcp = McpProcess::new(codex_home.path()).await?;
        timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    }

    tokio::time::sleep(Duration::from_millis(250)).await;
    wait_for_remote_plugin_request_count(&server, "/plugins/list", /*expected_count*/ 1).await?;
    Ok(())
}

#[tokio::test]
async fn plugin_list_fetches_featured_plugin_ids_without_chatgpt_auth() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    write_plugin_sync_config(codex_home.path(), &format!("{}/backend-api/", server.uri()))?;
    write_openai_curated_marketplace(codex_home.path(), &["linear", "gmail"])?;

    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/featured"))
        .and(query_param("platform", "codex"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"["linear@openai-curated"]"#))
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: None,
            force_remote_sync: false,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;

    assert_eq!(
        response.featured_plugin_ids,
        vec!["linear@openai-curated".to_string()]
    );
    assert_eq!(response.remote_sync_error, None);
    Ok(())
}

#[tokio::test]
async fn plugin_list_uses_warmed_featured_plugin_ids_cache_on_first_request() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    write_plugin_sync_config(codex_home.path(), &format!("{}/backend-api/", server.uri()))?;
    write_openai_curated_marketplace(codex_home.path(), &["linear", "gmail"])?;

    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/featured"))
        .and(query_param("platform", "codex"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"["linear@openai-curated"]"#))
        .expect(1)
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    wait_for_featured_plugin_request_count(&server, /*expected_count*/ 1).await?;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: None,
            force_remote_sync: false,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;

    assert_eq!(
        response.featured_plugin_ids,
        vec!["linear@openai-curated".to_string()]
    );
    assert_eq!(response.remote_sync_error, None);
    Ok(())
}

async fn wait_for_featured_plugin_request_count(
    server: &MockServer,
    expected_count: usize,
) -> Result<()> {
    wait_for_remote_plugin_request_count(server, "/plugins/featured", expected_count).await
}

async fn wait_for_remote_plugin_request_count(
    server: &MockServer,
    path_suffix: &str,
    expected_count: usize,
) -> Result<()> {
    timeout(DEFAULT_TIMEOUT, async {
        loop {
            let Some(requests) = server.received_requests().await else {
                bail!("wiremock did not record requests");
            };
            let request_count = requests
                .iter()
                .filter(|request| {
                    request.method == "GET" && request.url.path().ends_with(path_suffix)
                })
                .count();
            if request_count == expected_count {
                return Ok::<(), anyhow::Error>(());
            }
            if request_count > expected_count {
                bail!(
                    "expected exactly {expected_count} {path_suffix} requests, got {request_count}"
                );
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    Ok(())
}

async fn wait_for_path_exists(path: &std::path::Path) -> Result<()> {
    timeout(DEFAULT_TIMEOUT, async {
        loop {
            if path.exists() {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    Ok(())
}

fn write_installed_plugin(
    codex_home: &TempDir,
    marketplace_name: &str,
    plugin_name: &str,
) -> Result<()> {
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join(marketplace_name)
        .join(plugin_name)
        .join("local/.codex-plugin");
    std::fs::create_dir_all(&plugin_root)?;
    std::fs::write(
        plugin_root.join("plugin.json"),
        format!(r#"{{"name":"{plugin_name}"}}"#),
    )?;
    Ok(())
}

fn write_plugin_sync_config(codex_home: &std::path::Path, base_url: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{base_url}"

[features]
plugins = true

[plugins."linear@openai-curated"]
enabled = false

[plugins."gmail@openai-curated"]
enabled = false

[plugins."calendar@openai-curated"]
enabled = true
"#
        ),
    )
}

fn write_openai_curated_marketplace(
    codex_home: &std::path::Path,
    plugin_names: &[&str],
) -> std::io::Result<()> {
    let curated_root = codex_home.join(".tmp/plugins");
    std::fs::create_dir_all(curated_root.join(".git"))?;
    std::fs::create_dir_all(curated_root.join(".agents/plugins"))?;
    let plugins = plugin_names
        .iter()
        .map(|plugin_name| {
            format!(
                r#"{{
      "name": "{plugin_name}",
      "source": {{
        "source": "local",
        "path": "./plugins/{plugin_name}"
      }}
    }}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    std::fs::write(
        curated_root.join(".agents/plugins/marketplace.json"),
        format!(
            r#"{{
  "name": "openai-curated",
  "plugins": [
{plugins}
  ]
}}"#
        ),
    )?;

    for plugin_name in plugin_names {
        let plugin_root = curated_root.join(format!("plugins/{plugin_name}/.codex-plugin"));
        std::fs::create_dir_all(&plugin_root)?;
        std::fs::write(
            plugin_root.join("plugin.json"),
            format!(r#"{{"name":"{plugin_name}"}}"#),
        )?;
    }
    std::fs::create_dir_all(codex_home.join(".tmp"))?;
    std::fs::write(
        codex_home.join(".tmp/plugins.sha"),
        format!("{TEST_CURATED_PLUGIN_SHA}\n"),
    )?;
    Ok(())
}
