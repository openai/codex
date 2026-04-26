use std::collections::HashMap;
use std::io;

use codex_app_server_protocol::ConfigLayerSource;
use codex_config::Constrained;
use codex_config::config_toml::ConfigToml;
use codex_features::Feature;
use codex_model_provider::ProviderCapabilities;
use codex_model_provider::create_model_provider;
use codex_protocol::config_types::WebSearchMode;
use toml::Value as TomlValue;

use crate::config_loader::ConfigLayerEntry;
use crate::config_loader::ConfigLayerStack;

use super::Config;
use super::resolve_model_provider_from_config_toml;

pub(crate) fn add_provider_capabilities_layer(
    layers: ConfigLayerStack,
) -> io::Result<ConfigLayerStack> {
    let cfg: ConfigToml = match layers.effective_config().try_into() {
        Ok(cfg) => cfg,
        Err(_) => return Ok(layers),
    };

    let config_profile = cfg
        .profile
        .as_ref()
        .and_then(|key| cfg.profiles.get(key))
        .cloned()
        .unwrap_or_default();
    let resolved_model_provider = match resolve_model_provider_from_config_toml(
        &cfg,
        &config_profile,
        /*explicit_model_provider*/ None,
    ) {
        Ok(resolved_model_provider) => resolved_model_provider,
        Err(_) => return Ok(layers),
    };

    let provider = create_model_provider(resolved_model_provider.info, /*auth_manager*/ None);
    let Some(config) = provider_capabilities_layer_config(provider.capabilities()) else {
        return Ok(layers);
    };

    layers.with_layer(ConfigLayerEntry::new(
        ConfigLayerSource::ProviderCapabilities {
            provider: resolved_model_provider.id,
        },
        config,
    ))
}

/// Applies runtime safety constraints for provider-disabled capabilities.
///
/// The synthetic provider layer controls initial config visibility and origins.
/// These constraints keep mutable runtime state from re-enabling unsupported
/// features after the final [`Config`] has been built.
pub(crate) fn apply_provider_capability_runtime_constraints(
    config: &mut Config,
    capabilities: ProviderCapabilities,
) -> std::io::Result<()> {
    if !capabilities.apps {
        config.features.pin_disabled(Feature::Apps)?;
    }
    if !capabilities.plugins {
        config.features.pin_disabled(Feature::Plugins)?;
    }
    if !capabilities.tool_search {
        config.features.pin_disabled(Feature::ToolSearch)?;
    }
    if !capabilities.tool_suggest {
        config.features.pin_disabled(Feature::ToolSuggest)?;
    }
    if !capabilities.image_generation {
        config.features.pin_disabled(Feature::ImageGeneration)?;
    }
    if !capabilities.web_search {
        config.web_search_mode = Constrained::allow_only(WebSearchMode::Disabled);
    }
    if !capabilities.mcp_servers {
        config.mcp_servers = Constrained::allow_only(HashMap::new());
    }
    if !capabilities.apps_instructions {
        config.include_apps_instructions = false;
    }

    Ok(())
}

/// Builds the synthetic config layer for provider-disabled capabilities.
///
/// Supported capabilities emit no config, so this layer can only disable
/// features or clear provider-unsupported config surfaces.
pub(crate) fn provider_capabilities_layer_config(
    capabilities: ProviderCapabilities,
) -> Option<TomlValue> {
    let mut table = toml::map::Map::new();

    if !capabilities.apps {
        set_feature_disabled(&mut table, "apps");
    }
    if !capabilities.plugins {
        set_feature_disabled(&mut table, "plugins");
    }
    if !capabilities.tool_search {
        set_feature_disabled(&mut table, "tool_search");
    }
    if !capabilities.tool_suggest {
        set_feature_disabled(&mut table, "tool_suggest");
    }
    if !capabilities.image_generation {
        set_feature_disabled(&mut table, "image_generation");
    }
    if !capabilities.web_search {
        table.insert(
            "web_search".to_string(),
            TomlValue::String("disabled".to_string()),
        );
    }
    if !capabilities.mcp_servers {
        table.insert(
            "mcp_servers".to_string(),
            TomlValue::Table(toml::map::Map::new()),
        );
    }
    if !capabilities.apps_instructions {
        table.insert(
            "include_apps_instructions".to_string(),
            TomlValue::Boolean(false),
        );
    }

    (!table.is_empty()).then_some(TomlValue::Table(table))
}

fn set_feature_disabled(table: &mut toml::map::Map<String, TomlValue>, feature: &str) {
    let features = table
        .entry("features".to_string())
        .or_insert_with(|| TomlValue::Table(toml::map::Map::new()));
    if !features.is_table() {
        *features = TomlValue::Table(toml::map::Map::new());
    }
    if let Some(features) = features.as_table_mut() {
        features.insert(feature.to_string(), TomlValue::Boolean(false));
    }
}
