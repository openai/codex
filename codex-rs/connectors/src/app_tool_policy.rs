use codex_config::AppsRequirementsToml;
use codex_config::ConfigLayerStack;
use codex_config::types::AppToolApproval;
use codex_config::types::AppsConfigToml;
use serde::Deserialize;

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

/// Connector-owned metadata used to evaluate one app tool.
#[derive(Debug, Clone, Copy)]
pub struct AppToolPolicyInput<'a> {
    pub connector_id: Option<&'a str>,
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
pub fn apps_config_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
) -> Option<AppsConfigToml> {
    config_layer_stack
        .effective_config()
        .as_table()
        .and_then(|table| table.get("apps"))
        .cloned()
        .and_then(|value| AppsConfigToml::deserialize(value).ok())
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
        .or_else(|| app.and_then(|app| app.default_tools_approval_mode))
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
