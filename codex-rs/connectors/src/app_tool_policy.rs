use codex_config::AppsRequirementsToml;
use codex_config::ConfigLayerMetadata;
use codex_config::ConfigLayerStack;
use codex_config::TomlValue;
use codex_config::merge_toml_values;
use codex_config::types::AppToolApproval;
use codex_config::types::AppsConfigToml;
use codex_config::types::PluginsConfigToml;
use serde::Deserialize;
use std::collections::HashMap;

use crate::AppInfo;

/// The effective enablement and approval policy for one app tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppToolPolicy {
    pub enabled: bool,
    pub approval: AppToolApproval,
}

impl Default for AppToolPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            approval: AppToolApproval::Auto,
        }
    }
}

/// App and account metadata used to evaluate one tool.
#[derive(Debug, Clone, Copy)]
pub struct AppToolPolicyInput<'a> {
    pub connector_id: Option<&'a str>,
    pub link_id: Option<&'a str>,
    pub tool_name: &'a str,
    pub tool_title: Option<&'a str>,
    pub destructive_hint: Option<bool>,
    pub open_world_hint: Option<bool>,
}

/// Resolves app tool policy against one immutable config snapshot.
///
/// Callers should construct one evaluator and reuse it for every tool in the
/// same exposure build so config layers are merged and decoded only once.
pub struct AppToolPolicyEvaluator<'a> {
    apps_config: Option<AppsConfigToml>,
    requirements_apps_config: Option<&'a AppsRequirementsToml>,
}

impl<'a> AppToolPolicyEvaluator<'a> {
    pub fn new(config_layer_stack: &'a ConfigLayerStack) -> Self {
        let apps_config = apps_config_from_layer_stack(config_layer_stack);
        let requirements_apps_config = config_layer_stack.requirements_toml().apps.as_ref();
        Self::from_parts(apps_config, requirements_apps_config)
    }

    pub fn policy(&self, input: AppToolPolicyInput<'_>) -> AppToolPolicy {
        let managed_approval = managed_app_tool_approval(
            self.requirements_apps_config,
            input.connector_id,
            input.tool_name,
        );
        app_tool_policy_from_apps_config(self.apps_config.as_ref(), input, managed_approval)
    }

    /// Returns the effective local and managed enablement for one connector.
    pub fn app_enabled(&self, connector_id: &str) -> bool {
        self.apps_config
            .as_ref()
            .map(|apps_config| app_is_enabled(apps_config, Some(connector_id)))
            .unwrap_or(true)
    }

    /// Applies app policy without overriding source state for unconfigured apps.
    pub fn apply_app_enabled_state(&self, mut apps: Vec<AppInfo>) -> Vec<AppInfo> {
        let Some(apps_config) = self.apps_config.as_ref() else {
            return apps;
        };

        for app in &mut apps {
            if apps_config.default.is_some() || apps_config.apps.contains_key(app.id.as_str()) {
                app.is_enabled = self.app_enabled(app.id.as_str());
            }
        }

        apps
    }

    fn from_parts(
        apps_config: Option<AppsConfigToml>,
        requirements_apps_config: Option<&'a AppsRequirementsToml>,
    ) -> Self {
        Self {
            apps_config: effective_apps_config(apps_config, requirements_apps_config),
            requirements_apps_config,
        }
    }
}

/// Reads the merged, unmanaged Apps configuration from a config-layer stack.
///
/// App settings from plugins enabled by configuration apply connector-wide.
/// Tables use the standard config merge, with earlier plugin names taking
/// precedence and explicit top-level app fields applied last.
pub fn apps_config_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
) -> Option<AppsConfigToml> {
    let overlays = apps_config_overlays(&config_layer_stack.effective_config());
    if overlays.is_empty() {
        return None;
    }
    let mut apps = TomlValue::Table(Default::default());
    for (_, overlay) in overlays {
        // Merge before decoding so omitted fields do not acquire serde defaults.
        merge_toml_values(&mut apps, &overlay);
    }
    AppsConfigToml::deserialize(apps).ok()
}

/// Projects field origins onto the effective `apps.*` paths using Apps precedence.
pub fn apps_config_origins_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
) -> HashMap<String, ConfigLayerMetadata> {
    let source_origins = config_layer_stack.origins();
    let mut origins = HashMap::new();
    for (prefix, _) in apps_config_overlays(&config_layer_stack.effective_config()) {
        let prefix = format!("{prefix}.");
        for (path, origin) in &source_origins {
            if let Some(field) = path.strip_prefix(&prefix)
                && (prefix == "apps." || !field.starts_with("_default."))
            {
                origins.insert(format!("apps.{field}"), origin.clone());
            }
        }
    }
    origins
}

fn apps_config_overlays(config: &TomlValue) -> Vec<(String, TomlValue)> {
    let mut overlays = Vec::new();
    if let Some(plugins) = config.get("plugins")
        && let Ok(policy) = PluginsConfigToml::deserialize(plugins.clone())
        && let Some(plugins) = plugins.as_table()
    {
        let mut plugins: Vec<_> = plugins.iter().collect();
        plugins.sort_unstable_by_key(|(name, _)| *name);
        for (name, plugin) in plugins.into_iter().rev() {
            let Some(plugin_config) = policy.plugins.get(name) else {
                continue;
            };
            if !plugin_config.enabled.unwrap_or(true) || !policy.allows_plugin(name) {
                continue;
            }
            if let Some(plugin_apps) = plugin.get("apps").and_then(TomlValue::as_table) {
                let mut plugin_apps = plugin_apps.clone();
                // Nested tables address connector IDs, not global Apps defaults.
                plugin_apps.remove("_default");
                if !plugin_apps.is_empty() {
                    overlays.push((
                        format!("plugins.{name}.apps"),
                        TomlValue::Table(plugin_apps),
                    ));
                }
            }
        }
    }

    if let Some(top_level_apps) = config.get("apps") {
        overlays.push(("apps".to_string(), top_level_apps.clone()));
    }
    overlays
}

pub fn app_is_enabled(apps_config: &AppsConfigToml, connector_id: Option<&str>) -> bool {
    let default_enabled = apps_config
        .default
        .as_ref()
        .map(|defaults| defaults.enabled)
        .unwrap_or(true);

    connector_id
        .and_then(|connector_id| apps_config.apps.get(connector_id))
        .map(|app| app.enabled)
        .unwrap_or(default_enabled)
}

fn effective_apps_config(
    apps_config: Option<AppsConfigToml>,
    requirements_apps_config: Option<&AppsRequirementsToml>,
) -> Option<AppsConfigToml> {
    let had_apps_config = apps_config.is_some();
    let mut apps_config = apps_config.unwrap_or_default();
    apply_requirements_apps_constraints(&mut apps_config, requirements_apps_config);
    if had_apps_config || apps_config.default.is_some() || !apps_config.apps.is_empty() {
        Some(apps_config)
    } else {
        None
    }
}

fn apply_requirements_apps_constraints(
    apps_config: &mut AppsConfigToml,
    requirements_apps_config: Option<&AppsRequirementsToml>,
) {
    let Some(requirements_apps_config) = requirements_apps_config else {
        return;
    };

    for (app_id, requirement) in &requirements_apps_config.apps {
        if requirement.enabled == Some(false) {
            let app = apps_config.apps.entry(app_id.clone()).or_default();
            app.enabled = false;
        }
    }
}

fn managed_app_tool_approval(
    requirements_apps_config: Option<&AppsRequirementsToml>,
    connector_id: Option<&str>,
    tool_name: &str,
) -> Option<AppToolApproval> {
    let connector_id = connector_id?;
    requirements_apps_config?
        .apps
        .get(connector_id)?
        .tools
        .as_ref()?
        .tools
        .get(tool_name)?
        .approval_mode
}

fn app_tool_policy_from_apps_config(
    apps_config: Option<&AppsConfigToml>,
    input: AppToolPolicyInput<'_>,
    managed_approval: Option<AppToolApproval>,
) -> AppToolPolicy {
    let Some(apps_config) = apps_config else {
        return AppToolPolicy {
            approval: managed_approval.unwrap_or(AppToolApproval::Auto),
            ..Default::default()
        };
    };

    let app = input
        .connector_id
        .and_then(|connector_id| apps_config.apps.get(connector_id));
    let tools = app.and_then(|app| app.tools.as_ref());
    let tool_config = tools.and_then(|tools| {
        tools
            .tools
            .get(input.tool_name)
            .or_else(|| input.tool_title.and_then(|title| tools.tools.get(title)))
    });
    let approval = managed_approval
        .or_else(|| tool_config.and_then(|tool| tool.approval_mode))
        .or_else(|| {
            input
                .link_id
                .and_then(|link_id| app?.links.as_ref()?.links.get(link_id))
                .and_then(|link| link.default_tools_approval_mode)
        })
        .or_else(|| app.and_then(|app| app.default_tools_approval_mode))
        .or_else(|| {
            input
                .connector_id
                .and(apps_config.default.as_ref())
                .and_then(|defaults| defaults.default_tools_approval_mode)
        })
        .unwrap_or(AppToolApproval::Auto);

    if !app_is_enabled(apps_config, input.connector_id) {
        return AppToolPolicy {
            enabled: false,
            approval,
        };
    }

    if let Some(enabled) = tool_config.and_then(|tool| tool.enabled) {
        return AppToolPolicy { enabled, approval };
    }

    if let Some(enabled) = app.and_then(|app| app.default_tools_enabled) {
        return AppToolPolicy { enabled, approval };
    }

    let app_defaults = apps_config.default.as_ref();
    let destructive_enabled = app
        .and_then(|app| app.destructive_enabled)
        .unwrap_or_else(|| {
            app_defaults
                .map(|defaults| defaults.destructive_enabled)
                .unwrap_or(true)
        });
    let open_world_enabled = app
        .and_then(|app| app.open_world_enabled)
        .unwrap_or_else(|| {
            app_defaults
                .map(|defaults| defaults.open_world_enabled)
                .unwrap_or(true)
        });
    let destructive_hint = input.destructive_hint.unwrap_or(true);
    let open_world_hint = input.open_world_hint.unwrap_or(true);
    let enabled =
        (destructive_enabled || !destructive_hint) && (open_world_enabled || !open_world_hint);

    AppToolPolicy { enabled, approval }
}

#[cfg(test)]
#[path = "app_tool_policy_tests.rs"]
mod tests;
