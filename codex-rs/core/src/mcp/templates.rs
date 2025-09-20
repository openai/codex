use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use serde_json::Value as JsonValue;

use crate::config_types::McpServerConfig;
use crate::config_types::McpTemplate;
use crate::config_types::McpTemplateDefaults;

/// Container for built-in and dynamically loaded MCP templates.
#[derive(Default, Clone)]
pub struct TemplateCatalog {
    templates: HashMap<String, McpTemplate>,
}

impl TemplateCatalog {
    /// Create an empty catalog.
    pub fn empty() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    /// Load templates from the default resources directory.
    pub fn load_default() -> Result<Self> {
        let root = Self::default_template_dir();
        if !root.exists() {
            return Ok(Self::empty());
        }

        Self::load_from_dir(&root)
    }

    /// Load templates from a specific directory.
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let mut catalog = HashMap::new();
        if !dir.is_dir() {
            return Ok(Self { templates: catalog });
        }

        for entry in
            fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let contents = fs::read_to_string(&path)
                .with_context(|| format!("failed to read template {}", path.display()))?;
            match serde_json::from_str::<HashMap<String, McpTemplate>>(&contents) {
                Ok(map) => catalog.extend(map),
                Err(err) => {
                    tracing::warn!("Failed to parse MCP template {}: {err}", path.display());
                }
            }
        }

        Ok(Self { templates: catalog })
    }

    pub fn templates(&self) -> &HashMap<String, McpTemplate> {
        &self.templates
    }

    pub fn instantiate(&self, template_id: &str) -> Option<McpServerConfig> {
        let template = self.templates.get(template_id)?;
        let mut config = McpServerConfig {
            template_id: Some(template_id.to_string()),
            display_name: template.summary.clone(),
            category: template.category.clone(),
            metadata: template.metadata.clone(),
            ..McpServerConfig::default()
        };

        if let Some(defaults) = template.defaults.as_ref() {
            defaults.apply_to(&mut config);
        }

        Some(config)
    }

    fn default_template_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/mcp_templates")
    }
}

/// Utility to validate that a JSON blob can be deserialized as `McpTemplate`.
pub fn validate_template_json(json: &JsonValue) -> Result<McpTemplate> {
    let template: McpTemplate = serde_json::from_value(json.clone())?;
    Ok(template)
}

impl McpTemplateDefaults {
    pub fn apply_to(&self, config: &mut McpServerConfig) {
        if let Some(command) = &self.command {
            config.command = command.clone();
        }
        if !self.args.is_empty() {
            config.args = self.args.clone();
        }
        if let Some(env) = &self.env {
            if env.is_empty() {
                config.env = None;
            } else {
                config.env = Some(env.clone());
            }
        }
        if let Some(auth) = &self.auth {
            config.auth = Some(auth.clone());
        }
        if let Some(health) = &self.healthcheck {
            config.healthcheck = Some(health.clone());
        }
        if !self.tags.is_empty() {
            config.tags = self.tags.clone();
        }
        if let Some(timeout) = self.startup_timeout_ms {
            config.startup_timeout_ms = Some(timeout);
        }
        if let Some(description) = &self.description {
            config.description = Some(description.clone());
        }
        if let Some(metadata) = &self.metadata {
            config.metadata = Some(metadata.clone());
        }
    }
}
