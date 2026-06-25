use codex_config::McpServerConfig;

/// Input supplied while resolving MCP server contributions.
pub struct McpServerContributionContext<'a, C> {
    /// Host configuration visible during MCP resolution.
    config: &'a C,
}

impl<C> Clone for McpServerContributionContext<'_, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for McpServerContributionContext<'_, C> {}

impl<'a, C> McpServerContributionContext<'a, C> {
    /// Creates context for resolution that is not associated with a running thread.
    pub fn global(config: &'a C) -> Self {
        Self { config }
    }

    /// Returns the host configuration visible during resolution.
    pub fn config(&self) -> &'a C {
        self.config
    }
}

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
