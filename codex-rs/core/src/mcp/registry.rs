use std::collections::HashMap;

use anyhow::Result;

use crate::config::Config;
use crate::config_types::McpServerConfig;
use crate::config_types::McpTemplate;
use crate::mcp::templates::TemplateCatalog;

/// Lightweight view into MCP configuration state (experimental).
pub struct McpRegistry<'a> {
    config: &'a Config,
    templates: TemplateCatalog,
}

impl<'a> McpRegistry<'a> {
    /// Construct a registry backed by the given config and template catalog.
    pub fn new(config: &'a Config, templates: TemplateCatalog) -> Self {
        Self { config, templates }
    }

    /// Whether overhaul features are enabled.
    pub fn experimental_enabled(&self) -> bool {
        self.config.experimental_mcp_overhaul
    }

    /// Iterate configured MCP servers.
    pub fn servers(&self) -> impl Iterator<Item = (&String, &McpServerConfig)> {
        self.config.mcp_servers.iter()
    }

    /// Retrieve a single server by name.
    pub fn server(&self, name: &str) -> Option<&McpServerConfig> {
        self.config.mcp_servers.get(name)
    }

    /// Return template metadata by id, if available.
    pub fn template(&self, template_id: &str) -> Option<&McpTemplate> {
        self.templates.templates().get(template_id)
    }

    /// All known templates.
    pub fn templates(&self) -> &HashMap<String, McpTemplate> {
        self.templates.templates()
    }

    /// Placeholder for future create/update functionality.
    pub fn create_server(&mut self, _name: &str, _config: McpServerConfig) -> Result<()> {
        // Implementation will be provided in subsequent phases.
        anyhow::bail!("MCP registry create_server is not yet implemented")
    }
}
