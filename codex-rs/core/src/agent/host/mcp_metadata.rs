//! Bind private agent attribution to one host-selected MCP plugin registration.

use codex_config::McpServerTransportConfig;
use codex_mcp::McpServerSource;
use codex_mcp::ResolvedMcpCatalog;
use codex_utils_path_uri::PathUri;

/// Install in thread extension data to send `x-codex-root-turn-id` and
/// `x-codex-cyber-access-program` to every tool on this plugin registration.
/// Only a registration from the host plugin manager in the `local` MCP environment can match.
/// The full plugin config name (e.g. `agents@marketplace`), installed plugin root,
/// server name, and loaded transport configuration must all match the effective registration.
/// Core omits both keys without this attachment or an exact match. Same-name config
/// overrides, thread-selected plugins, and extension registrations do not match.
/// A matching registration receives JSON null for an unset value.
/// Transport configuration may contain credentials and must not be logged.
#[derive(Clone)]
pub struct AgentMcpMetadataRecipient {
    pub plugin_id: String,
    pub plugin_root: PathUri,
    pub server_name: String,
    pub transport: McpServerTransportConfig,
}

impl AgentMcpMetadataRecipient {
    pub(crate) fn matches(&self, server_name: &str, catalog: &ResolvedMcpCatalog) -> bool {
        let Some(registration) = catalog.server(server_name) else {
            return false;
        };
        server_name == self.server_name
            && registration.config().is_local_environment()
            && registration.config().transport == self.transport
            && matches!(registration.source(), McpServerSource::Plugin(plugin)
                if plugin.plugin_id() == self.plugin_id
                    && plugin.host_root() == Some(&self.plugin_root))
    }
}

#[cfg(test)]
#[path = "mcp_metadata_tests.rs"]
mod tests;
