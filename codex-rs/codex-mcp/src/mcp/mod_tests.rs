use super::*;
use codex_config::Constrained;
use codex_config::types::AppToolApproval;
use codex_login::CodexAuth;
use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GranularApprovalConfig;
use pretty_assertions::assert_eq;
use rmcp::model::Annotated;
use rmcp::model::Annotations;
use rmcp::model::Icon;
use rmcp::model::IconTheme;
use rmcp::model::Meta;
use rmcp::model::RawResource;
use rmcp::model::RawResourceTemplate;
use rmcp::model::Role;
use rmcp::model::ToolAnnotations;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

fn test_mcp_config(codex_home: PathBuf) -> McpConfig {
    McpConfig {
        chatgpt_base_url: "https://chatgpt.com".to_string(),
        apps_mcp_path_override: None,
        apps_mcp_product_sku: None,
        codex_home,
        mcp_oauth_credentials_store_mode: OAuthCredentialsStoreMode::default(),
        mcp_oauth_callback_port: None,
        mcp_oauth_callback_url: None,
        skill_mcp_dependency_install_enabled: true,
        approval_policy: Constrained::allow_any(AskForApproval::OnFailure),
        codex_linux_sandbox_exe: None,
        use_legacy_landlock: false,
        apps_enabled: false,
        prefix_mcp_tool_names: true,
        client_elicitation_capability: ElicitationCapability::default(),
        configured_mcp_servers: HashMap::new(),
        plugin_ids_by_mcp_server_name: HashMap::new(),
        plugin_capability_summaries: Vec::new(),
    }
}

#[test]
fn qualified_mcp_tool_name_prefix_sanitizes_server_names_without_lowercasing() {
    assert_eq!(
        qualified_mcp_tool_name_prefix("Some-Server"),
        "mcp__Some_Server__".to_string()
    );
}

#[test]
fn protocol_tool_conversion_preserves_typed_fields_and_open_json() {
    let input_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "query": {"type": "string"}
        }
    })
    .as_object()
    .expect("input schema")
    .clone();
    let output_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "result": {"type": "string"}
        }
    })
    .as_object()
    .expect("output schema")
    .clone();
    let tool = rmcp::model::Tool::new("search", "Search documents", Arc::new(input_schema))
        .with_title("Document search")
        .with_raw_output_schema(Arc::new(output_schema))
        .with_annotations(ToolAnnotations::from_raw(
            Some("Search".to_string()),
            Some(true),
            Some(false),
            Some(true),
            Some(false),
        ))
        .with_icons(vec![
            Icon::new("https://example.com/search.svg")
                .with_mime_type("image/svg+xml")
                .with_sizes(vec!["any".to_string()])
                .with_theme(IconTheme::Dark),
        ])
        .with_meta(Meta(
            serde_json::json!({"connectorId": "docs"})
                .as_object()
                .expect("tool metadata")
                .clone(),
        ));

    assert_eq!(
        protocol_tool_from_rmcp_tool(&tool),
        Tool {
            name: "search".to_string(),
            title: Some("Document search".to_string()),
            description: Some("Search documents".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            }),
            output_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "result": {"type": "string"}
                }
            })),
            annotations: Some(serde_json::json!({
                "title": "Search",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            })),
            icons: Some(vec![serde_json::json!({
                "src": "https://example.com/search.svg",
                "mimeType": "image/svg+xml",
                "sizes": ["any"],
                "theme": "dark"
            })]),
            meta: Some(serde_json::json!({"connectorId": "docs"})),
        }
    );
}

#[test]
fn resource_conversion_preserves_typed_fields_and_open_json() {
    let mut annotations = Annotations::default();
    annotations.audience = Some(vec![Role::User, Role::Assistant]);
    annotations.priority = Some(0.75);
    annotations.last_modified = Some(
        "2026-06-07T12:34:56.123Z"
            .parse()
            .expect("last modified timestamp"),
    );
    let resource = Annotated::new(
        RawResource::new("file:///docs/guide.md", "guide")
            .with_title("Guide")
            .with_description("Product guide")
            .with_mime_type("text/markdown")
            .with_size(4_000_000_000)
            .with_icons(vec![Icon::new("https://example.com/guide.png")])
            .with_meta(Meta(
                serde_json::json!({"etag": "abc"})
                    .as_object()
                    .expect("resource metadata")
                    .clone(),
            )),
        Some(annotations),
    );

    assert_eq!(
        convert_mcp_resources(HashMap::from([("docs".to_string(), vec![resource])])),
        HashMap::from([(
            "docs".to_string(),
            vec![Resource {
                annotations: Some(serde_json::json!({
                    "audience": ["user", "assistant"],
                    "priority": 0.75,
                    "lastModified": "2026-06-07T12:34:56.123Z"
                })),
                description: Some("Product guide".to_string()),
                mime_type: Some("text/markdown".to_string()),
                name: "guide".to_string(),
                size: Some(4_000_000_000),
                title: Some("Guide".to_string()),
                uri: "file:///docs/guide.md".to_string(),
                icons: Some(vec![serde_json::json!({
                    "src": "https://example.com/guide.png"
                })]),
                meta: Some(serde_json::json!({"etag": "abc"})),
            }]
        )])
    );
}

#[test]
fn resource_template_conversion_preserves_supported_fields() {
    let mut annotations = Annotations::default();
    annotations.audience = Some(vec![Role::User]);
    annotations.priority = Some(0.5);
    let template = Annotated::new(
        RawResourceTemplate::new("file:///docs/{name}", "document")
            .with_title("Document")
            .with_description("A document by name")
            .with_mime_type("text/markdown")
            .with_icons(vec![Icon::new("https://example.com/document.png")]),
        Some(annotations),
    );

    assert_eq!(
        convert_mcp_resource_templates(HashMap::from([("docs".to_string(), vec![template])])),
        HashMap::from([(
            "docs".to_string(),
            vec![ResourceTemplate {
                annotations: Some(serde_json::json!({
                    "audience": ["user"],
                    "priority": 0.5
                })),
                uri_template: "file:///docs/{name}".to_string(),
                name: "document".to_string(),
                title: Some("Document".to_string()),
                description: Some("A document by name".to_string()),
                mime_type: Some("text/markdown".to_string()),
            }]
        )])
    );
}

#[test]
fn mcp_prompt_auto_approval_honors_unrestricted_managed_profiles() {
    assert!(mcp_permission_prompt_is_auto_approved(
        AskForApproval::Never,
        &PermissionProfile::Managed {
            file_system: ManagedFileSystemPermissions::Unrestricted,
            network: NetworkSandboxPolicy::Enabled,
        },
        McpPermissionPromptAutoApproveContext::default(),
    ));
    assert!(mcp_permission_prompt_is_auto_approved(
        AskForApproval::Never,
        &PermissionProfile::Managed {
            file_system: ManagedFileSystemPermissions::Unrestricted,
            network: NetworkSandboxPolicy::Restricted,
        },
        McpPermissionPromptAutoApproveContext::default(),
    ));
    assert!(!mcp_permission_prompt_is_auto_approved(
        AskForApproval::Never,
        &PermissionProfile::read_only(),
        McpPermissionPromptAutoApproveContext::default(),
    ));
    assert!(!mcp_permission_prompt_is_auto_approved(
        AskForApproval::OnRequest,
        &PermissionProfile::Managed {
            file_system: ManagedFileSystemPermissions::Unrestricted,
            network: NetworkSandboxPolicy::Enabled,
        },
        McpPermissionPromptAutoApproveContext::default(),
    ));
}

#[test]
fn mcp_prompt_auto_approval_honors_approved_tools_in_all_permission_modes() {
    for approval_policy in [
        AskForApproval::UnlessTrusted,
        AskForApproval::OnFailure,
        AskForApproval::OnRequest,
        AskForApproval::Granular(GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: true,
        }),
        AskForApproval::Never,
    ] {
        assert!(mcp_permission_prompt_is_auto_approved(
            approval_policy,
            &PermissionProfile::read_only(),
            McpPermissionPromptAutoApproveContext {
                tool_approval_mode: Some(AppToolApproval::Approve),
            },
        ));
    }

    assert!(!mcp_permission_prompt_is_auto_approved(
        AskForApproval::OnRequest,
        &PermissionProfile::read_only(),
        McpPermissionPromptAutoApproveContext {
            tool_approval_mode: Some(AppToolApproval::Auto),
        },
    ));
}

#[test]
fn mcp_prompt_auto_approval_rejects_auto_mode_in_default_permission_mode() {
    assert!(!mcp_permission_prompt_is_auto_approved(
        AskForApproval::OnRequest,
        &PermissionProfile::read_only(),
        McpPermissionPromptAutoApproveContext {
            tool_approval_mode: Some(AppToolApproval::Auto),
        },
    ));
}

#[test]
fn tool_plugin_provenance_collects_app_and_mcp_sources() {
    let mut config = test_mcp_config(PathBuf::new());
    config.plugin_ids_by_mcp_server_name =
        HashMap::from([("alpha".to_string(), "alpha@test".to_string())]);
    config.plugin_capability_summaries = vec![
        PluginCapabilitySummary {
            display_name: "alpha-plugin".to_string(),
            app_connector_ids: vec![AppConnectorId("connector_example".to_string())],
            mcp_server_names: vec!["alpha".to_string()],
            ..PluginCapabilitySummary::default()
        },
        PluginCapabilitySummary {
            display_name: "beta-plugin".to_string(),
            app_connector_ids: vec![
                AppConnectorId("connector_example".to_string()),
                AppConnectorId("connector_gmail".to_string()),
            ],
            mcp_server_names: vec!["beta".to_string()],
            ..PluginCapabilitySummary::default()
        },
    ];
    let provenance = tool_plugin_provenance(&config);

    assert_eq!(
        provenance,
        ToolPluginProvenance {
            plugin_display_names_by_connector_id: HashMap::from([
                (
                    "connector_example".to_string(),
                    vec!["alpha-plugin".to_string(), "beta-plugin".to_string()],
                ),
                (
                    "connector_gmail".to_string(),
                    vec!["beta-plugin".to_string()],
                ),
            ]),
            plugin_display_names_by_mcp_server_name: HashMap::from([
                ("alpha".to_string(), vec!["alpha-plugin".to_string()]),
                ("beta".to_string(), vec!["beta-plugin".to_string()]),
            ]),
            plugin_ids_by_mcp_server_name: HashMap::from([(
                "alpha".to_string(),
                "alpha@test".to_string(),
            )]),
        }
    );
    assert_eq!(
        provenance.plugin_id_for_mcp_server_name("alpha"),
        Some("alpha@test")
    );
    assert_eq!(provenance.plugin_id_for_mcp_server_name("beta"), None);
}

#[test]
fn codex_apps_mcp_url_for_base_url_keeps_existing_paths() {
    assert_eq!(
        codex_apps_mcp_url_for_base_url(
            "https://chatgpt.com/backend-api",
            /*apps_mcp_path_override*/ None,
        ),
        "https://chatgpt.com/backend-api/wham/apps"
    );
    assert_eq!(
        codex_apps_mcp_url_for_base_url(
            "https://chat.openai.com",
            /*apps_mcp_path_override*/ None,
        ),
        "https://chat.openai.com/backend-api/wham/apps"
    );
    assert_eq!(
        codex_apps_mcp_url_for_base_url(
            "http://localhost:8080/api/codex",
            /*apps_mcp_path_override*/ None,
        ),
        "http://localhost:8080/api/codex/apps"
    );
    assert_eq!(
        codex_apps_mcp_url_for_base_url(
            "http://localhost:8080",
            /*apps_mcp_path_override*/ None,
        ),
        "http://localhost:8080/api/codex/apps"
    );
}

#[test]
fn codex_apps_mcp_url_uses_legacy_codex_apps_path() {
    let config = test_mcp_config(PathBuf::from("/tmp"));

    assert_eq!(
        codex_apps_mcp_url(&config),
        "https://chatgpt.com/backend-api/wham/apps"
    );
}

#[test]
fn codex_apps_server_config_uses_legacy_codex_apps_path() {
    let mut config = test_mcp_config(PathBuf::from("/tmp"));
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    let mut servers = with_codex_apps_mcp(HashMap::new(), /*auth*/ None, &config);
    assert!(!servers.contains_key(CODEX_APPS_MCP_SERVER_NAME));

    config.apps_enabled = true;

    servers = with_codex_apps_mcp(servers, Some(&auth), &config);
    let server = servers
        .get(CODEX_APPS_MCP_SERVER_NAME)
        .expect("codex apps should be present when apps is enabled");
    let config = server
        .configured_config()
        .expect("codex apps should use configured transport");
    let url = match &config.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => url,
        _ => panic!("expected streamable http transport for codex apps"),
    };

    assert_eq!(url, "https://chatgpt.com/backend-api/wham/apps");
}

#[test]
fn codex_apps_server_config_uses_configured_apps_mcp_path_override() {
    let mut config = test_mcp_config(PathBuf::from("/tmp"));
    config.apps_mcp_path_override = Some("/custom/mcp".to_string());
    config.apps_enabled = true;
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    let servers = with_codex_apps_mcp(HashMap::new(), Some(&auth), &config);
    let server = servers
        .get(CODEX_APPS_MCP_SERVER_NAME)
        .expect("codex apps should be present when apps is enabled");
    let config = server
        .configured_config()
        .expect("codex apps should use configured transport");
    let url = match &config.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => url,
        _ => panic!("expected streamable http transport for codex apps"),
    };

    assert_eq!(url, "https://chatgpt.com/backend-api/custom/mcp");
}

#[test]
fn codex_apps_server_config_forwards_configured_product_sku_header() {
    let mut config = test_mcp_config(PathBuf::from("/tmp"));
    config.apps_mcp_product_sku = Some("tpp".to_string());
    config.apps_enabled = true;
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    let servers = with_codex_apps_mcp(HashMap::new(), Some(&auth), &config);
    let server = servers
        .get(CODEX_APPS_MCP_SERVER_NAME)
        .expect("codex apps should be present when apps is enabled");
    let config = server
        .configured_config()
        .expect("codex apps should use configured transport");

    match &config.transport {
        McpServerTransportConfig::StreamableHttp {
            http_headers,
            env_http_headers,
            ..
        } => {
            assert_eq!(
                http_headers,
                &Some(HashMap::from([(
                    "X-OpenAI-Product-Sku".to_string(),
                    "tpp".to_string(),
                )]))
            );
            assert!(env_http_headers.is_none());
        }
        other => panic!("expected streamable http transport, got {other:?}"),
    }
}

#[tokio::test]
async fn effective_mcp_servers_preserve_user_servers_and_add_codex_apps() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let mut config = test_mcp_config(codex_home.path().to_path_buf());
    config.apps_enabled = true;
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    config.configured_mcp_servers.insert(
        "sample".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://user.example/mcp".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            enabled: true,
            required: false,
            supports_parallel_tool_calls: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            default_tools_approval_mode: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );
    config.configured_mcp_servers.insert(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://docs.example/mcp".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            enabled: true,
            required: false,
            supports_parallel_tool_calls: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            default_tools_approval_mode: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );

    let effective = effective_mcp_servers(&config, Some(&auth));

    let sample = effective.get("sample").expect("user server should exist");
    let docs = effective
        .get("docs")
        .expect("configured server should exist");
    let codex_apps = effective
        .get(CODEX_APPS_MCP_SERVER_NAME)
        .expect("codex apps server should exist");

    let sample = sample
        .configured_config()
        .expect("configured server should retain transport");
    let docs = docs
        .configured_config()
        .expect("configured server should retain transport");
    let codex_apps = codex_apps
        .configured_config()
        .expect("codex apps should use configured transport");

    match &sample.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => {
            assert_eq!(url, "https://user.example/mcp");
        }
        other => panic!("expected streamable http transport, got {other:?}"),
    }
    match &docs.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => {
            assert_eq!(url, "https://docs.example/mcp");
        }
        other => panic!("expected streamable http transport, got {other:?}"),
    }
    match &codex_apps.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => {
            assert_eq!(url, "https://chatgpt.com/backend-api/wham/apps");
        }
        other => panic!("expected streamable http transport, got {other:?}"),
    }
}
