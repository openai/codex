use super::fingerprint::record_origins;
use super::fingerprint::version_for_toml;
use super::merge::merge_toml_values;
use codex_app_server_protocol::ConfigLayer;
use codex_app_server_protocol::ConfigLayerMetadata;
use codex_app_server_protocol::ConfigLayerName;
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
    pub name: ConfigLayerName,
    pub source: PathBuf,
    pub config: TomlValue,
    pub version: String,
}

impl ConfigLayerEntry {
    pub fn new(name: ConfigLayerName, source: PathBuf, config: TomlValue) -> Self {
        let version = version_for_toml(&config);
        Self {
            name,
            source,
            config,
            version,
        }
    }

    pub fn metadata(&self) -> ConfigLayerMetadata {
        ConfigLayerMetadata {
            name: self.name.clone(),
            source: self.source.display().to_string(),
            version: self.version.clone(),
        }
    }

    pub fn as_layer(&self) -> ConfigLayer {
        ConfigLayer {
            name: self.name.clone(),
            source: self.source.display().to_string(),
            version: self.version.clone(),
            config: serde_json::to_value(&self.config).unwrap_or(JsonValue::Null),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigLayerStack {
    pub user: ConfigLayerEntry,
    pub session_flags: ConfigLayerEntry,
    pub system: Option<ConfigLayerEntry>,
    pub mdm: Option<ConfigLayerEntry>,
}

impl ConfigLayerStack {
    pub fn with_user_config(&self, user_config: TomlValue) -> Self {
        Self {
            user: ConfigLayerEntry::new(
                self.user.name.clone(),
                self.user.source.clone(),
                user_config,
            ),
            session_flags: self.session_flags.clone(),
            system: self.system.clone(),
            mdm: self.mdm.clone(),
        }
    }

    pub fn effective_config(&self) -> TomlValue {
        let mut merged = self.user.config.clone();
        merge_toml_values(&mut merged, &self.session_flags.config);
        if let Some(system) = &self.system {
            merge_toml_values(&mut merged, &system.config);
        }
        if let Some(mdm) = &self.mdm {
            merge_toml_values(&mut merged, &mdm.config);
        }
        merged
    }

    pub fn origins(&self) -> HashMap<String, ConfigLayerMetadata> {
        let mut origins = HashMap::new();
        let mut path = Vec::new();

        record_origins(
            &self.user.config,
            &self.user.metadata(),
            &mut path,
            &mut origins,
        );
        record_origins(
            &self.session_flags.config,
            &self.session_flags.metadata(),
            &mut path,
            &mut origins,
        );
        if let Some(system) = &self.system {
            record_origins(&system.config, &system.metadata(), &mut path, &mut origins);
        }
        if let Some(mdm) = &self.mdm {
            record_origins(&mdm.config, &mdm.metadata(), &mut path, &mut origins);
        }

        origins
    }

    pub fn layers_high_to_low(&self) -> Vec<ConfigLayer> {
        let mut layers = Vec::new();
        if let Some(mdm) = &self.mdm {
            layers.push(mdm.as_layer());
        }
        if let Some(system) = &self.system {
            layers.push(system.as_layer());
        }
        layers.push(self.session_flags.as_layer());
        layers.push(self.user.as_layer());
        layers
    }
}
