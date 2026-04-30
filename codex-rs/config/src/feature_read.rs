use std::collections::HashSet;
use std::io;

use codex_features::FeatureConfigSource;
use codex_features::FeatureOverrides;
use codex_features::Features;

use crate::config_toml::ConfigToml;
use crate::profile_toml::ConfigProfile;
use crate::state::ConfigLayerStack;
use crate::state::ConfigLayerStackOrdering;
use crate::types::ToolSuggestConfig;
use crate::types::ToolSuggestDisabledTool;
use crate::types::ToolSuggestDiscoverable;

/// Read the effective feature set from a config layer stack.
///
/// This mirrors the config-file/profile feature merge used by `codex-core`.
/// It intentionally does not apply core's runtime-only managed feature
/// constraints; callers that already have a fully loaded core `Config` should
/// keep using that config as the source of truth.
pub fn features_from_config_layer_stack(
    config_layer_stack: &ConfigLayerStack,
    feature_overrides: FeatureOverrides,
) -> io::Result<Features> {
    let cfg: ConfigToml = config_layer_stack
        .effective_config()
        .try_into()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

    let default_config_profile = ConfigProfile::default();
    let config_profile = match cfg.profile.as_ref() {
        Some(key) => cfg.profiles.get(key).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("config profile `{key}` not found"),
            )
        })?,
        None => &default_config_profile,
    };

    Ok(Features::from_sources(
        FeatureConfigSource {
            features: cfg.features.as_ref(),
            include_apply_patch_tool: None,
            experimental_use_freeform_apply_patch: cfg.experimental_use_freeform_apply_patch,
            experimental_use_unified_exec_tool: cfg.experimental_use_unified_exec_tool,
        },
        FeatureConfigSource {
            features: config_profile.features.as_ref(),
            include_apply_patch_tool: config_profile.include_apply_patch_tool,
            experimental_use_freeform_apply_patch: config_profile
                .experimental_use_freeform_apply_patch,
            experimental_use_unified_exec_tool: config_profile.experimental_use_unified_exec_tool,
        },
        feature_overrides,
    ))
}

pub fn resolve_tool_suggest_config_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
) -> ToolSuggestConfig {
    let tool_suggest = config_layer_stack
        .effective_config()
        .get("tool_suggest")
        .cloned()
        .and_then(|value| value.try_into::<ToolSuggestConfig>().ok());
    resolve_tool_suggest_config_from_config(tool_suggest.as_ref(), config_layer_stack)
}

fn resolve_tool_suggest_config_from_config(
    tool_suggest: Option<&ToolSuggestConfig>,
    config_layer_stack: &ConfigLayerStack,
) -> ToolSuggestConfig {
    let discoverables = tool_suggest
        .into_iter()
        .flat_map(|tool_suggest| tool_suggest.discoverables.iter())
        .filter_map(|discoverable| {
            let trimmed = discoverable.id.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(ToolSuggestDiscoverable {
                    kind: discoverable.kind,
                    id: trimmed.to_string(),
                })
            }
        })
        .collect();
    let mut seen_disabled_tools = HashSet::new();
    let mut disabled_tools = Vec::new();
    let mut add_disabled_tool = |disabled_tool: ToolSuggestDisabledTool| {
        if let Some(disabled_tool) = disabled_tool.normalized()
            && seen_disabled_tools.insert(disabled_tool.clone())
        {
            disabled_tools.push(disabled_tool);
        }
    };

    let layers = config_layer_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ false,
    );
    if layers.is_empty() {
        for disabled_tool in tool_suggest
            .into_iter()
            .flat_map(|tool_suggest| tool_suggest.disabled_tools.iter().cloned())
        {
            add_disabled_tool(disabled_tool);
        }
    } else {
        for layer in layers {
            let Some(tool_suggest) = layer
                .config
                .get("tool_suggest")
                .cloned()
                .and_then(|value| value.try_into::<ToolSuggestConfig>().ok())
            else {
                continue;
            };
            for disabled_tool in tool_suggest.disabled_tools {
                add_disabled_tool(disabled_tool);
            }
        }
    }

    ToolSuggestConfig {
        discoverables,
        disabled_tools,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigLayerEntry;
    use crate::ConfigRequirements;
    use crate::ConfigRequirementsToml;
    use codex_app_server_protocol::ConfigLayerSource;
    use codex_features::Feature;
    use pretty_assertions::assert_eq;

    fn stack_from_toml(toml: &str) -> ConfigLayerStack {
        ConfigLayerStack::new(
            vec![ConfigLayerEntry::new(
                ConfigLayerSource::SessionFlags,
                toml::from_str(toml).expect("test TOML should parse"),
            )],
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )
        .expect("config layer stack should be valid")
    }

    #[test]
    fn features_from_config_layer_stack_reads_profile_features() -> io::Result<()> {
        let stack = stack_from_toml(
            r#"
profile = "work"

[features]
plugins = false

[profiles.work.features]
plugins = true
"#,
        );

        let features = features_from_config_layer_stack(&stack, FeatureOverrides::default())?;

        assert!(features.enabled(Feature::Plugins));
        Ok(())
    }

    #[test]
    fn resolve_tool_suggest_config_merges_disabled_tools_across_layers() {
        let user = ConfigLayerEntry::new(
            ConfigLayerSource::SessionFlags,
            toml::from_str(
                r#"
[tool_suggest]
disabled_tools = [
  { type = "connector", id = " user_connector " },
  { type = "plugin", id = "shared_plugin" },
]
"#,
            )
            .expect("user test TOML should parse"),
        );
        let project = ConfigLayerEntry::new(
            ConfigLayerSource::SessionFlags,
            toml::from_str(
                r#"
[tool_suggest]
disabled_tools = [
  { type = "plugin", id = "shared_plugin" },
  { type = "plugin", id = "project_plugin" },
]
"#,
            )
            .expect("project test TOML should parse"),
        );
        let stack = ConfigLayerStack::new(
            vec![user, project],
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )
        .expect("config layer stack should be valid");

        let config = resolve_tool_suggest_config_from_layer_stack(&stack);

        assert_eq!(
            config.disabled_tools,
            vec![
                ToolSuggestDisabledTool::connector("user_connector"),
                ToolSuggestDisabledTool::plugin("shared_plugin"),
                ToolSuggestDisabledTool::plugin("project_plugin"),
            ]
        );
    }
}
