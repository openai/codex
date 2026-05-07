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

#[derive(Debug, Clone, Copy)]
struct BuiltinMcpServerMetadata {
    name: &'static str,
    supports_parallel_tool_calls: bool,
    pollutes_memory: bool,
}

impl BuiltinMcpServer {
    const fn metadata(self) -> BuiltinMcpServerMetadata {
        match self {
            Self::Memories => BuiltinMcpServerMetadata {
                name: MEMORIES_MCP_SERVER_NAME,
                supports_parallel_tool_calls: true,
                pollutes_memory: false,
            },
        }
    }

    pub const fn name(self) -> &'static str {
        self.metadata().name
    }

    pub const fn supports_parallel_tool_calls(self) -> bool {
        self.metadata().supports_parallel_tool_calls
    }

    pub const fn pollutes_memory(self) -> bool {
        self.metadata().pollutes_memory
    }

    pub async fn serve<T>(self, codex_home: &Path, transport: T) -> anyhow::Result<()>
    where
        T: AsyncRead + AsyncWrite + Send + 'static,
    {
        match self {
            Self::Memories => {
                let codex_home = codex_utils_absolute_path::AbsolutePathBuf::try_from(codex_home)?;
                codex_memories_mcp::run_server(&codex_home, transport).await
            }
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
