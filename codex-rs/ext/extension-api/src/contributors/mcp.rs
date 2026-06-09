use codex_config::McpServerConfig;

/// One extension-owned overlay for the runtime MCP server configuration.
#[derive(Clone, Debug)]
pub enum McpServerContribution {
    /// Adds or replaces a named MCP server.
    Set {
        name: String,
        config: Box<McpServerConfig>,
    },
    /// Removes a named MCP server.
    Remove { name: String },
}

impl McpServerContribution {
    /// Returns the stable server name owned by this contribution.
    pub fn name(&self) -> &str {
        match self {
            Self::Set { name, .. } | Self::Remove { name } => name,
        }
    }
}
