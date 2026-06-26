use std::collections::HashMap;
use std::time::Duration;

use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::McpServerConfig;
use codex_config::McpServerToolConfig;
use codex_config::McpServerTransportConfig;
use codex_config::McpToolApproval;
use pretty_assertions::assert_eq;

use super::McpPluginAttribution;
use super::McpServerConflict;
use super::McpServerConflictAction;
use super::McpServerRegistration;
use super::McpServerSource;
use super::ResolvedMcpCatalog;
use crate::EffectiveMcpServer;

fn server(url: &str) -> McpServerConfig {
    McpServerConfig {
        auth: Default::default(),
        transport: McpServerTransportConfig::StreamableHttp {
            url: url.to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled: true,
        required: true,
        supports_parallel_tool_calls: true,
        disabled_reason: None,
        startup_timeout_sec: Some(Duration::from_secs(7)),
        tool_timeout_sec: Some(Duration::from_secs(11)),
        default_tools_approval_mode: Some(McpToolApproval::Prompt),
        enabled_tools: Some(vec!["read".to_string()]),
        disabled_tools: Some(vec!["write".to_string()]),
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::from([(
            "read".to_string(),
            McpServerToolConfig {
                approval_mode: Some(McpToolApproval::Approve),
            },
        )]),
    }
}

fn effective_server(url: &str, bearer_token: &str) -> EffectiveMcpServer {
    EffectiveMcpServer::configured_with_runtime_bearer_token(server(url), bearer_token.to_string())
        .expect("valid runtime bearer server")
}

fn resolved_server(source: McpServerSource, config: McpServerConfig) -> super::ResolvedMcpServer {
    super::ResolvedMcpServer {
        source,
        server: super::McpServerRegistrationValue::Configured(Box::new(config)),
    }
}

#[test]
fn effective_extension_registration_is_excluded_from_configured_servers() {
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_effective_extension(
        "runtime".to_string(),
        "runtime_extension",
        /*contribution_order*/ 0,
        effective_server("http://127.0.0.1:4321/mcp", "runtime-secret"),
    ));

    let catalog = builder.build();

    assert!(catalog.server("runtime").is_some());
    assert!(catalog.configured_servers().is_empty());
    let effective = catalog.effective_servers();
    let effective_config = effective["runtime"].config();
    assert_eq!(
        &effective_config.transport,
        &server("http://127.0.0.1:4321/mcp").transport
    );
    let debug = format!("{catalog:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("runtime-secret"));
}

#[test]
fn effective_and_configured_extension_collisions_follow_contribution_order() {
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_effective_extension(
        "shared".to_string(),
        "runtime_extension",
        /*contribution_order*/ 0,
        effective_server("http://127.0.0.1:4321/mcp", "runtime-secret"),
    ));
    let configured = server("https://configured.example/mcp");
    builder.register(McpServerRegistration::from_extension(
        "shared".to_string(),
        "configured_extension",
        /*contribution_order*/ 1,
        configured.clone(),
    ));

    let catalog = builder.build();

    assert_eq!(
        catalog.configured_servers(),
        HashMap::from([("shared".to_string(), configured)])
    );
    assert!(catalog.effective_servers().is_empty());
    assert_eq!(
        catalog.conflicts(),
        &[McpServerConflict {
            name: "shared".to_string(),
            outcome: register(extension_source("configured_extension")),
            contenders: vec![
                register(extension_source("runtime_extension")),
                register(extension_source("configured_extension")),
            ],
        }]
    );
}

#[test]
fn disabled_name_veto_disables_an_effective_extension_winner() {
    let mut disabled = server("https://configured.example/mcp");
    disabled.enabled = false;
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_config(
        "shared".to_string(),
        disabled,
    ));
    let mut builder = builder.build().to_builder();
    builder.register(McpServerRegistration::from_effective_extension(
        "shared".to_string(),
        "runtime_extension",
        /*contribution_order*/ 0,
        effective_server("http://127.0.0.1:4321/mcp", "runtime-secret"),
    ));

    let effective = builder.build().effective_servers();

    assert!(!effective["shared"].enabled());
    let debug = format!("{:?}", effective["shared"]);
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("runtime-secret"));
}

fn plugin(plugin_id: &str) -> McpPluginAttribution {
    McpPluginAttribution::new(plugin_id.to_string(), plugin_id.to_string())
}

fn plugin_source(plugin_id: &str) -> McpServerSource {
    McpServerSource::Plugin(plugin(plugin_id))
}

fn selected_plugin_source(plugin_id: &str) -> McpServerSource {
    McpServerSource::SelectedPlugin(plugin(plugin_id))
}

fn extension_source(id: &str) -> McpServerSource {
    McpServerSource::Extension { id: id.to_string() }
}

fn register(source: McpServerSource) -> McpServerConflictAction {
    McpServerConflictAction::Register(source)
}

fn remove(source: McpServerSource) -> McpServerConflictAction {
    McpServerConflictAction::Remove(source)
}

#[test]
fn source_precedence_preserves_the_winning_registration() {
    let extension = server("https://extension.example/mcp");
    let mut plugin_server = server("https://plugin.example/mcp");
    plugin_server.enabled = false;
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_extension(
        "docs".to_string(),
        "hosted",
        /*contribution_order*/ 0,
        extension.clone(),
    ));
    builder.register(McpServerRegistration::from_plugin(
        "docs".to_string(),
        plugin("plugin@test"),
        /*plugin_order*/ 0,
        plugin_server,
    ));
    builder.register(McpServerRegistration::from_plugin(
        "docs".to_string(),
        plugin("other-plugin@test"),
        /*plugin_order*/ 1,
        server("https://other-plugin.example/mcp"),
    ));
    builder.register(McpServerRegistration::from_config(
        "docs".to_string(),
        server("https://config.example/mcp"),
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
    assert!(catalog.plugin_attributions_by_server_name().is_empty());
    assert_eq!(
        catalog.conflicts(),
        &[McpServerConflict {
            name: "docs".to_string(),
            outcome: register(extension_source("hosted")),
            contenders: vec![
                register(plugin_source("other-plugin@test")),
                register(plugin_source("plugin@test")),
            ],
        }]
    );
}

#[test]
fn disabled_veto_only_disables_the_winning_registration() {
    let extension = server("https://extension.example/mcp");
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
fn disabled_winner_remains_a_veto_when_the_catalog_is_extended() {
    let mut disabled = server("https://config.example/mcp");
    disabled.enabled = false;
    let mut expected = server("https://extension.example/mcp");
    expected.enabled = false;
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_config(
        "docs".to_string(),
        disabled,
    ));
    let mut builder = builder.build().to_builder();
    builder.register(McpServerRegistration::from_extension(
        "docs".to_string(),
        "hosted",
        /*contribution_order*/ 0,
        server("https://extension.example/mcp"),
    ));

    let resolved = builder.build();

    assert_eq!(
        resolved.server("docs"),
        Some(&resolved_server(extension_source("hosted"), expected))
    );
}

#[test]
fn disabled_discovered_plugin_remains_a_veto_for_runtime_overlays() {
    let mut disabled = server("https://plugin.example/mcp");
    disabled.enabled = false;
    let mut expected = server("https://extension.example/mcp");
    expected.enabled = false;
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_plugin(
        "docs".to_string(),
        plugin("plugin@test"),
        /*plugin_order*/ 0,
        disabled,
    ));
    let mut builder = builder.build().to_builder();
    builder.register(McpServerRegistration::from_extension(
        "docs".to_string(),
        "hosted",
        /*contribution_order*/ 0,
        server("https://extension.example/mcp"),
    ));

    let resolved = builder.build();

    assert_eq!(
        resolved.server("docs"),
        Some(&resolved_server(extension_source("hosted"), expected))
    );
}

#[test]
fn earlier_plugin_wins_with_an_explicit_conflict() {
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_plugin(
        "docs".to_string(),
        plugin("alpha@test"),
        /*plugin_order*/ 0,
        server("https://alpha.example/mcp"),
    ));
    builder.register(McpServerRegistration::from_plugin(
        "docs".to_string(),
        plugin("beta@test"),
        /*plugin_order*/ 1,
        server("https://beta.example/mcp"),
    ));

    let catalog = builder.build();

    assert_eq!(
        catalog.plugin_attributions_by_server_name(),
        HashMap::from([("docs".to_string(), plugin("alpha@test"))])
    );
    assert_eq!(
        catalog.conflicts(),
        &[McpServerConflict {
            name: "docs".to_string(),
            outcome: register(plugin_source("alpha@test")),
            contenders: vec![
                register(plugin_source("beta@test")),
                register(plugin_source("alpha@test")),
            ],
        }]
    );
}

#[test]
fn selected_plugins_override_discovered_plugins_but_not_config() {
    let selected = server("https://selected-alpha.example/mcp");
    let mut discovered = server("https://local.example/mcp");
    discovered.enabled = false;
    discovered.default_tools_approval_mode = Some(McpToolApproval::Auto);
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_plugin(
        "docs".to_string(),
        plugin("local@test"),
        /*plugin_order*/ 0,
        discovered,
    ));
    builder.register(McpServerRegistration::from_selected_plugin(
        "docs".to_string(),
        plugin("selected-beta"),
        /*selection_order*/ 1,
        server("https://selected-beta.example/mcp"),
    ));
    builder.register(McpServerRegistration::from_selected_plugin(
        "docs".to_string(),
        plugin("selected-alpha"),
        /*selection_order*/ 0,
        selected.clone(),
    ));

    let catalog = builder.build();

    assert_eq!(
        catalog.server("docs"),
        Some(&resolved_server(
            selected_plugin_source("selected-alpha"),
            selected,
        ))
    );
    assert_eq!(
        catalog.plugin_attributions_by_server_name(),
        HashMap::from([("docs".to_string(), plugin("selected-alpha"))])
    );
    assert_eq!(
        catalog.conflicts(),
        &[McpServerConflict {
            name: "docs".to_string(),
            outcome: register(selected_plugin_source("selected-alpha")),
            contenders: vec![
                register(selected_plugin_source("selected-beta")),
                register(selected_plugin_source("selected-alpha")),
            ],
        }]
    );

    let mut builder = catalog.to_builder();
    let configured = server("https://config.example/mcp");
    builder.register(McpServerRegistration::from_config(
        "docs".to_string(),
        configured.clone(),
    ));
    let catalog = builder.build();

    assert_eq!(
        catalog.server("docs"),
        Some(&resolved_server(McpServerSource::Config, configured))
    );
}

#[test]
fn selected_plugin_recomputes_a_disabled_discovered_plugin_veto_before_overlays() {
    let mut disabled = server("https://discovered.example/mcp");
    disabled.enabled = false;
    let selected = server("https://selected.example/mcp");
    let extension = server("https://extension.example/mcp");
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_plugin(
        "docs".to_string(),
        plugin("discovered"),
        /*plugin_order*/ 0,
        disabled,
    ));

    let mut builder = builder.build().to_builder_recomputing_disabled_vetoes();
    builder.register(McpServerRegistration::from_selected_plugin(
        "docs".to_string(),
        plugin("selected"),
        /*selection_order*/ 0,
        selected.clone(),
    ));
    let catalog = builder.build();

    assert_eq!(
        catalog.server("docs"),
        Some(&resolved_server(
            selected_plugin_source("selected"),
            selected,
        ))
    );

    let mut builder = catalog.to_builder();
    builder.register(McpServerRegistration::from_extension(
        "docs".to_string(),
        "runtime",
        /*contribution_order*/ 0,
        extension.clone(),
    ));

    assert_eq!(
        builder.build().server("docs"),
        Some(&resolved_server(extension_source("runtime"), extension))
    );
}

#[test]
fn selected_plugin_rebuild_preserves_an_explicit_disabled_name_veto() {
    let selected = server("https://selected.example/mcp");
    let extension = server("https://extension.example/mcp");
    let mut expected_extension = extension.clone();
    expected_extension.enabled = false;
    let mut builder = ResolvedMcpCatalog::builder();
    builder.disable("docs".to_string());

    let mut builder = builder.build().to_builder_recomputing_disabled_vetoes();
    builder.register(McpServerRegistration::from_selected_plugin(
        "docs".to_string(),
        plugin("selected"),
        /*selection_order*/ 0,
        selected,
    ));
    let catalog = builder.build();
    assert!(
        !catalog
            .server("docs")
            .expect("selected server")
            .config()
            .enabled
    );

    let mut builder = catalog.to_builder();
    builder.register(McpServerRegistration::from_extension(
        "docs".to_string(),
        "runtime",
        /*contribution_order*/ 0,
        extension,
    ));

    assert_eq!(
        builder.build().server("docs"),
        Some(&resolved_server(
            extension_source("runtime"),
            expected_extension,
        ))
    );
}

#[test]
fn disabled_selected_plugin_does_not_veto_runtime_overlays() {
    let mut disabled = server("https://selected.example/mcp");
    disabled.enabled = false;
    let extension = server("https://extension.example/mcp");
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_selected_plugin(
        "docs".to_string(),
        plugin("selected"),
        /*selection_order*/ 0,
        disabled,
    ));
    let mut builder = builder.build().to_builder();
    builder.register(McpServerRegistration::from_extension(
        "docs".to_string(),
        "hosted",
        /*contribution_order*/ 0,
        extension.clone(),
    ));

    let resolved = builder.build();

    assert_eq!(
        resolved.server("docs"),
        Some(&resolved_server(extension_source("hosted"), extension))
    );
}

#[test]
fn equal_precedence_uses_insertion_order_not_source_identity() {
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(McpServerRegistration::from_extension(
        "docs".to_string(),
        "z-first",
        /*contribution_order*/ 0,
        server("https://first.example/mcp"),
    ));
    builder.register(McpServerRegistration::from_extension(
        "docs".to_string(),
        "a-second",
        /*contribution_order*/ 0,
        server("https://second.example/mcp"),
    ));

    let catalog = builder.build();

    assert_eq!(
        catalog.server("docs"),
        Some(&resolved_server(
            extension_source("a-second"),
            server("https://second.example/mcp"),
        ))
    );
    let mut builder = catalog.to_builder();
    builder.remove_extension(
        "docs".to_string(),
        "remove-last",
        /*contribution_order*/ 0,
    );

    let catalog = builder.build();

    assert_eq!(catalog.server("docs"), None);
    assert_eq!(
        catalog.conflicts(),
        &[McpServerConflict {
            name: "docs".to_string(),
            outcome: remove(extension_source("remove-last")),
            contenders: vec![
                register(extension_source("z-first")),
                register(extension_source("a-second")),
                remove(extension_source("remove-last")),
            ],
        }]
    );
}
