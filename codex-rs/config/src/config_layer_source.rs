use crate::types::StatusLineSelection;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde_json::Value as JsonValue;
use std::borrow::Cow;
use toml::Value as TomlValue;

/// Provenance for one layer in the effective Codex configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLayerSource {
    /// Default configuration supplied with the installed Codex package.
    PackagedDefaults { file: AbsolutePathBuf },
    /// Managed preferences delivered by MDM.
    Mdm { domain: String, key: String },
    /// Host-wide configuration loaded from a file.
    System { file: AbsolutePathBuf },
    /// Configuration delivered by an enterprise cloud bundle.
    EnterpriseManaged { id: String, name: String },
    /// User configuration, optionally augmented by a selected profile.
    User {
        file: AbsolutePathBuf,
        profile: Option<String>,
    },
    /// Configuration loaded from a project's `.codex` directory.
    Project { dot_codex_folder: AbsolutePathBuf },
    /// Overrides supplied for the current session.
    SessionFlags,
    /// Legacy managed configuration loaded from a file.
    LegacyManagedConfigTomlFromFile { file: AbsolutePathBuf },
    /// Legacy managed configuration delivered by MDM.
    LegacyManagedConfigTomlFromMdm,
}

pub(crate) fn without_status_line_provider(config: &TomlValue) -> Cow<'_, TomlValue> {
    let mut values = Cow::Borrowed(config);
    if config
        .get("tui")
        .and_then(|tui| tui.get("status_line_provider"))
        .is_some()
        && let Some(table) = values.to_mut().as_table_mut()
        && let Some(tui) = table.get_mut("tui").and_then(TomlValue::as_table_mut)
    {
        tui.remove("status_line_provider");
        if tui.is_empty() {
            table.remove("tui");
        }
    }
    values
}

impl ConfigLayerSource {
    pub(crate) fn allows_status_line_provider(&self) -> bool {
        match self {
            Self::PackagedDefaults { .. } | Self::Project { .. } => false,
            Self::User { .. }
            | Self::SessionFlags
            | Self::System { .. }
            | Self::EnterpriseManaged { .. }
            | Self::Mdm { .. }
            | Self::LegacyManagedConfigTomlFromFile { .. }
            | Self::LegacyManagedConfigTomlFromMdm => true,
        }
    }

    /// Keep the original layer for diagnostics and config/read. An invalid selection
    /// still overrides lower providers, just like an explicit built-in selection.
    pub(crate) fn effective_values<'a>(&self, config: &'a TomlValue) -> Cow<'a, TomlValue> {
        if !self.allows_status_line_provider() {
            return without_status_line_provider(config);
        }
        if status_line_provider_is_valid(config) == Some(false) {
            let mut effective = config.clone();
            effective["tui"]["status_line_provider"] = "default".into();
            return Cow::Owned(effective);
        }
        Cow::Borrowed(config)
    }

    /// A setting from a layer with a higher precedence overrides a setting
    /// from a layer with a lower precedence.
    pub fn precedence(&self) -> i16 {
        match self {
            ConfigLayerSource::PackagedDefaults { .. } => -10,
            ConfigLayerSource::Mdm { .. } => 0,
            ConfigLayerSource::System { .. } => 10,
            ConfigLayerSource::EnterpriseManaged { .. } => 15,
            ConfigLayerSource::User { profile, .. } => {
                if profile.is_some() {
                    21
                } else {
                    20
                }
            }
            ConfigLayerSource::Project { .. } => 25,
            ConfigLayerSource::SessionFlags => 30,
            ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. } => 40,
            ConfigLayerSource::LegacyManagedConfigTomlFromMdm => 50,
        }
    }
}

pub(crate) fn status_line_provider_is_valid(config: &TomlValue) -> Option<bool> {
    let value = config.get("tui")?.get("status_line_provider")?;
    Some(value.clone().try_into::<StatusLineSelection>().is_ok())
}

/// Compares [`ConfigLayerSource`] by precedence, so `A < B` means settings
/// from layer `A` will be overridden by settings from layer `B`.
impl PartialOrd for ConfigLayerSource {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.precedence().cmp(&other.precedence()))
    }
}

/// Identity and version information for a configuration layer.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigLayerMetadata {
    pub name: ConfigLayerSource,
    pub version: String,
}

/// A materialized configuration layer and its provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigLayer {
    pub name: ConfigLayerSource,
    pub version: String,
    pub config: JsonValue,
    pub disabled_reason: Option<String>,
}

pub fn format_config_layer_source(source: &ConfigLayerSource, config_toml_file: &str) -> String {
    match source {
        ConfigLayerSource::PackagedDefaults { file } => {
            format!("packaged defaults ({})", file.as_path().display())
        }
        ConfigLayerSource::Mdm { domain, key } => {
            format!("MDM ({domain}:{key})")
        }
        ConfigLayerSource::System { file } => {
            format!("system ({})", file.as_path().display())
        }
        ConfigLayerSource::EnterpriseManaged { id, name } => {
            format!("enterprise-managed ({name}, {id})")
        }
        ConfigLayerSource::User { file, .. } => {
            format!("user ({})", file.as_path().display())
        }
        ConfigLayerSource::Project { dot_codex_folder } => {
            format!(
                "project ({}/{config_toml_file})",
                dot_codex_folder.as_path().display()
            )
        }
        ConfigLayerSource::SessionFlags => "session-flags".to_string(),
        ConfigLayerSource::LegacyManagedConfigTomlFromFile { file } => {
            format!("legacy managed_config.toml ({})", file.as_path().display())
        }
        ConfigLayerSource::LegacyManagedConfigTomlFromMdm => {
            "legacy managed_config.toml (MDM)".to_string()
        }
    }
}
