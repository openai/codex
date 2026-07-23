use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::routing::post;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::AppMetadata;
use codex_app_server_protocol::AppTemplateSummary;
use codex_app_server_protocol::AppTemplateUnavailableReason;
use codex_app_server_protocol::AppsReadParams;
use codex_app_server_protocol::AppsReadResponse;
use codex_app_server_protocol::HookEventName;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginInstallPolicySource;
use codex_app_server_protocol::PluginReadParams;
use codex_app_server_protocol::PluginReadResponse;
use codex_app_server_protocol::PluginShareDiscoverability;
use codex_app_server_protocol::PluginSharePrincipal;
use codex_app_server_protocol::PluginSharePrincipalRole;
use codex_app_server_protocol::PluginSharePrincipalType;
use codex_app_server_protocol::PluginSkillReadParams;
use codex_app_server_protocol::PluginSkillReadResponse;
use codex_app_server_protocol::PluginSource;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ScheduledTaskSchedule;
use codex_app_server_protocol::ScheduledTaskSummary;
use codex_app_server_protocol::ScheduledTaskWeekday;
use codex_app_server_protocol::SkillInterface;
use codex_config::types::AuthCredentialsStoreMode;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn plugin_read_rejects_missing_read_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

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
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                codex_home.path().join("marketplace.json"),
            )?),
            remote_marketplace_name: Some("openai-curated-remote".to_string()),
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
async fn plugin_read_returns_remote_mcp_servers_when_uninstalled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{}/backend-api/"

[features]
plugins = true
apps = true
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

    let detail_body = r#"{
  "id": "plugins~Plugin_00000000000000000000000000000000",
  "name": "example-plugin",
  "scope": "GLOBAL",
  "installation_policy": "AVAILABLE",
  "installation_policy_source": "IMPLICIT_CANONICAL_APP",
  "must_show_installation_interstitial": true,
  "authentication_policy": "ON_USE",
  "release": {
    "version": "1.2.1",
    "display_name": "Example Plugin",
    "description": "Example plugin",
    "app_ids": [],
    "app_manifest": {
      "apps": {
        "example-server": {
          "id": "example-app"
        }
      }
    },
    "keywords": [],
    "interface": {
      "short_description": "Example plugin",
      "capabilities": [],
      "default_prompt": "Use the legacy example prompt",
      "default_prompts": [],
      "logo_url_dark": "https://example.com/example-plugin-dark.png"
    },
    "skills": [],
    "scheduled_tasks": [
      {
        "key": "weekday-triage",
        "name": "Weekday triage",
        "prompt": "Triage the support queue.",
        "schedule": {
          "type": "weekdays",
          "time": "08:30"
        }
      },
      {
        "key": "queue-monitor",
        "name": "Queue monitor",
        "prompt": "Check the queue.",
        "schedule": {
          "type": "hourly",
          "intervalHours": 2,
          "days": ["MO", "WE", "FR"]
        }
      }
    ],
    "mcp_servers": [
      {
        "key": "example-server",
        "metadata": {
          "command": "example-mcp"
        }
      },
      {
        "key": "other-server",
        "metadata": {
          "command": "other-mcp"
        }
      }
    ]
  }
}"#;
    let installed_body = r#"{
  "plugins": [],
  "pagination": {
    "limit": 50,
    "next_page_token": null
  }
}"#;

    Mock::given(method("GET"))
        .and(path(
            "/backend-api/ps/plugins/plugins~Plugin_00000000000000000000000000000000",
        ))
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
    Mock::given(method("POST"))
        .and(path("/backend-api/ps/apps/batch"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .and(header("oai-product-sku", "codex"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "apps": [{
                "id": "example-app",
                "name": "Example App",
                "description": "Example app connector",
                "icon_url": "https://example.com/example.png",
                "tools": null
            }]
        })))
        .mount(&server)
        .await;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some("openai-curated-remote".to_string()),
            plugin_name: "plugins~Plugin_00000000000000000000000000000000".to_string(),
        })
        .await?;

    let response: PluginReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    assert_eq!(response.plugin.marketplace_name, "openai-curated-remote");
    assert_eq!(
        response.plugin.summary.id,
        "example-plugin@openai-curated-remote"
    );
    assert_eq!(
        response.plugin.summary.remote_plugin_id.as_deref(),
        Some("plugins~Plugin_00000000000000000000000000000000")
    );
    assert_eq!(response.plugin.summary.name, "example-plugin");
    assert_eq!(response.plugin.summary.source, PluginSource::Remote);
    assert_eq!(response.plugin.summary.share_context, None);
    assert_eq!(
        response.plugin.summary.install_policy_source,
        Some(PluginInstallPolicySource::ImplicitCanonicalApp)
    );
    assert_eq!(
        response.plugin.summary.must_show_installation_interstitial,
        Some(true)
    );
    assert_eq!(
        response
            .plugin
            .summary
            .interface
            .as_ref()
            .and_then(|interface| interface.default_prompt.clone()),
        Some(vec!["Use the legacy example prompt".to_string()])
    );
    assert_eq!(
        response
            .plugin
            .summary
            .interface
            .as_ref()
            .and_then(|interface| interface.logo_url_dark.as_deref()),
        Some("https://example.com/example-plugin-dark.png")
    );
    assert_eq!(
        response.plugin.mcp_servers,
        vec!["other-server".to_string()]
    );
    assert_eq!(
        response.plugin.scheduled_tasks,
        Some(vec![
            ScheduledTaskSummary {
                key: "weekday-triage".to_string(),
                name: "Weekday triage".to_string(),
                prompt: "Triage the support queue.".to_string(),
                schedule: ScheduledTaskSchedule::Weekdays {
                    time: "08:30".to_string(),
                },
            },
            ScheduledTaskSummary {
                key: "queue-monitor".to_string(),
                name: "Queue monitor".to_string(),
                prompt: "Check the queue.".to_string(),
                schedule: ScheduledTaskSchedule::Hourly {
                    interval_hours: 2,
                    days: Some(vec![
                        ScheduledTaskWeekday::Mo,
                        ScheduledTaskWeekday::We,
                        ScheduledTaskWeekday::Fr,
                    ]),
                },
            },
        ])
    );
    assert_eq!(
        response
            .plugin
            .apps
            .iter()
            .map(|app| app.id.as_str())
            .collect::<Vec<_>>(),
        vec!["example-app"]
    );
    Ok(())
}

#[tokio::test]
async fn plugin_read_returns_share_context_for_shared_remote_plugin() -> Result<()> {
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
  "id": "plugins~Plugin_11111111111111111111111111111111",
  "name": "shared-linear",
  "scope": "WORKSPACE",
  "discoverability": "PRIVATE",
  "creator_account_user_id": "user-gavin__account-123",
  "creator_name": "Gavin",
  "share_url": "https://chatgpt.example/plugins/share/share-key-1",
  "share_principals": [
    {
      "principal_type": "user",
      "principal_id": "user-gavin__account-123",
      "role": "owner",
      "name": "Gavin"
    },
    {
      "principal_type": "user",
      "principal_id": "user-ada__account-123",
      "role": "reader",
      "name": "Ada"
    }
  ],
  "installation_policy": "AVAILABLE",
  "authentication_policy": "ON_USE",
  "release": {
    "version": "2.3.4",
    "display_name": "Shared Linear",
    "description": "Track shared work",
    "app_ids": [],
    "keywords": [],
    "interface": {},
    "skills": []
  }
}"#;
    let installed_body = r#"{
  "plugins": [],
  "pagination": {
    "limit": 50,
    "next_page_token": null
  }
}"#;

    Mock::given(method("GET"))
        .and(path(
            "/backend-api/ps/plugins/plugins~Plugin_11111111111111111111111111111111",
        ))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(detail_body))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .and(query_param("scope", "WORKSPACE"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(installed_body))
        .mount(&server)
        .await;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    for remote_marketplace_name in [
        "workspace-shared-with-me-private",
        "workspace-shared-with-me",
    ] {
        let request_id = mcp
            .send_plugin_read_request(PluginReadParams {
                marketplace_path: None,
                remote_marketplace_name: Some(remote_marketplace_name.to_string()),
                plugin_name: "plugins~Plugin_11111111111111111111111111111111".to_string(),
            })
            .await?;

        let response: PluginReadResponse =
            timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

        assert_eq!(response.plugin.marketplace_name, "workspace-shared-with-me");
        assert_eq!(
            response.plugin.summary.id,
            "shared-linear@workspace-shared-with-me"
        );
        assert_eq!(
            response.plugin.summary.remote_plugin_id.as_deref(),
            Some("plugins~Plugin_11111111111111111111111111111111")
        );
        let share_context = response
            .plugin
            .summary
            .share_context
            .as_ref()
            .expect("expected share context");
        assert_eq!(
            share_context.remote_plugin_id,
            "plugins~Plugin_11111111111111111111111111111111"
        );
        assert_eq!(share_context.remote_version.as_deref(), Some("2.3.4"));
        assert_eq!(
            share_context.discoverability,
            Some(PluginShareDiscoverability::Private)
        );
        assert_eq!(
            share_context.creator_account_user_id.as_deref(),
            Some("user-gavin__account-123")
        );
        assert_eq!(share_context.creator_name.as_deref(), Some("Gavin"));
        assert_eq!(
            share_context.share_url.as_deref(),
            Some("https://chatgpt.example/plugins/share/share-key-1")
        );
        assert_eq!(
            share_context.share_principals,
            Some(vec![
                PluginSharePrincipal {
                    principal_type: PluginSharePrincipalType::User,
                    principal_id: "user-gavin__account-123".to_string(),
                    role: PluginSharePrincipalRole::Owner,
                    name: "Gavin".to_string(),
                },
                PluginSharePrincipal {
                    principal_type: PluginSharePrincipalType::User,
                    principal_id: "user-ada__account-123".to_string(),
                    role: PluginSharePrincipalRole::Reader,
                    name: "Ada".to_string(),
                },
            ])
        );
    }
    Ok(())
}

#[tokio::test]
async fn plugin_read_includes_share_url_for_admin_disabled_remote_plugin() -> Result<()> {
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
  "id": "plugins~Plugin_00000000000000000000000000000000",
  "name": "example-plugin",
  "scope": "GLOBAL",
  "share_url": "https://chatgpt.example/plugins/share/example-plugin",
  "status": "DISABLED_BY_ADMIN",
  "installation_policy": "AVAILABLE",
  "authentication_policy": "ON_USE",
  "release": {
    "display_name": "Example Plugin",
    "description": "Exercise example workflows",
    "app_ids": [],
    "app_templates": [
      {
        "template_id": "templated_apps_SourceControlEnterprise",
        "name": "Source Control Enterprise",
        "description": "Connect source control",
        "category": "Developer Tools",
        "canonical_connector_id": "source_control_enterprise",
        "logo_url": "https://example.com/source-control-light.png",
        "logo_url_dark": "https://example.com/source-control-dark.png",
        "materialized_app_ids": ["asdk_app_source_control"],
        "reason": null
      },
      {
        "template_id": "templated_apps_DataWarehouse",
        "name": "Data Warehouse",
        "description": null,
        "canonical_connector_id": null,
        "logo_url": null,
        "logo_url_dark": null,
        "materialized_app_ids": [],
        "reason": "NOT_CONFIGURED_FOR_WORKSPACE"
      }
    ],
    "keywords": ["workflow", "example"],
    "interface": {
      "short_description": "Run example workflows",
      "capabilities": ["Read", "Write"],
      "default_prompt": "Use the legacy example prompt",
      "default_prompts": ["Create an example item", "Review example projects"],
      "logo_url": "https://example.com/example-plugin.png",
      "screenshot_urls": ["https://example.com/example-plugin-shot.png"]
    },
    "skills": [
      {
        "name": "plan-work",
        "description": "Plan example work",
        "plugin_release_skill_id": "skill-1",
        "interface": {
          "display_name": "Plan Work",
          "short_description": "Create a plan from issues",
          "icon_small_url": "https://example.com/plan-work-small.svg",
          "icon_large_url": "https://example.com/plan-work-large.png"
        }
      }
    ]
  }
}"#;
    let installed_body = r#"{
  "plugins": [
    {
      "id": "plugins~Plugin_00000000000000000000000000000000",
      "name": "example-plugin",
      "scope": "GLOBAL",
      "installation_policy": "AVAILABLE",
      "authentication_policy": "ON_USE",
      "release": {
        "display_name": "Example Plugin",
        "description": "Exercise example workflows",
        "app_ids": [],
        "interface": {
          "short_description": "Run example workflows",
          "capabilities": ["Read", "Write"],
          "logo_url": "https://example.com/example-plugin.png",
          "screenshot_urls": ["https://example.com/example-plugin-shot.png"]
        },
        "skills": [
          {
            "name": "plan-work",
            "description": "Plan example work",
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
        .and(path(
            "/backend-api/ps/plugins/plugins~Plugin_00000000000000000000000000000000",
        ))
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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some("openai-curated-remote".to_string()),
            plugin_name: "plugins~Plugin_00000000000000000000000000000000".to_string(),
        })
        .await?;

    let response: PluginReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    assert_eq!(response.plugin.marketplace_name, "openai-curated-remote");
    assert_eq!(response.plugin.marketplace_path, None);
    assert_eq!(response.plugin.summary.source, PluginSource::Remote);
    assert_eq!(
        response.plugin.summary.id,
        "example-plugin@openai-curated-remote"
    );
    assert_eq!(
        response.plugin.summary.remote_plugin_id.as_deref(),
        Some("plugins~Plugin_00000000000000000000000000000000")
    );
    assert_eq!(response.plugin.summary.name, "example-plugin");
    assert_eq!(response.plugin.summary.installed, true);
    assert_eq!(response.plugin.summary.enabled, false);
    assert_eq!(
        response.plugin.summary.availability,
        PluginAvailability::DisabledByAdmin
    );
    assert_eq!(response.plugin.summary.share_context, None);
    assert_eq!(
        response.plugin.share_url.as_deref(),
        Some("https://chatgpt.example/plugins/share/example-plugin")
    );
    assert_eq!(
        response.plugin.description.as_deref(),
        Some("Exercise example workflows")
    );
    assert_eq!(
        response.plugin.summary.keywords,
        vec!["workflow".to_string(), "example".to_string()]
    );
    assert_eq!(
        response
            .plugin
            .summary
            .interface
            .as_ref()
            .and_then(|interface| interface.default_prompt.clone()),
        Some(vec![
            "Create an example item".to_string(),
            "Review example projects".to_string(),
        ])
    );
    assert_eq!(response.plugin.skills.len(), 1);
    assert_eq!(response.plugin.skills[0].name, "plan-work");
    assert_eq!(response.plugin.skills[0].path, None);
    assert_eq!(response.plugin.skills[0].enabled, false);
    assert_eq!(
        response.plugin.skills[0].interface,
        Some(SkillInterface {
            display_name: Some("Plan Work".to_string()),
            short_description: Some("Create a plan from issues".to_string()),
            icon_small: None,
            icon_large: None,
            icon_small_url: Some("https://example.com/plan-work-small.svg".to_string()),
            icon_large_url: Some("https://example.com/plan-work-large.png".to_string()),
            brand_color: None,
            default_prompt: None,
        })
    );
    assert_eq!(response.plugin.apps.len(), 0);
    assert_eq!(
        response.plugin.app_templates,
        vec![
            AppTemplateSummary {
                template_id: "templated_apps_SourceControlEnterprise".to_string(),
                name: "Source Control Enterprise".to_string(),
                description: Some("Connect source control".to_string()),
                category: Some("Developer Tools".to_string()),
                canonical_connector_id: Some("source_control_enterprise".to_string()),
                logo_url: Some("https://example.com/source-control-light.png".to_string()),
                logo_url_dark: Some("https://example.com/source-control-dark.png".to_string()),
                materialized_app_ids: vec!["asdk_app_source_control".to_string()],
                reason: None,
            },
            AppTemplateSummary {
                template_id: "templated_apps_DataWarehouse".to_string(),
                name: "Data Warehouse".to_string(),
                description: None,
                category: None,
                canonical_connector_id: None,
                logo_url: None,
                logo_url_dark: None,
                materialized_app_ids: Vec::new(),
                reason: Some(AppTemplateUnavailableReason::NotConfiguredForWorkspace),
            },
        ]
    );
    Ok(())
}

#[tokio::test]
async fn plugin_skill_read_reads_remote_skill_contents_when_remote_plugin_enabled() -> Result<()> {
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

    let skill_body = r##"{
  "plugin_id": "plugins~Plugin_00000000000000000000000000000000",
  "status": "ENABLED",
  "plugin_release_id": "release-1",
  "name": "plan-work",
  "description": "Plan work from Linear issues",
  "plugin_release_skill_id": "skill-1",
  "skill_md_contents": "# Plan Work\n\nUse Linear issues to create a plan."
}"##;

    Mock::given(method("GET"))
        .and(path(
            "/backend-api/ps/plugins/plugins~Plugin_00000000000000000000000000000000/skills/plan-work",
        ))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(skill_body))
        .mount(&server)
        .await;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_skill_read_request(PluginSkillReadParams {
            remote_marketplace_name: "openai-curated-remote".to_string(),
            remote_plugin_id: "plugins~Plugin_00000000000000000000000000000000".to_string(),
            skill_name: "plan-work".to_string(),
        })
        .await?;

    let response: PluginSkillReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    assert_eq!(
        response,
        PluginSkillReadResponse {
            contents: Some("# Plan Work\n\nUse Linear issues to create a plan.".to_string()),
        }
    );
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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some("openai-curated-remote".to_string()),
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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some("openai-curated-remote".to_string()),
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
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some("openai-curated-remote".to_string()),
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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

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
    assert_eq!(response.result["plugin"]["scheduledTasks"], json!(null));
    let response: PluginReadResponse = to_response(response)?;

    assert_eq!(response.plugin.marketplace_name, "openai-curated");
    assert_eq!(response.plugin.marketplace_path, Some(marketplace_path));
    assert_eq!(response.plugin.summary.id, "demo-plugin@openai-curated");
    assert_eq!(response.plugin.summary.name, "demo-plugin");
    Ok(())
}

#[tokio::test]
async fn plugin_read_returns_share_context_for_shared_local_plugin() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
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
    write_plugin_marketplace(
        repo_root.path(),
        "codex-curated",
        "demo-plugin",
        "./demo-plugin",
    )?;
    std::fs::create_dir_all(repo_root.path().join("demo-plugin/.codex-plugin"))?;
    std::fs::write(
        repo_root
            .path()
            .join("demo-plugin/.codex-plugin/plugin.json"),
        r#"{"name":"demo-plugin","version":"1.2.3"}"#,
    )?;
    std::fs::write(
        repo_root.path().join("demo-plugin/.mcp.json"),
        r#"{"mcpServers":{"demo":{"command":"demo-mcp"}}}"#,
    )?;
    let plugin_path = AbsolutePathBuf::try_from(repo_root.path().join("demo-plugin"))?;
    write_plugin_share_local_path_mapping(codex_home.path(), "plugins_123", &plugin_path)?;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/plugins_123"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "plugins_123",
            "name": "demo-plugin",
            "scope": "WORKSPACE",
            "discoverability": "UNLISTED",
            "creator_account_user_id": "user-owner__account-123",
            "creator_name": "Owner",
            "share_url": "https://chatgpt.example/plugins/share/share-key-1",
            "share_principals": [
                {
                    "principal_type": "user",
                    "principal_id": "user-owner__account-123",
                    "role": "owner",
                    "name": "Owner",
                },
                {
                    "principal_type": "user",
                    "principal_id": "user-editor__account-123",
                    "role": "editor",
                    "name": "Editor",
                },
            ],
            "installation_policy": "AVAILABLE",
            "authentication_policy": "ON_USE",
            "release": {
                "version": "1.2.4",
                "display_name": "Demo Plugin",
                "description": "Shared local plugin",
                "app_ids": [],
                "keywords": [],
                "interface": {},
                "skills": []
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                repo_root.path().join(".agents/plugins/marketplace.json"),
            )?),
            remote_marketplace_name: None,
            plugin_name: "demo-plugin".to_string(),
        })
        .await?;

    let response: PluginReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    assert_eq!(response.plugin.summary.remote_plugin_id, None);
    assert_eq!(
        response.plugin.summary.local_version.as_deref(),
        Some("1.2.3")
    );
    let share_context = response
        .plugin
        .summary
        .share_context
        .as_ref()
        .expect("expected share context");
    assert_eq!(share_context.remote_plugin_id, "plugins_123");
    assert_eq!(share_context.remote_version.as_deref(), Some("1.2.4"));
    assert_eq!(
        share_context.discoverability,
        Some(PluginShareDiscoverability::Unlisted)
    );
    assert_eq!(
        share_context.share_url.as_deref(),
        Some("https://chatgpt.example/plugins/share/share-key-1")
    );
    assert_eq!(
        share_context.creator_account_user_id.as_deref(),
        Some("user-owner__account-123")
    );
    assert_eq!(share_context.creator_name.as_deref(), Some("Owner"));
    assert_eq!(
        share_context.share_principals,
        Some(vec![
            PluginSharePrincipal {
                principal_type: PluginSharePrincipalType::User,
                principal_id: "user-owner__account-123".to_string(),
                role: PluginSharePrincipalRole::Owner,
                name: "Owner".to_string(),
            },
            PluginSharePrincipal {
                principal_type: PluginSharePrincipalType::User,
                principal_id: "user-editor__account-123".to_string(),
                role: PluginSharePrincipalRole::Editor,
                name: "Editor".to_string(),
            },
        ])
    );
    Ok(())
}

#[tokio::test]
async fn plugin_read_keeps_remote_version_when_share_principals_are_missing() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
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
    write_plugin_marketplace(
        repo_root.path(),
        "codex-curated",
        "demo-plugin",
        "./demo-plugin",
    )?;
    std::fs::create_dir_all(repo_root.path().join("demo-plugin/.codex-plugin"))?;
    std::fs::write(
        repo_root
            .path()
            .join("demo-plugin/.codex-plugin/plugin.json"),
        r#"{"name":"demo-plugin","version":"1.2.3"}"#,
    )?;
    std::fs::write(
        repo_root.path().join("demo-plugin/.mcp.json"),
        r#"{"mcpServers":{"demo":{"command":"demo-mcp"}}}"#,
    )?;
    let plugin_path = AbsolutePathBuf::try_from(repo_root.path().join("demo-plugin"))?;
    write_plugin_share_local_path_mapping(codex_home.path(), "plugins_123", &plugin_path)?;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/plugins_123"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "plugins_123",
            "name": "demo-plugin",
            "scope": "WORKSPACE",
            "discoverability": "UNLISTED",
            "creator_account_user_id": "user-owner__account-123",
            "creator_name": "Owner",
            "share_url": "https://chatgpt.example/plugins/share/share-key-1",
            "share_principals": null,
            "installation_policy": "AVAILABLE",
            "authentication_policy": "ON_USE",
            "release": {
                "version": "1.2.4",
                "display_name": "Demo Plugin",
                "description": "Shared local plugin",
                "app_ids": [],
                "keywords": [],
                "interface": {},
                "skills": []
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                repo_root.path().join(".agents/plugins/marketplace.json"),
            )?),
            remote_marketplace_name: None,
            plugin_name: "demo-plugin".to_string(),
        })
        .await?;

    let response: PluginReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    assert_eq!(response.plugin.summary.remote_plugin_id, None);
    assert_eq!(
        response.plugin.summary.local_version.as_deref(),
        Some("1.2.3")
    );
    let share_context = response
        .plugin
        .summary
        .share_context
        .as_ref()
        .expect("expected share context");
    assert_eq!(share_context.remote_plugin_id, "plugins_123");
    assert_eq!(share_context.remote_version.as_deref(), Some("1.2.4"));
    assert_eq!(share_context.discoverability, None);
    assert_eq!(share_context.share_url, None);
    assert_eq!(share_context.creator_account_user_id, None);
    assert_eq!(share_context.creator_name, None);
    assert_eq!(share_context.share_principals, None);
    Ok(())
}

#[tokio::test]
async fn plugin_read_falls_back_to_local_share_context_without_remote_auth() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    write_plugins_enabled_config(&codex_home)?;
    write_plugin_marketplace(
        repo_root.path(),
        "codex-curated",
        "demo-plugin",
        "./demo-plugin",
    )?;
    write_plugin_source(repo_root.path(), "demo-plugin", &[])?;
    let plugin_path = AbsolutePathBuf::try_from(repo_root.path().join("demo-plugin"))?;
    write_plugin_share_local_path_mapping(codex_home.path(), "plugins_123", &plugin_path)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                repo_root.path().join(".agents/plugins/marketplace.json"),
            )?),
            remote_marketplace_name: None,
            plugin_name: "demo-plugin".to_string(),
        })
        .await?;

    let response: PluginReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    assert_eq!(response.plugin.summary.remote_plugin_id, None);
    assert_eq!(response.plugin.summary.local_version, None);
    let share_context = response
        .plugin
        .summary
        .share_context
        .as_ref()
        .expect("expected share context");
    assert_eq!(share_context.remote_plugin_id, "plugins_123");
    assert_eq!(share_context.remote_version, None);
    assert_eq!(share_context.discoverability, None);
    assert_eq!(share_context.share_url, None);
    assert_eq!(share_context.creator_account_user_id, None);
    assert_eq!(share_context.creator_name, None);
    assert_eq!(share_context.share_principals, None);
    Ok(())
}

#[tokio::test]
async fn plugin_read_fails_on_malformed_share_mapping() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    write_plugins_enabled_config(&codex_home)?;
    write_plugin_marketplace(
        repo_root.path(),
        "codex-curated",
        "demo-plugin",
        "./demo-plugin",
    )?;
    write_plugin_source(repo_root.path(), "demo-plugin", &[])?;
    std::fs::create_dir_all(codex_home.path().join(".tmp"))?;
    std::fs::write(
        codex_home
            .path()
            .join(".tmp/plugin-share-local-paths-v1.json"),
        "not valid json\n",
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                repo_root.path().join(".agents/plugins/marketplace.json"),
            )?),
            remote_marketplace_name: None,
            plugin_name: "demo-plugin".to_string(),
        })
        .await?;

    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32603);
    assert!(
        error
            .error
            .message
            .contains("failed to load plugin share local path mapping")
    );
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
    std::fs::create_dir_all(plugin_root.join("hooks"))?;
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
  "keywords": ["api-key", "developer tools"],
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
    "logoDark": "./assets/logo-dark.png",
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
      "id": "gmail",
      "category": "Communication"
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
        plugin_root.join("hooks/hooks.json"),
        r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "echo startup"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "echo first"
          },
          {
            "type": "command",
            "command": "echo second"
          }
        ]
      }
    ]
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

[hooks.state."demo-plugin@codex-curated:hooks/hooks.json:pre_tool_use:0:0"]
enabled = false
"#,
    )?;
    write_installed_plugin(&codex_home, "codex-curated", "demo-plugin")?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;
    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(marketplace_path.clone()),
            remote_marketplace_name: None,
            plugin_name: "demo-plugin".to_string(),
        })
        .await?;

    let response: PluginReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

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
    assert_eq!(
        response
            .plugin
            .summary
            .interface
            .as_ref()
            .and_then(|interface| interface.logo_dark.as_ref()),
        Some(
            &AbsolutePathBuf::try_from(plugin_root.join("assets/logo-dark.png"))
                .expect("absolute dark logo path")
        )
    );
    assert_eq!(
        response.plugin.summary.keywords,
        vec!["api-key".to_string(), "developer tools".to_string()]
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
    assert_eq!(
        response.plugin.hooks,
        vec![
            codex_app_server_protocol::PluginHookSummary {
                key: "demo-plugin@codex-curated:hooks/hooks.json:pre_tool_use:0:0".to_string(),
                event_name: HookEventName::PreToolUse,
            },
            codex_app_server_protocol::PluginHookSummary {
                key: "demo-plugin@codex-curated:hooks/hooks.json:pre_tool_use:0:1".to_string(),
                event_name: HookEventName::PreToolUse,
            },
            codex_app_server_protocol::PluginHookSummary {
                key: "demo-plugin@codex-curated:hooks/hooks.json:session_start:0:0".to_string(),
                event_name: HookEventName::SessionStart,
            },
        ]
    );
    assert_eq!(response.plugin.apps.len(), 1);
    assert_eq!(response.plugin.apps[0].id, "gmail");
    assert_eq!(response.plugin.apps[0].name, "gmail");
    assert_eq!(
        response.plugin.apps[0].install_url.as_deref(),
        Some("https://chatgpt.com/apps/gmail/gmail")
    );
    assert_eq!(
        response.plugin.apps[0].category.as_deref(),
        Some("Communication")
    );
    assert_eq!(response.plugin.mcp_servers.len(), 1);
    assert_eq!(response.plugin.mcp_servers[0], "demo");
    Ok(())
}

#[tokio::test]
async fn plugin_read_batches_large_app_metadata_requests() -> Result<()> {
    let app_ids = (0..101)
        .map(|index| format!("app-{index:03}"))
        .collect::<Vec<_>>();
    let connectors = app_ids
        .iter()
        .map(|app_id| AppInfo {
            id: app_id.clone(),
            name: format!("App {app_id}"),
            description: Some(format!("{app_id} connector")),
            logo_url: None,
            logo_url_dark: None,
            icon_assets: None,
            icon_dark_assets: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        })
        .collect::<Vec<_>>();
    let (server_url, server_handle) = start_apps_server(connectors).await?;

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
    write_plugin_source(
        repo_root.path(),
        "sample-plugin",
        &app_ids.iter().map(String::as_str).collect::<Vec<_>>(),
    )?;
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
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
    let mut expected_app_ids = app_ids.iter().map(String::as_str).collect::<Vec<_>>();
    expected_app_ids.sort_unstable();

    assert_eq!(
        response
            .plugin
            .apps
            .iter()
            .map(|app| app.id.as_str())
            .collect::<Vec<_>>(),
        expected_app_ids
    );

    server_handle.abort();
    let _ = server_handle.await;
    Ok(())
}

#[tokio::test]
async fn plugin_read_stops_batching_after_app_metadata_failure() -> Result<()> {
    let app_ids = (0..101)
        .map(|index| format!("app-{index:03}"))
        .collect::<Vec<_>>();
    let server = MockServer::start().await;
    // Warm one app in the failing chunk and one in the skipped chunk, then fail exactly one refresh.
    Mock::given(method("POST"))
        .and(path("/ps/apps/batch"))
        .and(body_json(json!({
            "app_ids": ["app-000", "app-100"],
            "include_tools": false,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "apps": [
                {"id": "app-000", "name": "Cached first", "description": "First cached app", "tools": null},
                {"id": "app-100", "name": "Cached last", "description": "Last cached app", "tools": null},
            ]
        })))
        .expect(1)
        .with_priority(/*p*/ 1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/ps/apps/batch"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .with_priority(/*p*/ 2)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    write_connectors_config(codex_home.path(), &server.uri())?;
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
    write_plugin_source(
        repo_root.path(),
        "sample-plugin",
        &app_ids.iter().map(String::as_str).collect::<Vec<_>>(),
    )?;
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_apps_read_request(AppsReadParams {
            app_ids: vec!["app-000".to_string(), "app-100".to_string()],
            include_tools: false,
        })
        .await?;
    let _: AppsReadResponse = timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

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
    let mut expected_apps = app_ids
        .iter()
        .map(|app_id| match app_id.as_str() {
            "app-000" => ("app-000", "Cached first", Some("First cached app")),
            "app-100" => ("app-100", "Cached last", Some("Last cached app")),
            app_id => (app_id, app_id, None),
        })
        .collect::<Vec<_>>();
    expected_apps.sort_unstable();

    assert_eq!(
        response
            .plugin
            .apps
            .iter()
            .map(|app| (
                app.id.as_str(),
                app.name.as_str(),
                app.description.as_deref()
            ))
            .collect::<Vec<_>>(),
        expected_apps
    );

    Ok(())
}

#[tokio::test]
async fn plugin_read_hides_apps_for_api_key_auth() -> Result<()> {
    let connectors = vec![AppInfo {
        id: "alpha".to_string(),
        name: "Alpha".to_string(),
        description: Some("Alpha connector".to_string()),
        logo_url: Some("https://example.com/alpha.png".to_string()),
        logo_url_dark: None,
        icon_assets: None,
        icon_dark_assets: None,
        distribution_channel: Some("featured".to_string()),
        branding: None,
        app_metadata: Some(AppMetadata {
            review: None,
            categories: Some(vec!["Productivity".to_string()]),
            sub_categories: None,
            seo_description: None,
            screenshots: None,
            developer: None,
            version: None,
            version_id: None,
            version_notes: None,
            first_party_requires_install: None,
            show_in_composer_when_unlinked: None,
        }),
        labels: None,
        install_url: None,
        is_accessible: false,
        is_enabled: true,
        plugin_display_names: Vec::new(),
    }];
    let (server_url, server_handle) = start_apps_server(connectors).await?;

    let codex_home = TempDir::new()?;
    write_connectors_config(codex_home.path(), &server_url)?;
    std::fs::write(
        codex_home.path().join("auth.json"),
        r#"{"OPENAI_API_KEY":"sk-test-key","tokens":null,"last_refresh":null}"#,
    )?;

    let repo_root = TempDir::new()?;
    write_plugin_marketplace(
        repo_root.path(),
        "debug",
        "sample-plugin",
        "./sample-plugin",
    )?;
    write_plugin_source(repo_root.path(), "sample-plugin", &["alpha"])?;
    std::fs::write(
        repo_root.path().join("sample-plugin/.mcp.json"),
        r#"{"mcpServers":{"alpha":{"command":"alpha-mcp"}}}"#,
    )?;
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[
            ("CODEX_ACCESS_TOKEN", None),
            ("CODEX_API_KEY", None),
            ("OPENAI_API_KEY", None),
        ])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;

    let response: PluginReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    assert!(response.plugin.apps.is_empty());
    assert_eq!(response.plugin.mcp_servers, vec!["alpha".to_string()]);

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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                repo_root.path().join(".agents/plugins/marketplace.json"),
            )?),
            remote_marketplace_name: None,
            plugin_name: "demo-plugin".to_string(),
        })
        .await?;

    let response: PluginReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_read_request(PluginReadParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                repo_root.path().join(".agents/plugins/marketplace.json"),
            )?),
            remote_marketplace_name: None,
            plugin_name: "toolkit".to_string(),
        })
        .await?;

    let response: PluginReadResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

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
    connectors: Vec<AppInfo>,
}

async fn start_apps_server(connectors: Vec<AppInfo>) -> Result<(String, JoinHandle<()>)> {
    let state = Arc::new(AppsServerState { connectors });

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let router = Router::new()
        .route("/ps/apps/batch", post(batch_apps))
        .with_state(state);

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Ok((format!("http://{addr}"), handle))
}

async fn batch_apps(
    State(state): State<Arc<AppsServerState>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    let bearer_ok = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "Bearer chatgpt-token");
    let account_ok = headers
        .get("chatgpt-account-id")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "account-123");
    let product_sku_ok = headers
        .get("oai-product-sku")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "codex");

    if !bearer_ok || !account_ok || !product_sku_ok {
        Err(StatusCode::UNAUTHORIZED)
    } else {
        let app_ids = body
            .get("app_ids")
            .and_then(serde_json::Value::as_array)
            .ok_or(StatusCode::BAD_REQUEST)?;
        if app_ids.len() > 100 {
            return Err(StatusCode::BAD_REQUEST);
        }
        let apps = state
            .connectors
            .iter()
            .filter(|connector| {
                app_ids
                    .iter()
                    .any(|app_id| app_id.as_str() == Some(connector.id.as_str()))
            })
            .map(|connector| {
                json!({
                    "id": connector.id,
                    "name": connector.name,
                    "description": connector.description,
                    "icon_url": connector.logo_url,
                    "tools": null
                })
            })
            .collect::<Vec<_>>();
        Ok(Json(json!({ "apps": apps })))
    }
}

fn write_connectors_config(codex_home: &std::path::Path, base_url: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{base_url}"
cli_auth_credentials_store = "file"
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

fn write_plugin_share_local_path_mapping(
    codex_home: &std::path::Path,
    remote_plugin_id: &str,
    plugin_path: &AbsolutePathBuf,
) -> std::io::Result<()> {
    let mut local_plugin_paths_by_remote_plugin_id = serde_json::Map::new();
    local_plugin_paths_by_remote_plugin_id.insert(
        remote_plugin_id.to_string(),
        serde_json::to_value(plugin_path).map_err(std::io::Error::other)?,
    );
    let contents = serde_json::to_string_pretty(&json!({
        "localPluginPathsByRemotePluginId": local_plugin_paths_by_remote_plugin_id,
    }))
    .map_err(std::io::Error::other)?;
    std::fs::create_dir_all(codex_home.join(".tmp"))?;
    std::fs::write(
        codex_home.join(".tmp/plugin-share-local-paths-v1.json"),
        format!("{contents}\n"),
    )
}
