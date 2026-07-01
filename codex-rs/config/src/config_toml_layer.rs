use crate::config_toml::ConfigToml;
use crate::mcp_types::RawMcpServerConfig;
use crate::strict_config::ignored_toml_value_field;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use serde::Deserialize;
use serde::de::Error as _;
use std::collections::HashMap;
use std::path::Path;
use toml::Value as TomlValue;

/// A parsed config layer that can contain incomplete composable sections.
pub struct ParsedConfigTomlLayer {
    raw: TomlValue,
    self_contained_config: ConfigToml,
}

impl ParsedConfigTomlLayer {
    /// Parses and validates a layer without requiring composable sections to be complete.
    pub fn parse(raw: TomlValue, base_dir: &Path) -> Result<Self, toml::de::Error> {
        let parts = split_config_toml_layer(raw.clone());
        let _guard = AbsolutePathBufGuard::new(base_dir);
        if let Some(ignored_field) =
            ignored_toml_value_field::<ConfigTomlComposableSections>(parts.composable.clone())
        {
            return Err(toml::de::Error::custom(format!(
                "unknown configuration field `{ignored_field}`"
            )));
        }
        let _: ConfigTomlComposableSections = parts.composable.try_into()?;
        let self_contained_config = parts.self_contained.try_into()?;

        Ok(Self {
            raw,
            self_contained_config,
        })
    }

    /// Returns the config with composable sections omitted.
    pub fn self_contained_config(&self) -> &ConfigToml {
        &self.self_contained_config
    }

    pub fn into_raw(self) -> TomlValue {
        self.raw
    }
}

#[derive(Deserialize)]
pub(crate) struct ConfigTomlComposableSections {
    #[serde(default, rename = "mcp_servers")]
    _mcp_servers: HashMap<String, RawMcpServerConfig>,
}

pub(crate) struct ConfigTomlLayerParts {
    pub(crate) self_contained: TomlValue,
    pub(crate) composable: TomlValue,
}

pub(crate) fn split_config_toml_layer(mut value: TomlValue) -> ConfigTomlLayerParts {
    let mut composable = toml::map::Map::new();
    if let Some(table) = value.as_table_mut() {
        let key = "mcp_servers";
        if let Some(section) = table.remove(key) {
            composable.insert(key.to_string(), section);
        }
    }

    ConfigTomlLayerParts {
        self_contained: value,
        composable: TomlValue::Table(composable),
    }
}
