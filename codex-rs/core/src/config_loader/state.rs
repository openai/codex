use crate::config_loader::ConfigRequirements;

use super::fingerprint::record_origins;
use super::fingerprint::version_for_toml;
use super::merge::merge_toml_values;
use codex_app_server_protocol::ConfigLayer;
use codex_app_server_protocol::ConfigLayerMetadata;
use codex_app_server_protocol::ConfigLayerSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;
use toml::Value as TomlValue;

#[derive(Debug, Default, Clone)]
pub struct LoaderOverrides {
    pub managed_config_path: Option<PathBuf>,
    #[cfg(target_os = "macos")]
    pub managed_preferences_base64: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigLayerEntry {
    pub name: ConfigLayerSource,
    pub config: TomlValue,
    pub version: String,
}

impl ConfigLayerEntry {
    pub fn new(name: ConfigLayerSource, config: TomlValue) -> Self {
        let version = version_for_toml(&config);
        Self {
            name,
            config,
            version,
        }
    }

    pub fn metadata(&self) -> ConfigLayerMetadata {
        ConfigLayerMetadata {
            name: self.name.clone(),
            version: self.version.clone(),
        }
    }

    pub fn as_layer(&self) -> ConfigLayer {
        ConfigLayer {
            name: self.name.clone(),
            version: self.version.clone(),
            config: serde_json::to_value(&self.config).unwrap_or(JsonValue::Null),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigLayerStack {
    /// Layers are listed from lowest precedence (base) to highest (top), so
    /// later entries in the Vec override earlier ones.
    layers: Vec<ConfigLayerEntry>,

    /// Index into [layers] of the user config layer, if any.
    user_layer_index: Option<usize>,

    /// Constraints that must be enforced when deriving a [Config] from the
    /// layers.
    requirements: ConfigRequirements,
}

impl ConfigLayerStack {
    pub fn new(
        layers: Vec<ConfigLayerEntry>,
        requirements: ConfigRequirements,
    ) -> std::io::Result<Self> {
        let user_layer_index = verify_layer_ordering(&layers)?;
        Ok(Self {
            layers,
            user_layer_index,
            requirements,
        })
    }

    /// Returns the user config layer, if any.
    pub fn get_user_layer(&self) -> Option<&ConfigLayerEntry> {
        self.user_layer_index
            .and_then(|index| self.layers.get(index))
    }

    pub fn requirements(&self) -> &ConfigRequirements {
        &self.requirements
    }

    /// Creates a new [ConfigLayerStack] using the specified values to inject a
    /// "user layer" into the stack. If such a layer already exists, it is
    /// replaced; otherwise, it is inserted into the stack at the appropriate
    /// position based on precedence rules.
    pub fn with_user_config(&self, config_toml: &AbsolutePathBuf, user_config: TomlValue) -> Self {
        let user_layer = ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: config_toml.clone(),
            },
            user_config,
        );

        let mut layers = self.layers.clone();
        match self.user_layer_index {
            Some(index) => {
                layers[index] = user_layer;
                Self {
                    layers,
                    user_layer_index: self.user_layer_index,
                    requirements: self.requirements.clone(),
                }
            }
            None => {
                let user_layer_index = match layers
                    .iter()
                    .position(|layer| layer.name.precedence() > user_layer.name.precedence())
                {
                    Some(index) => {
                        layers.insert(index, user_layer);
                        index
                    }
                    None => {
                        layers.push(user_layer);
                        layers.len() - 1
                    }
                };
                Self {
                    layers,
                    user_layer_index: Some(user_layer_index),
                    requirements: self.requirements.clone(),
                }
            }
        }
    }

    pub fn effective_config(&self) -> TomlValue {
        let mut merged = TomlValue::Table(toml::map::Map::new());
        for layer in &self.layers {
            merge_toml_values(&mut merged, &layer.config);
        }
        merged
    }

    pub fn origins(&self) -> HashMap<String, ConfigLayerMetadata> {
        let mut origins = HashMap::new();
        let mut path = Vec::new();

        for layer in &self.layers {
            record_origins(&layer.config, &layer.metadata(), &mut path, &mut origins);
        }

        origins
    }

    /// Returns the highest-precedence to lowest-precedence layers, so
    /// `ConfigLayerSource::SessionFlags` would be first, if present.
    pub fn layers_high_to_low(&self) -> Vec<&ConfigLayerEntry> {
        self.layers.iter().rev().collect()
    }
}

/// Ensures precedence ordering of config layers is correct. Returns the index
/// of the user config layer, if any (at most one should exist).
fn verify_layer_ordering(layers: &[ConfigLayerEntry]) -> std::io::Result<Option<usize>> {
    if !layers.iter().map(|layer| &layer.name).is_sorted() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "config layers are not in correct precedence order",
        ));
    }

    let mut user_layer_index: Option<usize> = None;
    for (index, layer) in layers.iter().enumerate() {
        if matches!(layer.name, ConfigLayerSource::User { .. }) {
            if user_layer_index.is_some() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "multiple user config layers found",
                ));
            }
            user_layer_index = Some(index);
        }
    }

    Ok(user_layer_index)
}
