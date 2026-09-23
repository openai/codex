//! Stable plugin identifier parsing and validation shared with the plugin cache.

#[derive(Debug, thiserror::Error)]
pub enum PluginIdError {
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginId {
    pub plugin_name: String,
    pub marketplace_name: String,
}

impl PluginId {
    pub fn new(plugin_name: String, marketplace_name: String) -> Result<Self, PluginIdError> {
        validate_plugin_segment(&plugin_name, "plugin name").map_err(PluginIdError::Invalid)?;
        validate_plugin_segment(&marketplace_name, "marketplace name")
            .map_err(PluginIdError::Invalid)?;
        Ok(Self {
            plugin_name,
            marketplace_name,
        })
    }

    pub fn parse(plugin_key: &str) -> Result<Self, PluginIdError> {
        let Some((plugin_name, marketplace_name)) = plugin_key.rsplit_once('@') else {
            return Err(PluginIdError::Invalid(format!(
                "invalid plugin key `{plugin_key}`; expected <plugin>@<marketplace>"
            )));
        };
        if plugin_name.is_empty() || marketplace_name.is_empty() {
            return Err(PluginIdError::Invalid(format!(
                "invalid plugin key `{plugin_key}`; expected <plugin>@<marketplace>"
            )));
        }

        Self::new(plugin_name.to_string(), marketplace_name.to_string()).map_err(|err| match err {
            PluginIdError::Invalid(message) => {
                PluginIdError::Invalid(format!("{message} in `{plugin_key}`"))
            }
        })
    }

    pub fn as_key(&self) -> String {
        format!("{}@{}", self.plugin_name, self.marketplace_name)
    }
}

/// Validates a single path segment used in plugin IDs and cache layout.
pub fn validate_plugin_segment(segment: &str, kind: &str) -> Result<(), String> {
    if segment.is_empty() {
        return Err(format!("invalid {kind}: must not be empty"));
    }
    let allow_dots = kind == "plugin name";
    if allow_dots && matches!(segment, "." | "..") {
        return Err(format!("invalid {kind}: path traversal is not allowed"));
    }
    if allow_dots && (segment.starts_with('.') || segment.ends_with('.') || segment.contains(".."))
    {
        return Err(format!(
            "invalid {kind}: dots must separate non-empty name segments"
        ));
    }
    if !segment
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') || allow_dots && ch == '.')
    {
        let allowed_characters = if allow_dots {
            "ASCII letters, digits, `.`, `_`, and `-`"
        } else {
            "ASCII letters, digits, `_`, and `-`"
        };
        return Err(format!(
            "invalid {kind}: only {allowed_characters} are allowed"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "plugin_id_tests.rs"]
mod tests;
