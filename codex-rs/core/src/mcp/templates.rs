use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use serde_json::Value as JsonValue;

use crate::config_types::McpTemplate;

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

    fn default_template_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/mcp_templates")
    }
}

/// Utility to validate that a JSON blob can be deserialized as `McpTemplate`.
pub fn validate_template_json(json: &JsonValue) -> Result<McpTemplate> {
    let template: McpTemplate = serde_json::from_value(json.clone())?;
    Ok(template)
}
