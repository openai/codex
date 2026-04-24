use std::borrow::Cow;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::http::header::AUTHORIZATION;
use axum::routing::get;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginReadParams;
use codex_app_server_protocol::PluginReadResponse;
use codex_app_server_protocol::PluginSource;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::Meta;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::model::ToolAnnotations;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn plugin_read_rejects_missing_read_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: None,
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(
        err.error
            .message
            .contains("requires exactly one of marketplacePath or remoteMarketplaceName")
    );
    Ok(())
}

#[tokio::test]
async fn plugin_read_rejects_multiple_read_sources() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                codex_home.path().join("marketplace.json"),
            )?),
            remote_marketplace_name: Some("openai-curated".to_string()),
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(
        err.error
            .message
            .contains("requires exactly one of marketplacePath or remoteMarketplaceName")
    );
    Ok(())
}

#[tokio::test]
async fn plugin_read_rejects_remote_marketplace_when_remote_plugin_is_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some("chatgpt-global".to_string()),
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(
        err.error
            .message
            .contains("remote plugin read is not enabled")
    );
    assert!(err.error.message.contains("chatgpt-global"));
    Ok(())
}

#[tokio::test]
async fn plugin_read_reads_remote_plugin_details_when_remote_plugin_enabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    write_remote_plugin_catalog_config(
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

    let detail_body = r#"{
  "id": "plugins~Plugin_linear",
  "name": "linear",
  "scope": "GLOBAL",
  "installation_policy": "AVAILABLE",
  "authentication_policy": "ON_USE",
  "release": {
    "display_name": "Linear",
    "description": "Track work in Linear",
    "app_ids": [],
    "interface": {
      "short_description": "Plan and track work",
      "capabilities": ["Read", "Write"],
      "logo_url": "https://example.com/linear.png",
      "screenshot_urls": ["https://example.com/linear-shot.png"]
    },
    "skills": [
      {
        "name": "plan-work",
        "description": "Plan work from Linear issues",
        "plugin_release_skill_id": "skill-1",
        "interface": {
          "display_name": "Plan Work",
          "short_description": "Create a plan from issues"
        }
      }
    ]
  }
}"#;
    let installed_body = r#"{
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
        "interface": {
          "short_description": "Plan and track work",
          "capabilities": ["Read", "Write"],
          "logo_url": "https://example.com/linear.png",
          "screenshot_urls": ["https://example.com/linear-shot.png"]
        },
        "skills": [
          {
            "name": "plan-work",
            "description": "Plan work from Linear issues",
            "plugin_release_skill_id": "skill-1",
            "interface": {
              "display_name": "Plan Work",
              "short_description": "Create a plan from issues"
            }
          }
        ]
      },
      "enabled": false,
      "disabled_skill_names": ["plan-work"]
    }
  ],
  "pagination": {
    "limit": 50,
    "next_page_token": null
  }
}"#;

    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/plugins~Plugin_linear"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(detail_body))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .and(query_param("scope", "GLOBAL"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(installed_body))
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some("chatgpt-global".to_string()),
            plugin_name: "plugins~Plugin_linear".to_string(),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginReadResponse = to_response(response)?;

    assert_eq!(response.plugin.marketplace_name, "chatgpt-global");
    assert_eq!(response.plugin.marketplace_path, None);
    assert_eq!(response.plugin.summary.source, PluginSource::Remote);
    assert_eq!(response.plugin.summary.id, "plugins~Plugin_linear");
    assert_eq!(response.plugin.summary.name, "linear");
    assert_eq!(response.plugin.summary.installed, true);
    assert_eq!(response.plugin.summary.enabled, false);
    assert_eq!(
        response.plugin.description.as_deref(),
        Some("Track work in Linear")
    );
    assert_eq!(response.plugin.skills.len(), 1);
    assert_eq!(response.plugin.skills[0].name, "plan-work");
    assert_eq!(response.plugin.skills[0].path, None);
    assert_eq!(response.plugin.skills[0].enabled, false);
    assert_eq!(response.plugin.apps.len(), 0);
    Ok(())
}

#[tokio::test]
async fn plugin_read_maps_missing_remote_plugin_to_invalid_request() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    write_remote_plugin_catalog_config(
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

    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/plugins~Plugin_missing"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"detail":"not found"}"#))
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some("chatgpt-global".to_string()),
            plugin_name: "plugins~Plugin_missing".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(
        err.error
            .message
            .contains("read remote plugin details: remote plugin catalog request")
    );
    Ok(())
}

#[tokio::test]
async fn plugin_read_rejects_remote_marketplace_when_plugins_are_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{}/backend-api/"

[features]
plugins = false
remote_plugin = true
"#,
            server.uri()
        ),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some("chatgpt-global".to_string()),
            plugin_name: "linear".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(
        err.error
            .message
            .contains("remote plugin read is not enabled")
    );
    Ok(())
}

#[tokio::test]
async fn plugin_read_rejects_invalid_remote_plugin_name() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_remote_plugin_catalog_config(codex_home.path(), "https://example.invalid/backend-api/")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some("chatgpt-global".to_string()),
            plugin_name: "linear/../../oops".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(err.error.message.contains("invalid remote plugin id"));
    assert!(
        err.error
            .message
            .contains("only ASCII letters, digits, `_`, `-`, and `~` are allowed")
    );
    Ok(())
}

#[tokio::test]
async fn plugin_read_returns_canonical_openai_curated_marketplace_name() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    write_plugin_marketplace(
        repo_root.path(),
        "openai-curated",
        "demo-plugin",
        "./demo-plugin",
    )?;
    std::fs::create_dir_all(repo_root.path().join("demo-plugin/.codex-plugin"))?;
    std::fs::write(
        repo_root
            .path()
            .join("demo-plugin/.codex-plugin/plugin.json"),
        r#"{
  "name": "demo-plugin",
  "description": "OpenAI curated plugin"
}"#,
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"[features]
plugins = true

[plugins."demo-plugin@openai-curated"]
enabled = true
"#,
    )?;
    write_installed_plugin(&codex_home, "openai-curated", "demo-plugin")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;
    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(marketplace_path.clone()),
            remote_marketplace_name: None,
            plugin_name: "demo-plugin".to_string(),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginReadResponse = to_response(response)?;

    assert_eq!(response.plugin.marketplace_name, "openai-curated");
    assert_eq!(response.plugin.marketplace_path, Some(marketplace_path));
    assert_eq!(response.plugin.summary.id, "demo-plugin@openai-curated");
    assert_eq!(response.plugin.summary.name, "demo-plugin");
    Ok(())
}

#[tokio::test]
async fn plugin_read_returns_plugin_details_with_bundle_contents() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let plugin_root = repo_root.path().join("plugins/demo-plugin");
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(repo_root.path().join(".agents/plugins"))?;
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::create_dir_all(plugin_root.join("skills/thread-summarizer"))?;
    std::fs::create_dir_all(plugin_root.join("skills/chatgpt-only"))?;
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
  "description": "Longer manifest description",
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
      "Draft the reply",
      "Find my next action"
    ],
    "brandColor": "#3B82F6",
    "composerIcon": "./assets/icon.png",
    "logo": "./assets/logo.png",
    "screenshots": ["./assets/screenshot1.png"]
  }
}"##,
    )?;
    std::fs::write(
        plugin_root.join("skills/thread-summarizer/SKILL.md"),
        r#"---
name: thread-summarizer
description: Summarize email threads
---

# Thread Summarizer
"#,
    )?;
    std::fs::write(
        plugin_root.join("skills/chatgpt-only/SKILL.md"),
        r#"---
name: chatgpt-only
description: Visible only for ChatGPT
---

# ChatGPT Only
"#,
    )?;
    std::fs::create_dir_all(plugin_root.join("skills/thread-summarizer/agents"))?;
    std::fs::write(
        plugin_root.join("skills/thread-summarizer/agents/openai.yaml"),
        r#"policy:
  products:
    - CODEX
"#,
    )?;
    std::fs::create_dir_all(plugin_root.join("skills/chatgpt-only/agents"))?;
    std::fs::write(
        plugin_root.join("skills/chatgpt-only/agents/openai.yaml"),
        r#"policy:
  products:
    - CHATGPT
"#,
    )?;
    std::fs::write(
        plugin_root.join(".app.json"),
        r#"{
  "apps": {
    "gmail": {
      "id": "gmail"
    }
  }
}"#,
    )?;
    std::fs::write(
        plugin_root.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "demo": {
      "command": "demo-server"
    }
  }
}"#,
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"[features]
plugins = true

[[skills.config]]
name = "demo-plugin:thread-summarizer"
enabled = false

[plugins."demo-plugin@codex-curated"]
enabled = true
"#,
    )?;
    write_installed_plugin(&codex_home, "codex-curated", "demo-plugin")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;
    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(marketplace_path.clone()),
            remote_marketplace_name: None,
            plugin_name: "demo-plugin".to_string(),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginReadResponse = to_response(response)?;

    assert_eq!(response.plugin.marketplace_name, "codex-curated");
    assert_eq!(response.plugin.marketplace_path, Some(marketplace_path));
    assert_eq!(response.plugin.summary.id, "demo-plugin@codex-curated");
    assert_eq!(response.plugin.summary.name, "demo-plugin");
    assert_eq!(
        response.plugin.description.as_deref(),
        Some("Longer manifest description")
    );
    assert_eq!(response.plugin.summary.installed, true);
    assert_eq!(response.plugin.summary.enabled, true);
    assert_eq!(
        response.plugin.summary.install_policy,
        PluginInstallPolicy::Available
    );
    assert_eq!(
        response.plugin.summary.auth_policy,
        PluginAuthPolicy::OnInstall
    );
    assert_eq!(
        response
            .plugin
            .summary
            .interface
            .as_ref()
            .and_then(|interface| interface.display_name.as_deref()),
        Some("Plugin Display Name")
    );
    assert_eq!(
        response
            .plugin
            .summary
            .interface
            .as_ref()
            .and_then(|interface| interface.category.as_deref()),
        Some("Design")
    );
    assert_eq!(
        response
            .plugin
            .summary
            .interface
            .as_ref()
            .and_then(|interface| interface.default_prompt.clone()),
        Some(vec![
            "Draft the reply".to_string(),
            "Find my next action".to_string()
        ])
    );
    assert_eq!(response.plugin.skills.len(), 1);
    assert_eq!(
        response.plugin.skills[0].name,
        "demo-plugin:thread-summarizer"
    );
    assert_eq!(
        response.plugin.skills[0].description,
        "Summarize email threads"
    );
    assert!(!response.plugin.skills[0].enabled);
    assert_eq!(response.plugin.apps.len(), 1);
    assert_eq!(response.plugin.apps[0].id, "gmail");
    assert_eq!(response.plugin.apps[0].name, "gmail");
    assert_eq!(
        response.plugin.apps[0].install_url.as_deref(),
        Some("https://chatgpt.com/apps/gmail/gmail")
    );
    assert_eq!(response.plugin.apps[0].needs_auth, true);
    assert_eq!(response.plugin.mcp_servers.len(), 1);
    assert_eq!(response.plugin.mcp_servers[0], "demo");
    Ok(())
}

#[tokio::test]
async fn plugin_read_returns_app_needs_auth() -> Result<()> {
    let connectors = vec![
        AppInfo {
            id: "alpha".to_string(),
            name: "Alpha".to_string(),
            description: Some("Alpha connector".to_string()),
            logo_url: Some("https://example.com/alpha.png".to_string()),
            logo_url_dark: None,
            distribution_channel: Some("featured".to_string()),
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        },
        AppInfo {
            id: "beta".to_string(),
            name: "Beta".to_string(),
            description: Some("Beta connector".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        },
    ];
    let tools = vec![connector_tool("beta", "Beta App")?];
    let (server_url, server_handle) = start_apps_server(connectors, tools).await?;

    let codex_home = TempDir::new()?;
    write_connectors_config(codex_home.path(), &server_url)?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let repo_root = TempDir::new()?;
    write_plugin_marketplace(
        repo_root.path(),
        "debug",
        "sample-plugin",
        "./sample-plugin",
    )?;
    write_plugin_source(repo_root.path(), "sample-plugin", &["alpha", "beta"])?;
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginReadResponse = to_response(response)?;

    assert_eq!(
        response
            .plugin
            .apps
            .iter()
            .map(|app| (app.id.as_str(), app.needs_auth))
            .collect::<Vec<_>>(),
        vec![("alpha", true), ("beta", false)]
    );

    server_handle.abort();
    let _ = server_handle.await;
    Ok(())
}

#[tokio::test]
async fn plugin_read_accepts_legacy_string_default_prompt() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let plugin_root = repo_root.path().join("plugins/demo-plugin");
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(repo_root.path().join(".agents/plugins"))?;
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
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
    write_plugins_enabled_config(&codex_home)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                repo_root.path().join(".agents/plugins/marketplace.json"),
            )?),
            remote_marketplace_name: None,
            plugin_name: "demo-plugin".to_string(),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginReadResponse = to_response(response)?;

    assert_eq!(
        response
            .plugin
            .summary
            .interface
            .as_ref()
            .and_then(|interface| interface.default_prompt.clone()),
        Some(vec!["Starter prompt for trying a plugin".to_string()])
    );
    Ok(())
}

#[tokio::test]
async fn plugin_read_describes_uninstalled_git_source_without_cloning() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let missing_remote_repo = repo_root.path().join("missing-remote-plugin-repo");
    let missing_remote_repo_url = url::Url::from_directory_path(&missing_remote_repo)
        .unwrap()
        .to_string();
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(repo_root.path().join(".agents/plugins"))?;
    std::fs::write(
        repo_root.path().join(".agents/plugins/marketplace.json"),
        format!(
            r#"{{
  "name": "debug",
  "plugins": [
    {{
      "name": "toolkit",
      "source": {{
        "source": "git-subdir",
        "url": "{missing_remote_repo_url}",
        "path": "plugins/toolkit"
      }}
    }}
  ]
}}"#
        ),
    )?;
    write_plugins_enabled_config(&codex_home)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                repo_root.path().join(".agents/plugins/marketplace.json"),
            )?),
            remote_marketplace_name: None,
            plugin_name: "toolkit".to_string(),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginReadResponse = to_response(response)?;

    let expected_description = format!(
        "This is a cross-repo plugin. Install it to view more detailed information. The source of the plugin is {missing_remote_repo_url}, path `plugins/toolkit`."
    );
    assert_eq!(
        response.plugin.description.as_deref(),
        Some(expected_description.as_str())
    );
    assert!(!response.plugin.summary.installed);
    assert!(response.plugin.skills.is_empty());
    assert!(response.plugin.apps.is_empty());
    assert!(response.plugin.mcp_servers.is_empty());
    assert!(
        !codex_home
            .path()
            .join("plugins/.marketplace-plugin-source-staging")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn plugin_read_returns_invalid_request_when_plugin_is_missing() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(repo_root.path().join(".agents/plugins"))?;
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
    write_plugins_enabled_config(&codex_home)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                repo_root.path().join(".agents/plugins/marketplace.json"),
            )?),
            remote_marketplace_name: None,
            plugin_name: "missing-plugin".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(
        err.error
            .message
            .contains("plugin `missing-plugin` was not found")
    );
    Ok(())
}

#[tokio::test]
async fn plugin_read_returns_invalid_request_when_plugin_manifest_is_missing() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let plugin_root = repo_root.path().join("plugins/demo-plugin");
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(repo_root.path().join(".agents/plugins"))?;
    std::fs::create_dir_all(&plugin_root)?;
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
    write_plugins_enabled_config(&codex_home)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                repo_root.path().join(".agents/plugins/marketplace.json"),
            )?),
            remote_marketplace_name: None,
            plugin_name: "demo-plugin".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(err.error.message.contains("missing or invalid plugin.json"));
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

fn write_plugins_enabled_config(codex_home: &TempDir) -> Result<()> {
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"[features]
plugins = true
"#,
    )?;
    Ok(())
}

#[derive(Clone)]
struct AppsServerState {
    response: Arc<StdMutex<serde_json::Value>>,
}

#[derive(Clone)]
struct PluginReadMcpServer {
    tools: Arc<StdMutex<Vec<Tool>>>,
}

impl ServerHandler for PluginReadMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_
    {
        let tools = self.tools.clone();
        async move {
            let tools = tools
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            Ok(ListToolsResult {
                tools,
                next_cursor: None,
                meta: None,
            })
        }
    }
}

async fn start_apps_server(
    connectors: Vec<AppInfo>,
    tools: Vec<Tool>,
) -> Result<(String, JoinHandle<()>)> {
    let state = Arc::new(AppsServerState {
        response: Arc::new(StdMutex::new(
            json!({ "apps": connectors, "next_token": null }),
        )),
    });
    let tools = Arc::new(StdMutex::new(tools));

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let mcp_service = StreamableHttpService::new(
        {
            let tools = tools.clone();
            move || {
                Ok(PluginReadMcpServer {
                    tools: tools.clone(),
                })
            }
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let router = Router::new()
        .route("/connectors/directory/list", get(list_directory_connectors))
        .route(
            "/connectors/directory/list_workspace",
            get(list_directory_connectors),
        )
        .with_state(state)
        .nest_service("/api/codex/apps", mcp_service);

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Ok((format!("http://{addr}"), handle))
}

async fn list_directory_connectors(
    State(state): State<Arc<AppsServerState>>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    let bearer_ok = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "Bearer chatgpt-token");
    let account_ok = headers
        .get("chatgpt-account-id")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "account-123");
    let external_logos_ok = uri
        .query()
        .is_some_and(|query| query.split('&').any(|pair| pair == "external_logos=true"));

    if !bearer_ok || !account_ok {
        Err(StatusCode::UNAUTHORIZED)
    } else if !external_logos_ok {
        Err(StatusCode::BAD_REQUEST)
    } else {
        let response = state
            .response
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        Ok(Json(response))
    }
}

fn connector_tool(connector_id: &str, connector_name: &str) -> Result<Tool> {
    let schema: JsonObject = serde_json::from_value(json!({
        "type": "object",
        "additionalProperties": false
    }))?;
    let mut tool = Tool::new(
        Cow::Owned(format!("connector_{connector_id}")),
        Cow::Borrowed("Connector test tool"),
        Arc::new(schema),
    );
    tool.annotations = Some(ToolAnnotations::new().read_only(true));

    let mut meta = Meta::new();
    meta.0
        .insert("connector_id".to_string(), json!(connector_id));
    meta.0
        .insert("connector_name".to_string(), json!(connector_name));
    tool.meta = Some(meta);
    Ok(tool)
}

fn write_connectors_config(codex_home: &std::path::Path, base_url: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{base_url}"
mcp_oauth_credentials_store = "file"

[features]
plugins = true
connectors = true
"#
        ),
    )
}

fn write_remote_plugin_catalog_config(
    codex_home: &std::path::Path,
    base_url: &str,
) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{base_url}"

[features]
plugins = true
remote_plugin = true
"#
        ),
    )
}

fn write_plugin_marketplace(
    repo_root: &std::path::Path,
    marketplace_name: &str,
    plugin_name: &str,
    source_path: &str,
) -> std::io::Result<()> {
    std::fs::create_dir_all(repo_root.join(".git"))?;
    std::fs::create_dir_all(repo_root.join(".agents/plugins"))?;
    std::fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        format!(
            r#"{{
  "name": "{marketplace_name}",
  "plugins": [
    {{
      "name": "{plugin_name}",
      "source": {{
        "source": "local",
        "path": "{source_path}"
      }}
    }}
  ]
}}"#
        ),
    )
}

fn write_plugin_source(
    repo_root: &std::path::Path,
    plugin_name: &str,
    app_ids: &[&str],
) -> Result<()> {
    let plugin_root = repo_root.join(plugin_name);
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        format!(r#"{{"name":"{plugin_name}"}}"#),
    )?;

    let apps = app_ids
        .iter()
        .map(|app_id| ((*app_id).to_string(), json!({ "id": app_id })))
        .collect::<serde_json::Map<_, _>>();
    std::fs::write(
        plugin_root.join(".app.json"),
        serde_json::to_vec_pretty(&json!({ "apps": apps }))?,
    )?;
    Ok(())
}
