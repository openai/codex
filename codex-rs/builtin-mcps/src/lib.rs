//! Built-in MCP servers shipped with Codex.
//!
//! This crate owns the catalog of product-owned MCP servers and the small
//! amount of server-specific dispatch needed to run them. Runtime placement is
//! chosen by `codex-mcp`; built-ins should not be flattened into user-facing
//! MCP server config just to make them launchable.

use std::path::Path;

use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;

pub const MEMORIES_MCP_SERVER_NAME: &str = "memories";

/// Product-owned MCP servers that Codex can provide without user config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinMcpServer {
    Memories,
}

impl BuiltinMcpServer {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Memories => MEMORIES_MCP_SERVER_NAME,
        }
    }

    pub const fn supports_parallel_tool_calls(self) -> bool {
        match self {
            Self::Memories => true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BuiltinMcpServerOptions {
    pub memories_enabled: bool,
}

pub fn enabled_builtin_mcp_servers(options: BuiltinMcpServerOptions) -> Vec<BuiltinMcpServer> {
    let mut servers = Vec::new();
    if options.memories_enabled {
        servers.push(BuiltinMcpServer::Memories);
    }
    servers
}

pub async fn serve_builtin_mcp_server<T>(
    server: BuiltinMcpServer,
    codex_home: &Path,
    transport: T,
) -> anyhow::Result<()>
where
    T: AsyncRead + AsyncWrite + Send + 'static,
{
    match server {
        BuiltinMcpServer::Memories => {
            let codex_home = codex_utils_absolute_path::AbsolutePathBuf::try_from(codex_home)?;
            codex_memories_mcp::run_server(&codex_home, transport).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn enabled_builtin_mcp_servers_adds_memories_when_enabled() {
        assert_eq!(
            enabled_builtin_mcp_servers(BuiltinMcpServerOptions {
                memories_enabled: true,
            }),
            vec![BuiltinMcpServer::Memories]
        );
    }

    #[test]
    fn enabled_builtin_mcp_servers_omits_memories_when_disabled() {
        assert_eq!(
            enabled_builtin_mcp_servers(BuiltinMcpServerOptions {
                memories_enabled: false,
            }),
            Vec::<BuiltinMcpServer>::new()
        );
    }
}
