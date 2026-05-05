//! Built-in MCP servers shipped with Codex.
//!
//! Built-ins use the same stdio MCP path as user-configured servers, but are
//! declared here so product-owned MCPs do not need to live in `codex-core`.

use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;

pub const MEMORIES_MCP_SERVER_NAME: &str = "memories";
const BUILTIN_MCP_SUBCOMMAND: &str = "builtin-mcp";

#[derive(Debug, Clone, Copy)]
pub struct BuiltinMcpServerOptions<'a> {
    pub codex_self_exe: Option<&'a Path>,
    pub codex_home: &'a Path,
    pub memories_enabled: bool,
}

pub fn configured_builtin_mcp_servers(
    options: BuiltinMcpServerOptions<'_>,
) -> HashMap<String, McpServerConfig> {
    let Some(codex_self_exe) = options.codex_self_exe else {
        return HashMap::new();
    };

    let mut servers = HashMap::new();
    if options.memories_enabled {
        servers.insert(
            MEMORIES_MCP_SERVER_NAME.to_string(),
            builtin_stdio_server_config(
                codex_self_exe,
                options.codex_home,
                MEMORIES_MCP_SERVER_NAME,
            ),
        );
    }
    servers
}

pub async fn run_builtin_mcp_server(
    name: &str,
    codex_home: &AbsolutePathBuf,
) -> anyhow::Result<()> {
    match name {
        MEMORIES_MCP_SERVER_NAME => codex_memories_mcp::run_stdio_server(codex_home).await,
        _ => anyhow::bail!("unknown built-in MCP server: {name}"),
    }
}

fn builtin_stdio_server_config(
    codex_self_exe: &Path,
    codex_home: &Path,
    name: &str,
) -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::Stdio {
            command: codex_self_exe.to_string_lossy().into_owned(),
            args: vec![
                BUILTIN_MCP_SUBCOMMAND.to_string(),
                name.to_string(),
                "--codex-home".to_string(),
                codex_home.to_string_lossy().into_owned(),
            ],
            env: None,
            env_vars: Vec::new(),
            cwd: None,
        },
        experimental_environment: None,
        enabled: true,
        required: false,
        supports_parallel_tool_calls: true,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth_resource: None,
        tools: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn configured_builtin_mcp_servers_adds_memories_when_enabled() {
        let codex_home = AbsolutePathBuf::try_from("/tmp/codex-home").expect("absolute codex home");
        let servers = configured_builtin_mcp_servers(BuiltinMcpServerOptions {
            codex_self_exe: Some(Path::new("/tmp/codex")),
            codex_home: codex_home.as_path(),
            memories_enabled: true,
        });

        let server = servers
            .get(MEMORIES_MCP_SERVER_NAME)
            .expect("memories server should exist");
        assert_eq!(
            server.transport,
            McpServerTransportConfig::Stdio {
                command: "/tmp/codex".to_string(),
                args: vec![
                    "builtin-mcp".to_string(),
                    "memories".to_string(),
                    "--codex-home".to_string(),
                    "/tmp/codex-home".to_string(),
                ],
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            }
        );
    }

    #[test]
    fn configured_builtin_mcp_servers_requires_reexec_path() {
        let codex_home = AbsolutePathBuf::try_from("/tmp/codex-home").expect("absolute codex home");
        let servers = configured_builtin_mcp_servers(BuiltinMcpServerOptions {
            codex_self_exe: None,
            codex_home: codex_home.as_path(),
            memories_enabled: true,
        });

        assert!(servers.is_empty());
    }
}
