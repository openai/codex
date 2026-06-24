use super::PluginMcpConfigParseOutcome;
use super::PluginMcpServerParseError;
use super::parse_executor_plugin_mcp_config;
use super::parse_plugin_mcp_config;
use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::McpServerConfig;
use codex_config::McpServerEnvVar;
use codex_config::McpServerOAuthConfig;
use codex_config::McpServerTransportConfig;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

fn plugin_root() -> PathBuf {
    std::env::current_dir()
        .expect("current directory")
        .join("plugin-root")
}

fn plugin_root_uri(plugin_root: &Path) -> PathUri {
    PathUri::from_host_native_path(plugin_root).expect("plugin root URI")
}

fn stdio_server(
    command: &str,
    environment_id: &str,
    cwd: LegacyAppPathString,
    env_vars: Vec<McpServerEnvVar>,
) -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::Stdio {
            command: command.to_string(),
            args: Vec::new(),
            env: None,
            env_vars,
            cwd: Some(cwd),
        },
        environment_id: environment_id.to_string(),
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
fn declared_placement_preserves_local_plugin_normalization() {
    let plugin_root = plugin_root();
    let expected_stdio = stdio_server(
        "demo-mcp",
        DEFAULT_MCP_SERVER_ENVIRONMENT_ID,
        LegacyAppPathString::from_path(&plugin_root.join("scripts")),
        Vec::new(),
    );
    let expected_http = McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url: "https://example.com/mcp".to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
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
        oauth: Some(McpServerOAuthConfig {
            client_id: Some("client-id".to_string()),
        }),
        oauth_resource: None,
        tools: HashMap::new(),
    };

    let outcome = parse_plugin_mcp_config(
        &plugin_root,
        r#"{
            "demo": {
                "type": "stdio",
                "command": "demo-mcp",
                "cwd": "scripts"
            },
            "hosted": {
                "type": "http",
                "url": "https://example.com/mcp",
                "oauth": {"clientId": "client-id", "callbackPort": 9876}
            }
        }"#,
    )
    .expect("parse plugin MCP config");

    assert_eq!(
        outcome,
        PluginMcpConfigParseOutcome {
            servers: BTreeMap::from([
                ("demo".to_string(), expected_stdio),
                ("hosted".to_string(), expected_http),
            ]),
            errors: Vec::new(),
        }
    );
}

#[test]
fn environment_placement_forces_authority_and_defaults_null_cwd() {
    let plugin_root = plugin_root();
    let plugin_root_uri = plugin_root_uri(&plugin_root);
    let outcome = parse_executor_plugin_mcp_config(
        &plugin_root_uri,
        r#"{
            "$schema":"https://example.com/plugin-mcp.schema.json",
            "mcpServers":{"demo":{
                "command":"demo-mcp",
                "environment_id":"local",
                "cwd":null,
                "env_vars":["EXECUTOR_TOKEN", {"name":"OTHER_TOKEN"}]
            }}
        }"#,
        "executor-1",
    )
    .expect("parse plugin MCP config");

    assert_eq!(
        outcome,
        PluginMcpConfigParseOutcome {
            servers: BTreeMap::from([(
                "demo".to_string(),
                stdio_server(
                    "demo-mcp",
                    "executor-1",
                    plugin_root_uri.into(),
                    vec![
                        McpServerEnvVar::Config {
                            name: "EXECUTOR_TOKEN".to_string(),
                            source: Some("remote".to_string()),
                        },
                        McpServerEnvVar::Config {
                            name: "OTHER_TOKEN".to_string(),
                            source: Some("remote".to_string()),
                        },
                    ],
                ),
            )]),
            errors: Vec::new(),
        }
    );
}

#[test]
fn environment_placement_resolves_relative_cwd_beneath_plugin_root() {
    let plugin_root = plugin_root();
    let plugin_root_uri = plugin_root_uri(&plugin_root);
    let outcome = parse_executor_plugin_mcp_config(
        &plugin_root_uri,
        r#"{"demo":{"command":"demo-mcp","cwd":"scripts"}}"#,
        "executor-1",
    )
    .expect("parse plugin MCP config");

    assert_eq!(
        outcome,
        PluginMcpConfigParseOutcome {
            servers: BTreeMap::from([(
                "demo".to_string(),
                stdio_server(
                    "demo-mcp",
                    "executor-1",
                    plugin_root_uri
                        .join("scripts")
                        .expect("plugin cwd URI")
                        .into(),
                    Vec::new(),
                ),
            )]),
            errors: Vec::new(),
        }
    );
}

#[test]
fn executor_environment_placement_resolves_foreign_uri_cwd() {
    let plugin_root = PathUri::parse("file:///C:/plugins/demo").expect("plugin root URI");
    let outcome = parse_executor_plugin_mcp_config(
        &plugin_root,
        r#"{"demo":{"command":"demo-mcp","cwd":"scripts"}}"#,
        "executor-1",
    )
    .expect("parse plugin MCP config");

    assert_eq!(
        outcome,
        PluginMcpConfigParseOutcome {
            servers: BTreeMap::from([(
                "demo".to_string(),
                stdio_server(
                    "demo-mcp",
                    "executor-1",
                    LegacyAppPathString::from(
                        plugin_root.join("scripts").expect("executor cwd URI"),
                    ),
                    Vec::new(),
                ),
            )]),
            errors: Vec::new(),
        }
    );
}

#[test]
fn environment_placement_rejects_relative_cwd_that_escapes_package() {
    let plugin_root = plugin_root();
    let plugin_root_uri = plugin_root_uri(&plugin_root);
    let outcome = parse_executor_plugin_mcp_config(
        &plugin_root_uri,
        r#"{"demo":{"command":"demo-mcp","cwd":"../outside"}}"#,
        "executor-1",
    )
    .expect("parse plugin MCP config");

    assert_eq!(
        outcome,
        PluginMcpConfigParseOutcome {
            servers: BTreeMap::new(),
            errors: vec![PluginMcpServerParseError {
                name: "demo".to_string(),
                message: format!(
                    "cwd `../outside` must remain within plugin root `{plugin_root_uri}`"
                ),
            }],
        }
    );
}

#[test]
fn environment_placement_rejects_orchestrator_env_vars() {
    let plugin_root = plugin_root();
    let outcome = parse_executor_plugin_mcp_config(
        &plugin_root_uri(&plugin_root),
        r#"{"demo":{"command":"demo-mcp","env_vars":[{"name":"TOKEN","source":"local"}]}}"#,
        "executor-1",
    )
    .expect("parse plugin MCP config");

    assert_eq!(
        outcome,
        PluginMcpConfigParseOutcome {
            servers: BTreeMap::new(),
            errors: vec![PluginMcpServerParseError {
                name: "demo".to_string(),
                message:
                    "env_vars entry `TOKEN` cannot use source `local` in an executor-owned plugin"
                        .to_string(),
            }],
        }
    );
}

#[test]
fn local_environment_placement_preserves_local_env_vars() {
    let plugin_root = plugin_root();
    let plugin_root_uri = plugin_root_uri(&plugin_root);
    let outcome = parse_executor_plugin_mcp_config(
        &plugin_root_uri,
        r#"{"demo":{"command":"demo-mcp","env_vars":["TOKEN",{"name":"OTHER","source":"local"}]}}"#,
        DEFAULT_MCP_SERVER_ENVIRONMENT_ID,
    )
    .expect("parse plugin MCP config");

    assert_eq!(
        outcome,
        PluginMcpConfigParseOutcome {
            servers: BTreeMap::from([(
                "demo".to_string(),
                stdio_server(
                    "demo-mcp",
                    DEFAULT_MCP_SERVER_ENVIRONMENT_ID,
                    plugin_root_uri.into(),
                    vec![
                        McpServerEnvVar::Name("TOKEN".to_string()),
                        McpServerEnvVar::Config {
                            name: "OTHER".to_string(),
                            source: Some("local".to_string()),
                        },
                    ],
                ),
            )]),
            errors: Vec::new(),
        }
    );
}

#[test]
fn local_environment_placement_rejects_remote_env_vars() {
    let plugin_root = plugin_root();
    let outcome = parse_executor_plugin_mcp_config(
        &plugin_root_uri(&plugin_root),
        r#"{"demo":{"command":"demo-mcp","env_vars":[{"name":"TOKEN","source":"remote"}]}}"#,
        DEFAULT_MCP_SERVER_ENVIRONMENT_ID,
    )
    .expect("parse plugin MCP config");

    assert_eq!(
        outcome,
        PluginMcpConfigParseOutcome {
            servers: BTreeMap::new(),
            errors: vec![PluginMcpServerParseError {
                name: "demo".to_string(),
                message: "env_vars entry `TOKEN` cannot use source `remote` in a local environment"
                    .to_string(),
            }],
        }
    );
}
