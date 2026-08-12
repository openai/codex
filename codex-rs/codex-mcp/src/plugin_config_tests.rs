use super::PluginMcpConfigParseOutcome;
use super::PluginMcpServerParseError;
use super::parse_agent_plugin_mcp_config;
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

#[test]
fn agent_plugin_placeholder_expansion_is_single_pass() {
    let plugin_root = plugin_root().join("${PLUGIN_DATA}");
    let plugin_data_root = plugin_root
        .parent()
        .expect("plugin root parent")
        .join("plugin-data");
    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_data_root,
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers":{"demo":{
            "type":"stdio",
            "command":"python",
            "args":["${PLUGIN_ROOT}:${PLUGIN_DATA}"]
          }}
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    let McpServerTransportConfig::Stdio { args, .. } = &outcome.servers["demo"].transport else {
        panic!("expected stdio transport");
    };
    assert_eq!(
        args,
        &vec![format!(
            "{}:{}",
            plugin_root.display(),
            plugin_data_root.display()
        )]
    );
}

#[test]
fn agent_plugin_mcp_expands_reserved_paths_and_maps_transports() {
    let plugin_root = plugin_root();
    let plugin_data_root = plugin_root.parent().expect("parent").join("plugin-data");
    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_data_root,
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers": {
            "local": {
              "type":"stdio",
              "command":"python",
              "args":["${PLUGIN_ROOT}/server.py", "${PLUGIN_DATA}/state.json"],
              "env":{"CACHE":"${PLUGIN_DATA}/cache"},
              "cwd":"${PLUGIN_ROOT}/scripts"
            },
            "remote": {
              "type":"streamable-http",
              "url":"https://example.com/mcp",
              "headers":{"X-Plugin":"demo"}
            }
          }
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    assert!(outcome.errors.is_empty());
    let local = outcome.servers.get("local").expect("local server");
    let McpServerTransportConfig::Stdio { args, env, cwd, .. } = &local.transport else {
        panic!("expected stdio transport");
    };
    assert_eq!(
        args,
        &vec![
            format!("{}/server.py", plugin_root.display()),
            format!("{}/state.json", plugin_data_root.display()),
        ]
    );
    assert_eq!(
        env.as_ref().expect("environment").get("PLUGIN_ROOT"),
        Some(&plugin_root.display().to_string())
    );
    assert_eq!(
        env.as_ref().expect("environment").get("PLUGIN_DATA"),
        Some(&plugin_data_root.display().to_string())
    );
    assert_eq!(
        cwd.as_ref(),
        Some(&LegacyAppPathString::from_path(
            &plugin_root.join("scripts")
        ))
    );

    let remote = outcome.servers.get("remote").expect("remote server");
    let McpServerTransportConfig::StreamableHttp { http_headers, .. } = &remote.transport else {
        panic!("expected HTTP transport");
    };
    assert_eq!(
        http_headers
            .as_ref()
            .and_then(|headers| headers.get("X-Plugin")),
        Some(&"demo".to_string())
    );
}

#[test]
fn agent_plugin_mcp_handles_portable_path_and_http_edge_cases() {
    let plugin_root = plugin_root();
    let plugin_data_root = plugin_root.parent().expect("parent").join("plugin-data");
    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_data_root,
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers": {
            "contained":{"type":"stdio","command":"./bin/../server","cwd":"${PLUGIN_ROOT}/work/../data"},
            "redundant-separator":{"type":"stdio","command":".//bin/server"},
            "root-slash":{"type":"stdio","command":"python","cwd":"${PLUGIN_ROOT}/"},
            "data-slash":{"type":"stdio","command":"python","cwd":"${PLUGIN_DATA}/"},
            "headers":{"type":"streamable-http","url":"https://example.com/mcp","headers":{"aUtHoRiZaTiOn":"public-package-value","Content-Length":"0","HOST":"other.example.com","Proxy-Authorization":"public-package-value","Transfer-Encoding":"chunked","uSeR-aGeNt":"plugin-agent/1.0","X-Plugin":"demo","X-Plugin-Name":"café"}},
            "loopback":{"type":"streamable-http","url":"http://[::1]/mcp"}
          }
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    assert!(outcome.errors.is_empty());
    assert_eq!(
        outcome.servers.keys().collect::<Vec<_>>(),
        vec![
            "contained",
            "data-slash",
            "headers",
            "loopback",
            "redundant-separator",
            "root-slash"
        ]
    );
    let McpServerTransportConfig::Stdio { command, cwd, .. } =
        &outcome.servers["contained"].transport
    else {
        panic!("expected stdio transport");
    };
    assert_eq!(command, &plugin_root.join("server").display().to_string());
    assert_eq!(
        cwd.as_ref(),
        Some(&LegacyAppPathString::from_path(&plugin_root.join("data")))
    );
    let McpServerTransportConfig::Stdio { command, .. } =
        &outcome.servers["redundant-separator"].transport
    else {
        panic!("expected stdio transport");
    };
    assert_eq!(
        command,
        &plugin_root.join("bin").join("server").display().to_string()
    );
    for (server_name, expected_cwd) in [
        ("root-slash", plugin_root.as_path()),
        ("data-slash", plugin_data_root.as_path()),
    ] {
        let McpServerTransportConfig::Stdio { cwd, .. } = &outcome.servers[server_name].transport
        else {
            panic!("expected stdio transport");
        };
        assert_eq!(
            cwd.as_ref(),
            Some(&LegacyAppPathString::from_path(expected_cwd))
        );
    }
    let McpServerTransportConfig::StreamableHttp { http_headers, .. } =
        &outcome.servers["headers"].transport
    else {
        panic!("expected HTTP transport");
    };
    assert_eq!(
        http_headers,
        &Some(HashMap::from([
            ("X-Plugin".to_string(), "demo".to_string()),
            ("X-Plugin-Name".to_string(), "café".to_string()),
        ]))
    );
}

#[test]
fn agent_plugin_mcp_skips_invalid_server_without_disabling_siblings() {
    let plugin_root = plugin_root();
    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_root.join("data"),
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers": {
            "valid":{"type":"stdio","command":"python"},
            "reserved":{"type":"stdio","command":"python","env":{"PLUGIN_ROOT":"bad"}}
          }
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    assert_eq!(outcome.servers.keys().collect::<Vec<_>>(), vec!["valid"]);
    assert_eq!(outcome.errors.len(), 1);
    assert_eq!(outcome.errors[0].name, "reserved");
    assert!(
        outcome.errors[0]
            .message
            .contains("reserved variable `PLUGIN_ROOT`")
    );
}

#[test]
fn agent_plugin_mcp_preserves_server_named_mcp_servers() {
    let plugin_root = plugin_root();
    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_root.join("data"),
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers": {
            "mcpServers":{"type":"stdio","command":"first"},
            "sibling":{"type":"stdio","command":"second"}
          }
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    assert!(outcome.errors.is_empty());
    assert_eq!(
        outcome.servers.keys().collect::<Vec<_>>(),
        vec!["mcpServers", "sibling"]
    );
}

#[test]
fn agent_plugin_mcp_preserves_arbitrary_server_names() {
    let plugin_root = plugin_root();
    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_root.join("data"),
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers":{"agent.smoke / local":{"type":"stdio","command":"python"}}
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    assert!(outcome.errors.is_empty());
    assert!(outcome.servers.contains_key("agent.smoke / local"));
}

#[test]
fn agent_plugin_mcp_rejects_explicit_null_optional_fields() {
    let plugin_root = plugin_root();
    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_root.join("data"),
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers":{
            "cwd":{"type":"stdio","command":"python","cwd":null},
            "headers":{"type":"streamable-http","url":"https://example.com/mcp","headers":null}
          }
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    assert!(outcome.servers.is_empty());
    assert_eq!(outcome.errors.len(), 2);
    assert!(
        outcome
            .errors
            .iter()
            .any(|error| error.message.contains("`cwd`"))
    );
    assert!(
        outcome
            .errors
            .iter()
            .any(|error| error.message.contains("`headers`"))
    );
}

#[cfg(windows)]
#[test]
fn agent_plugin_mcp_rejects_reserved_environment_aliases_case_insensitively() {
    let plugin_root = plugin_root();
    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_root.join("data"),
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers":{"reserved":{"type":"stdio","command":"python","env":{"plugin_root":"bad"}}}
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    assert!(outcome.servers.is_empty());
    assert_eq!(outcome.errors.len(), 1);
}

#[cfg(windows)]
#[test]
fn agent_plugin_mcp_overlays_windows_environment_case_insensitively() {
    let plugin_root = plugin_root();
    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_root.join("data"),
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers":{
            "configured":{"type":"stdio","command":"python","env":{"Path":"configured"}},
            "duplicate":{"type":"stdio","command":"python","env":{"PATH":"one","Path":"two"}}
          }
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    assert_eq!(
        outcome.servers.keys().collect::<Vec<_>>(),
        vec!["configured"]
    );
    assert_eq!(outcome.errors.len(), 1);
    let McpServerTransportConfig::Stdio { env, .. } = &outcome.servers["configured"].transport
    else {
        panic!("expected stdio transport");
    };
    assert_eq!(
        env.as_ref().and_then(|env| env.get("PATH")),
        Some(&"configured".to_string())
    );
}

#[cfg(unix)]
#[test]
fn agent_plugin_mcp_resolves_root_before_collapsing_parent_components() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("temporary directory");
    let base = temp.path().join("base");
    let outside = temp.path().join("outside");
    let outside_directory = outside.join("directory");
    let resolved_plugin_root = outside.join("plugin");
    let plugin_data_root = temp.path().join("plugin-data");
    std::fs::create_dir_all(&base).expect("create base directory");
    std::fs::create_dir_all(&outside_directory).expect("create symlink target");
    std::fs::create_dir_all(&resolved_plugin_root).expect("create resolved plugin root");
    std::fs::create_dir_all(&plugin_data_root).expect("create plugin data root");
    let canonical_plugin_root = resolved_plugin_root
        .canonicalize()
        .expect("canonical plugin root");
    symlink(&outside_directory, base.join("link")).expect("create root symlink");
    let plugin_root = base.join("link/../plugin");

    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_data_root,
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers":{"demo":{"type":"stdio","command":"python"}}
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    assert!(outcome.errors.is_empty());
    let McpServerTransportConfig::Stdio { env, cwd, .. } = &outcome.servers["demo"].transport
    else {
        panic!("expected stdio transport");
    };
    assert_eq!(
        env.as_ref().and_then(|env| env.get("PLUGIN_ROOT")),
        Some(&canonical_plugin_root.display().to_string())
    );
    assert_eq!(
        cwd.as_ref(),
        Some(&LegacyAppPathString::from_path(&canonical_plugin_root))
    );
}

#[cfg(unix)]
#[test]
fn agent_plugin_mcp_rejects_missing_descendant_below_escaping_symlink() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("temporary directory");
    let plugin_root = temp.path().join("plugin");
    let plugin_data_root = temp.path().join("plugin-data");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&plugin_root).expect("create plugin root");
    std::fs::create_dir_all(&plugin_data_root).expect("create plugin data root");
    std::fs::create_dir_all(&outside).expect("create outside directory");
    symlink(&outside, plugin_root.join("link")).expect("create escaping symlink");

    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_data_root,
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers":{"escape":{"type":"stdio","command":"./link/missing"}}
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    assert!(outcome.servers.is_empty());
    assert_eq!(outcome.errors.len(), 1);
    assert!(outcome.errors[0].message.contains("must remain within"));
}

#[test]
fn agent_plugin_mcp_enforces_closed_transport_and_path_semantics() {
    let plugin_root = plugin_root();
    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_root.join("data"),
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers": {
            "valid":{"type":"stdio","command":"python"},
            "command":{"type":"stdio","command":"../server"},
            "escape":{"type":"stdio","command":"./../server"},
            "cwd":{"type":"stdio","command":"python","cwd":"${PLUGIN_ROOT}/../outside"},
            "backslash":{"type":"stdio","command":"./scripts\\..\\outside"},
            "remote":{"type":"streamable-http","url":"http://example.com/mcp"},
            "header":{"type":"streamable-http","url":"https://example.com/mcp","headers":{"X-Demo":"one","x-demo":"two"}},
            "sse":{"type":"sse","url":"https://example.com/sse"},
            "unknown":{"type":"stdio","command":"python","future":true}
          }
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    assert_eq!(outcome.servers.keys().collect::<Vec<_>>(), vec!["valid"]);
    assert_eq!(outcome.errors.len(), 8);
}

#[cfg(windows)]
#[test]
fn agent_plugin_mcp_rejects_drive_relative_windows_command() {
    let plugin_root = plugin_root();
    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_root.join("data"),
        r#"{"$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json","mcpServers":{"drive-relative":{"type":"stdio","command":"C:server.exe"}}}"#,
    )
    .expect("parse Agent Plugins MCP config");
    assert!(outcome.servers.is_empty());
    assert_eq!(outcome.errors.len(), 1);
}

#[test]
fn agent_plugin_mcp_treats_args_and_env_as_opaque_after_expansion() {
    let plugin_root = plugin_root();
    let data_root = plugin_root.join("data");
    let outcome = parse_agent_plugin_mcp_config(
        &plugin_root,
        &data_root,
        r#"{
          "$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
          "mcpServers":{"demo":{
            "type":"stdio",
            "command":"python",
            "args":["${PLUGIN_ROOT}/../opaque"],
            "env":{"OPAQUE":"${PLUGIN_DATA}/../opaque"}
          }}
        }"#,
    )
    .expect("parse Agent Plugins MCP config");

    assert!(outcome.errors.is_empty());
    let McpServerTransportConfig::Stdio { args, env, .. } = &outcome.servers["demo"].transport
    else {
        panic!("expected stdio transport");
    };
    assert_eq!(args, &vec![format!("{}/../opaque", plugin_root.display())]);
    assert_eq!(
        env.as_ref().and_then(|env| env.get("OPAQUE")),
        Some(&format!("{}/../opaque", data_root.display()))
    );
}

#[test]
fn agent_plugin_mcp_rejects_unsupported_schema() {
    let plugin_root = plugin_root();
    let error = parse_agent_plugin_mcp_config(
        &plugin_root,
        &plugin_root.join("data"),
        r#"{"$schema":"https://agent-plugins.org/schemas/2.0.0/mcp.schema.json","mcpServers":{}}"#,
    )
    .expect_err("unsupported schema");

    assert!(
        error
            .to_string()
            .contains("unsupported Agent Plugins MCP schema")
    );
}

fn stdio_server(
    command: &str,
    environment_id: &str,
    cwd: LegacyAppPathString,
    env_vars: Vec<McpServerEnvVar>,
) -> McpServerConfig {
    McpServerConfig {
        auth: Default::default(),
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
        omit_tools_from: None,
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
        auth: Default::default(),
        transport: McpServerTransportConfig::StreamableHttp {
            url: "https://example.com/mcp".to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
            http_headers_helper: None,
        },
        environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        omit_tools_from: None,
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
    let mut expected_helper = McpServerConfig {
        oauth: None,
        ..expected_http.clone()
    };
    let McpServerTransportConfig::StreamableHttp {
        http_headers_helper,
        ..
    } = &mut expected_helper.transport
    else {
        unreachable!("expected HTTP transport");
    };
    *http_headers_helper = Some("./auth.sh".to_string());

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
            },
            "helper": {"type":"http","url":"https://example.com/mcp","http_headers_helper":"./auth.sh"}
        }"#,
    )
    .expect("parse plugin MCP config");

    assert_eq!(
        outcome,
        PluginMcpConfigParseOutcome {
            servers: BTreeMap::from([
                ("demo".to_string(), expected_stdio),
                ("helper".to_string(), expected_helper),
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
fn remote_environment_placement_rejects_http_env_references() {
    let plugin_root = plugin_root();
    let outcome = parse_executor_plugin_mcp_config(
        &plugin_root_uri(&plugin_root),
        r#"{
            "bearer": {
                "url": "https://example.com/bearer",
                "bearer_token_env_var": "TOKEN"
            },
            "headers": {
                "url": "https://example.com/headers",
                "env_http_headers": {"Authorization": "TOKEN"}
            }
        }"#,
        "executor-1",
    )
    .expect("parse plugin MCP config");

    assert_eq!(
        outcome,
        PluginMcpConfigParseOutcome {
            servers: BTreeMap::new(),
            errors: vec![
                PluginMcpServerParseError {
                    name: "bearer".to_string(),
                    message: "`bearer_token_env_var` requires executor-side environment resolution for an executor-owned HTTP MCP"
                        .to_string(),
                },
                PluginMcpServerParseError {
                    name: "headers".to_string(),
                    message: "`env_http_headers` requires executor-side environment resolution for an executor-owned HTTP MCP"
                        .to_string(),
                },
            ],
        }
    );
}

#[test]
fn local_environment_placement_preserves_http_env_references() {
    let plugin_root = plugin_root();
    let outcome = parse_executor_plugin_mcp_config(
        &plugin_root_uri(&plugin_root),
        r#"{
            "demo": {
                "url": "https://example.com/mcp",
                "bearer_token_env_var": "TOKEN",
                "env_http_headers": {"X-Account": "ACCOUNT_ID"}
            }
        }"#,
        DEFAULT_MCP_SERVER_ENVIRONMENT_ID,
    )
    .expect("parse plugin MCP config");

    assert_eq!(
        outcome,
        PluginMcpConfigParseOutcome {
            servers: BTreeMap::from([(
                "demo".to_string(),
                McpServerConfig {
                    auth: Default::default(),
                    transport: McpServerTransportConfig::StreamableHttp {
                        url: "https://example.com/mcp".to_string(),
                        bearer_token_env_var: Some("TOKEN".to_string()),
                        http_headers: None,
                        env_http_headers: Some(HashMap::from([(
                            "X-Account".to_string(),
                            "ACCOUNT_ID".to_string(),
                        )])),
                        http_headers_helper: None,
                    },
                    environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                    enabled: true,
                    required: false,
                    supports_parallel_tool_calls: false,
                    omit_tools_from: None,
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
            )]),
            errors: Vec::new(),
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
