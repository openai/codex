use std::collections::HashMap;
use std::time::Duration;

use codex_config::AppToolApproval;
use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::McpServerConfig;
use codex_config::McpServerDisabledReason;
use codex_config::McpServerToolConfig;
use codex_config::McpServerTransportConfig;
use pretty_assertions::assert_eq;

use super::McpServerConflict;
use super::McpServerRegistration;
use super::McpServerSource;
use super::ResolvedMcpCatalog;

fn server(url: &str, enabled: bool) -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url: url.to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled,
        required: true,
        supports_parallel_tool_calls: true,
        disabled_reason: (!enabled).then_some(McpServerDisabledReason::Unknown),
        startup_timeout_sec: Some(Duration::from_secs(7)),
        tool_timeout_sec: Some(Duration::from_secs(11)),
        default_tools_approval_mode: Some(AppToolApproval::Prompt),
        enabled_tools: Some(vec!["read".to_string()]),
        disabled_tools: Some(vec!["write".to_string()]),
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::from([(
            "read".to_string(),
            McpServerToolConfig {
                approval_mode: Some(AppToolApproval::Approve),
            },
        )]),
    }
}

#[test]
fn source_precedence_preserves_the_winning_registration() {
    let extension = server("https://extension.example/mcp", true);
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_extension(
        "docs".to_string(),
        "hosted",
        /*contribution_order*/ 0,
        extension.clone(),
    ));
    builder.register(McpServerRegistration::from_plugin(
        "docs".to_string(),
        "plugin@test".to_string(),
        /*plugin_order*/ 0,
        server("https://plugin.example/mcp", true),
    ));
    builder.register(McpServerRegistration::from_compatibility(
        "docs".to_string(),
        "legacy",
        server("https://compatibility.example/mcp", true),
    ));
    builder.register(McpServerRegistration::from_config(
        "docs".to_string(),
        server("https://config.example/mcp", true),
    ));

    let catalog = builder.build();
    let resolved = catalog.server("docs").expect("resolved server");

    assert_eq!(
        resolved.source(),
        &McpServerSource::Extension {
            id: "hosted".to_string(),
        }
    );
    assert_eq!(resolved.config(), &extension);
    assert!(catalog.plugin_ids_by_server_name().is_empty());
    assert!(catalog.conflicts().is_empty());
}

#[test]
fn disabled_veto_only_disables_the_winning_registration() {
    let extension = server("https://extension.example/mcp", true);
    let mut expected = extension.clone();
    expected.enabled = false;
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_extension(
        "docs".to_string(),
        "hosted",
        /*contribution_order*/ 0,
        extension,
    ));
    builder.disable("docs".to_string());

    let actual = builder
        .build()
        .server("docs")
        .expect("resolved server")
        .config()
        .clone();

    assert_eq!(actual, expected);
}

#[test]
fn earlier_plugin_wins_with_an_explicit_conflict() {
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_plugin(
        "docs".to_string(),
        "alpha@test".to_string(),
        /*plugin_order*/ 0,
        server("https://alpha.example/mcp", true),
    ));
    builder.register(McpServerRegistration::from_plugin(
        "docs".to_string(),
        "beta@test".to_string(),
        /*plugin_order*/ 1,
        server("https://beta.example/mcp", true),
    ));

    let catalog = builder.build();

    assert_eq!(
        catalog.plugin_ids_by_server_name(),
        HashMap::from([("docs".to_string(), "alpha@test".to_string())])
    );
    assert_eq!(
        catalog.conflicts(),
        &[McpServerConflict {
            name: "docs".to_string(),
            winner: McpServerSource::Plugin {
                plugin_id: "alpha@test".to_string(),
            },
            shadowed: McpServerSource::Plugin {
                plugin_id: "beta@test".to_string(),
            },
        }]
    );
}
