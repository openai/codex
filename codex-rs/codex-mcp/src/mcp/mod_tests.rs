use super::*;
use crate::McpPluginAttribution;
use crate::McpServerRegistration;
use crate::McpServerRuntimeMetadata;
use codex_config::Constrained;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::McpToolApproval;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GranularApprovalConfig;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::collections::HashSet;

fn test_mcp_config() -> McpConfig {
    McpConfig {
        mcp_oauth_credentials_store_mode: OAuthCredentialsStoreMode::default(),
        auth_keyring_backend_kind: AuthKeyringBackendKind::default(),
        mcp_oauth_callback_port: None,
        mcp_oauth_callback_url: None,
        skill_mcp_dependency_install_enabled: true,
        approval_policy: Constrained::allow_any(AskForApproval::OnRequest),
        codex_linux_sandbox_exe: None,
        use_legacy_landlock: false,
        prefix_mcp_tool_names: true,
        client_elicitation_capability: ElicitationCapability::default(),
        mcp_server_catalog: ResolvedMcpCatalog::default(),
    }
}

fn test_http_server(url: &str) -> McpServerConfig {
    McpServerConfig {
        auth: Default::default(),
        transport: McpServerTransportConfig::StreamableHttp {
            url: url.to_string(),
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
                tool_approval_mode: Some(McpToolApproval::Approve),
            },
        ));
    }

    assert!(!mcp_permission_prompt_is_auto_approved(
        AskForApproval::OnRequest,
        &PermissionProfile::read_only(),
        McpPermissionPromptAutoApproveContext {
            tool_approval_mode: Some(McpToolApproval::Auto),
        },
    ));
}

#[test]
fn mcp_prompt_auto_approval_rejects_auto_mode_in_default_permission_mode() {
    assert!(!mcp_permission_prompt_is_auto_approved(
        AskForApproval::OnRequest,
        &PermissionProfile::read_only(),
        McpPermissionPromptAutoApproveContext {
            tool_approval_mode: Some(McpToolApproval::Auto),
        },
    ));
}

#[test]
fn tool_plugin_provenance_collects_mcp_server_sources() {
    let mut config = test_mcp_config();
    let mut catalog = ResolvedMcpCatalog::builder();
    catalog.register(McpServerRegistration::from_plugin(
        "alpha".to_string(),
        McpPluginAttribution::new("alpha@test".to_string(), "alpha-plugin".to_string()),
        /*plugin_order*/ 0,
        test_http_server("https://alpha.example/mcp"),
    ));
    config.mcp_server_catalog = catalog.build();
    let provenance = tool_plugin_provenance(&config);

    assert_eq!(
        provenance,
        ToolPluginProvenance {
            plugin_display_names_by_mcp_server_name: HashMap::from([(
                "alpha".to_string(),
                vec!["alpha-plugin".to_string()],
            )]),
            plugin_ids_by_mcp_server_name: HashMap::from([(
                "alpha".to_string(),
                "alpha@test".to_string(),
            )]),
            selected_plugin_mcp_server_names: HashSet::new(),
        }
    );
    assert_eq!(
        provenance.plugin_id_for_mcp_server_name("alpha"),
        Some("alpha@test")
    );
    assert_eq!(provenance.plugin_id_for_mcp_server_name("beta"), None);
}

#[test]
fn tool_plugin_provenance_collects_multi_source_runtime_metadata() {
    let mut config = test_mcp_config();
    let mut catalog = ResolvedMcpCatalog::builder();
    catalog.register(McpServerRegistration::from_effective_extension(
        "shared-app".to_string(),
        "test-extension",
        /*contribution_order*/ 0,
        EffectiveMcpServer::configured(test_http_server("https://apps.example/mcp"))
            .with_runtime_metadata(
                McpServerRuntimeMetadata::default().with_plugin_display_names([
                    "Workspace".to_string(),
                    " Calendar ".to_string(),
                    "Workspace".to_string(),
                    "  ".to_string(),
                ]),
            ),
    ));
    config.mcp_server_catalog = catalog.build();

    let provenance = tool_plugin_provenance(&config);

    assert_eq!(
        provenance.plugin_display_names_for_mcp_server_name("shared-app"),
        &["Calendar".to_string(), "Workspace".to_string()]
    );
    assert_eq!(
        provenance.plugin_id_for_mcp_server_name("shared-app"),
        None,
        "generic runtime attribution must not synthesize a plugin identity"
    );
}

#[test]
fn selected_mcp_attribution_uses_the_selected_registration() {
    let mut config = test_mcp_config();
    let mut catalog = ResolvedMcpCatalog::builder();
    catalog.register(McpServerRegistration::from_selected_plugin(
        "github".to_string(),
        McpPluginAttribution::new(
            "shared-plugin-id".to_string(),
            "Executor GitHub".to_string(),
        ),
        /*selection_order*/ 0,
        test_http_server("https://github.example/mcp"),
    ));
    config.mcp_server_catalog = catalog.build();

    let provenance = tool_plugin_provenance(&config);

    assert_eq!(
        provenance,
        ToolPluginProvenance {
            plugin_display_names_by_mcp_server_name: HashMap::from([(
                "github".to_string(),
                vec!["Executor GitHub".to_string()],
            )]),
            plugin_ids_by_mcp_server_name: HashMap::from([(
                "github".to_string(),
                "shared-plugin-id".to_string(),
            )]),
            selected_plugin_mcp_server_names: HashSet::from(["github".to_string()]),
        }
    );
    assert!(provenance.is_selected_plugin_mcp_server("github"));
}

#[test]
fn effective_mcp_servers_preserve_registered_servers() {
    let mut config = test_mcp_config();
    let mut catalog = ResolvedMcpCatalog::builder();
    catalog.register(McpServerRegistration::from_config(
        "sample".to_string(),
        test_http_server("https://user.example/mcp"),
    ));
    catalog.register(McpServerRegistration::from_config(
        "docs".to_string(),
        test_http_server("https://docs.example/mcp"),
    ));
    config.mcp_server_catalog = catalog.build();

    let effective = effective_mcp_servers(&config);

    let sample = effective.get("sample").expect("user server should exist");
    let docs = effective
        .get("docs")
        .expect("configured server should exist");

    let sample = sample.config();
    let docs = docs.config();

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
}

#[test]
fn runtime_extension_uses_standard_public_name_precedence() {
    for selected in [false, true] {
        let mut config = test_mcp_config();
        let mut catalog = ResolvedMcpCatalog::builder();
        let attribution =
            McpPluginAttribution::new("workspace@test".to_string(), "Workspace".to_string());
        let plugin = if selected {
            McpServerRegistration::from_selected_plugin(
                "shared_namespace".to_string(),
                attribution,
                /*selection_order*/ 0,
                test_http_server("https://plugin.example/mcp"),
            )
        } else {
            McpServerRegistration::from_plugin(
                "shared_namespace".to_string(),
                attribution,
                /*plugin_order*/ 0,
                test_http_server("https://plugin.example/mcp"),
            )
        };
        catalog.register(plugin);
        catalog.register(McpServerRegistration::from_effective_extension(
            "shared_namespace".to_string(),
            "runtime-test",
            /*contribution_order*/ 0,
            EffectiveMcpServer::configured(test_http_server("http://127.0.0.1:4321/mcp")),
        ));
        config.mcp_server_catalog = catalog.build();

        let effective = effective_mcp_servers(&config);
        assert_eq!(effective.len(), 1);
        let McpServerTransportConfig::StreamableHttp { url, .. } =
            &effective["shared_namespace"].config().transport
        else {
            panic!("expected HTTP runtime extension")
        };
        assert_eq!(url, "http://127.0.0.1:4321/mcp");
    }
}
