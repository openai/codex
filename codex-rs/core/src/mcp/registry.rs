use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::config::load_global_mcp_servers;
use crate::config::write_global_mcp_servers;
use crate::config_types::McpServerConfig;
use crate::config_types::McpTemplate;
use crate::mcp::health::HealthReport;
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

    /// Resolve template defaults into a server configuration skeleton.
    pub fn instantiate_template(&self, template_id: &str) -> Option<McpServerConfig> {
        self.templates.instantiate(template_id)
    }

    /// Persist the provided server configuration under the given name.
    pub fn upsert_server(&self, name: &str, config: McpServerConfig) -> Result<()> {
        self.upsert_server_with_existing(None, name, config)
    }

    /// Persist a server configuration, optionally replacing an existing entry.
    /// When `existing_name` is provided and differs from `name`, the old entry is removed
    /// after the new one is written, ensuring we never drop the original configuration
    /// before a successful write of the replacement.
    pub fn upsert_server_with_existing(
        &self,
        existing_name: Option<&str>,
        name: &str,
        config: McpServerConfig,
    ) -> Result<()> {
        validate_server_name(name)?;
        let mut servers = load_global_mcp_servers(self.codex_home())?;
        servers.insert(name.to_string(), config);

        if let Some(old_name) = existing_name
            && old_name != name
        {
            servers.remove(old_name);
        }

        write_global_mcp_servers(self.codex_home(), &servers)?;
        Ok(())
    }

    /// Remove a server entry. Returns true if removed.
    pub fn remove_server(&self, name: &str) -> Result<bool> {
        let mut servers = load_global_mcp_servers(self.codex_home())?;
        let removed = servers.remove(name).is_some();
        if removed {
            write_global_mcp_servers(self.codex_home(), &servers)?;
        }
        Ok(removed)
    }

    /// Health status placeholder for UI surfaces.
    pub fn health_report(&self, _name: &str) -> HealthReport {
        HealthReport::unknown()
    }

    pub fn codex_home(&self) -> &Path {
        &self.config.codex_home
    }

    pub fn reload_servers(&self) -> Result<BTreeMap<String, McpServerConfig>> {
        Ok(load_global_mcp_servers(self.codex_home())?)
    }
}

pub fn validate_server_name(name: &str) -> Result<()> {
    let is_valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');

    if is_valid {
        Ok(())
    } else {
        anyhow::bail!("invalid server name '{name}' (use letters, numbers, '-', '_')")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::config::ConfigOverrides;
    use crate::config::ConfigToml;
    use crate::config_types::McpServerConfig;
    use crate::mcp::templates::TemplateCatalog;
    use tempfile::TempDir;

    fn make_config(temp: &TempDir) -> Config {
        Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            temp.path().to_path_buf(),
        )
        .expect("load config")
    }

    #[test]
    fn upsert_with_existing_renames_without_losing_original() {
        let tmp = TempDir::new().expect("tempdir");
        let config = make_config(&tmp);
        let registry = McpRegistry::new(&config, TemplateCatalog::empty());

        let mut initial = BTreeMap::new();
        initial.insert(
            "old".to_string(),
            McpServerConfig {
                command: "run-old".into(),
                ..Default::default()
            },
        );
        write_global_mcp_servers(registry.codex_home(), &initial).expect("seed config");

        let replacement = McpServerConfig {
            command: "run-new".into(),
            ..Default::default()
        };

        registry
            .upsert_server_with_existing(Some("old"), "new", replacement)
            .expect("rename server");

        let updated = load_global_mcp_servers(registry.codex_home()).expect("load config");
        assert!(updated.contains_key("new"));
        assert!(!updated.contains_key("old"));
        assert_eq!(updated["new"].command, "run-new".to_string());
    }

    #[test]
    fn upsert_with_existing_updates_in_place_when_name_unchanged() {
        let tmp = TempDir::new().expect("tempdir");
        let config = make_config(&tmp);
        let registry = McpRegistry::new(&config, TemplateCatalog::empty());

        let mut initial = BTreeMap::new();
        initial.insert(
            "service".to_string(),
            McpServerConfig {
                command: "original".into(),
                ..Default::default()
            },
        );
        write_global_mcp_servers(registry.codex_home(), &initial).expect("seed config");

        let replacement = McpServerConfig {
            command: "updated".into(),
            ..Default::default()
        };

        registry
            .upsert_server_with_existing(Some("service"), "service", replacement)
            .expect("update server");

        let updated = load_global_mcp_servers(registry.codex_home()).expect("load config");
        assert_eq!(updated.len(), 1);
        assert_eq!(updated["service"].command, "updated".to_string());
    }
}
