use crate::config::Config;
use crate::config::types::AppConfig;
use crate::config::types::AppDisabledReason;
use crate::config::types::AppToolApproval;
use crate::config::types::AppToolConfig;
use crate::config::types::AppsConfigToml;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_connection_manager::ToolInfo;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ResolvedToolApprovalMode {
    #[default]
    Auto,
    Prompt,
    Approve,
}

impl From<AppToolApproval> for ResolvedToolApprovalMode {
    fn from(value: AppToolApproval) -> Self {
        match value {
            AppToolApproval::Auto => Self::Auto,
            AppToolApproval::Prompt => Self::Prompt,
            AppToolApproval::Approve => Self::Approve,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AppToolBlockReason {
    MissingConnectorId,
    AppDisabled {
        disabled_reason: Option<AppDisabledReason>,
    },
    ToolDisabled {
        disabled_reason: Option<AppDisabledReason>,
    },
    DestructiveDisallowed,
    OpenWorldDisallowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ResolvedAppToolPolicy {
    pub(crate) approval_mode: ResolvedToolApprovalMode,
    pub(crate) block_reason: Option<AppToolBlockReason>,
}

impl ResolvedAppToolPolicy {
    pub(crate) fn is_allowed(&self) -> bool {
        self.block_reason.is_none()
    }
}

pub(crate) fn read_apps_config(config: &Config) -> Option<AppsConfigToml> {
    let effective_config = config.config_layer_stack.effective_config();
    let apps_config = effective_config.as_table()?.get("apps")?.clone();
    AppsConfigToml::deserialize(apps_config).ok()
}

pub(crate) fn resolve_app_tool_policy(
    apps_config: Option<&AppsConfigToml>,
    tool: &ToolInfo,
) -> ResolvedAppToolPolicy {
    if tool.server_name != CODEX_APPS_MCP_SERVER_NAME {
        return ResolvedAppToolPolicy::default();
    }

    let Some(connector_id) = tool.connector_id.as_deref() else {
        return ResolvedAppToolPolicy {
            approval_mode: ResolvedToolApprovalMode::Auto,
            block_reason: Some(AppToolBlockReason::MissingConnectorId),
        };
    };

    let app_config = apps_config.and_then(|apps| apps.apps.get(connector_id));
    let tool_config = tool_config(app_config, &tool.tool_name);
    let approval_mode = resolve_approval_mode(app_config, tool_config);

    if app_config.is_some_and(|app| !app.enabled) {
        return ResolvedAppToolPolicy {
            approval_mode,
            block_reason: Some(AppToolBlockReason::AppDisabled {
                disabled_reason: app_config.and_then(|app| app.disabled_reason.clone()),
            }),
        };
    }

    if tool_config.is_some_and(|tool_cfg| tool_cfg.enabled == Some(false)) {
        return ResolvedAppToolPolicy {
            approval_mode,
            block_reason: Some(AppToolBlockReason::ToolDisabled {
                disabled_reason: tool_config.and_then(|tool_cfg| tool_cfg.disabled_reason.clone()),
            }),
        };
    }

    if destructive_hint_is_blocked(apps_config, app_config, tool) {
        return ResolvedAppToolPolicy {
            approval_mode,
            block_reason: Some(AppToolBlockReason::DestructiveDisallowed),
        };
    }

    if open_world_hint_is_blocked(apps_config, app_config, tool) {
        return ResolvedAppToolPolicy {
            approval_mode,
            block_reason: Some(AppToolBlockReason::OpenWorldDisallowed),
        };
    }

    ResolvedAppToolPolicy {
        approval_mode,
        block_reason: None,
    }
}

pub(crate) fn blocked_message(
    reason: &AppToolBlockReason,
    tool_name: &str,
    connector_name: Option<&str>,
) -> String {
    let app_label = connector_name.unwrap_or("This app");
    match reason {
        AppToolBlockReason::MissingConnectorId => {
            format!("tool \"{tool_name}\" is missing connector metadata")
        }
        AppToolBlockReason::AppDisabled { disabled_reason } => match disabled_reason {
            Some(disabled_reason) => {
                format!(
                    "{app_label} is disabled ({disabled_reason}); tool \"{tool_name}\" is blocked"
                )
            }
            None => format!("{app_label} is disabled; tool \"{tool_name}\" is blocked"),
        },
        AppToolBlockReason::ToolDisabled { disabled_reason } => match disabled_reason {
            Some(disabled_reason) => {
                format!("{app_label} tool \"{tool_name}\" is disabled ({disabled_reason})")
            }
            None => format!("{app_label} tool \"{tool_name}\" is disabled"),
        },
        AppToolBlockReason::DestructiveDisallowed => {
            format!("{app_label} tool \"{tool_name}\" is blocked by disable_destructive policy")
        }
        AppToolBlockReason::OpenWorldDisallowed => {
            format!("{app_label} tool \"{tool_name}\" is blocked by disable_open_world policy")
        }
    }
}

fn resolve_approval_mode(
    app_config: Option<&AppConfig>,
    tool_config: Option<&AppToolConfig>,
) -> ResolvedToolApprovalMode {
    tool_config
        .and_then(|tool_cfg| tool_cfg.approval)
        .or_else(|| {
            app_config
                .and_then(|app| app.tools.as_ref())
                .and_then(|tools| tools.default.approval)
        })
        .unwrap_or_default()
        .into()
}

fn tool_config<'a>(
    app_config: Option<&'a AppConfig>,
    tool_name: &str,
) -> Option<&'a AppToolConfig> {
    app_config
        .and_then(|app| app.tools.as_ref())
        .and_then(|tools| tools.tools.get(tool_name))
}

fn destructive_hint_is_blocked(
    apps_config: Option<&AppsConfigToml>,
    app_config: Option<&AppConfig>,
    tool: &ToolInfo,
) -> bool {
    let disable_destructive = app_config
        .and_then(|app| app.disable_destructive)
        .unwrap_or_else(|| {
            apps_config
                .map(|apps| apps.default.disable_destructive)
                .unwrap_or(false)
        });
    disable_destructive
        && tool
            .tool
            .annotations
            .as_ref()
            .and_then(|a| a.destructive_hint)
            == Some(true)
}

fn open_world_hint_is_blocked(
    apps_config: Option<&AppsConfigToml>,
    app_config: Option<&AppConfig>,
    tool: &ToolInfo,
) -> bool {
    let disable_open_world = app_config
        .and_then(|app| app.disable_open_world)
        .unwrap_or_else(|| {
            apps_config
                .map(|apps| apps.default.disable_open_world)
                .unwrap_or(false)
        });
    disable_open_world
        && tool
            .tool
            .annotations
            .as_ref()
            .and_then(|a| a.open_world_hint)
            == Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::AppToolDefaults;
    use crate::config::types::AppToolsConfigToml;
    use pretty_assertions::assert_eq;
    use rmcp::model::JsonObject;
    use rmcp::model::Tool;
    use rmcp::model::ToolAnnotations;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn make_tool(
        server_name: &str,
        tool_name: &str,
        connector_id: Option<&str>,
        destructive_hint: Option<bool>,
        open_world_hint: Option<bool>,
    ) -> ToolInfo {
        ToolInfo {
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            tool: Tool {
                name: tool_name.to_string().into(),
                title: None,
                description: None,
                input_schema: Arc::new(JsonObject::default()),
                output_schema: None,
                annotations: Some(ToolAnnotations {
                    destructive_hint,
                    idempotent_hint: None,
                    open_world_hint,
                    read_only_hint: None,
                    title: None,
                }),
                execution: None,
                icons: None,
                meta: None,
            },
            connector_id: connector_id.map(str::to_string),
            connector_name: connector_id.map(str::to_string),
        }
    }

    #[test]
    fn non_codex_apps_tool_uses_default_policy() {
        let tool = make_tool(
            "other_server",
            "issues/create",
            Some("github"),
            Some(true),
            None,
        );
        assert_eq!(
            resolve_app_tool_policy(None, &tool),
            ResolvedAppToolPolicy::default()
        );
    }

    #[test]
    fn app_disabled_blocks_tool() {
        let tool = make_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "issues/create",
            Some("connector_123"),
            None,
            None,
        );
        let apps = AppsConfigToml {
            default: Default::default(),
            apps: HashMap::from([(
                "connector_123".to_string(),
                AppConfig {
                    enabled: false,
                    disabled_reason: Some(AppDisabledReason::AdminPolicy),
                    disable_destructive: None,
                    disable_open_world: None,
                    tools: None,
                },
            )]),
        };
        assert_eq!(
            resolve_app_tool_policy(Some(&apps), &tool),
            ResolvedAppToolPolicy {
                approval_mode: ResolvedToolApprovalMode::Auto,
                block_reason: Some(AppToolBlockReason::AppDisabled {
                    disabled_reason: Some(AppDisabledReason::AdminPolicy),
                }),
            }
        );
    }

    #[test]
    fn app_default_disables_destructive_tools() {
        let tool = make_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "issues/create",
            Some("connector_123"),
            Some(true),
            None,
        );
        let apps = AppsConfigToml {
            default: crate::config::types::AppsDefaultConfig {
                disable_destructive: true,
                disable_open_world: false,
            },
            apps: HashMap::new(),
        };
        assert_eq!(
            resolve_app_tool_policy(Some(&apps), &tool),
            ResolvedAppToolPolicy {
                approval_mode: ResolvedToolApprovalMode::Auto,
                block_reason: Some(AppToolBlockReason::DestructiveDisallowed),
            }
        );
    }

    #[test]
    fn app_override_reenables_destructive_tools() {
        let tool = make_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "issues/create",
            Some("connector_123"),
            Some(true),
            None,
        );
        let apps = AppsConfigToml {
            default: crate::config::types::AppsDefaultConfig {
                disable_destructive: true,
                disable_open_world: false,
            },
            apps: HashMap::from([(
                "connector_123".to_string(),
                AppConfig {
                    enabled: true,
                    disabled_reason: None,
                    disable_destructive: Some(false),
                    disable_open_world: None,
                    tools: None,
                },
            )]),
        };
        assert_eq!(
            resolve_app_tool_policy(Some(&apps), &tool),
            ResolvedAppToolPolicy {
                approval_mode: ResolvedToolApprovalMode::Auto,
                block_reason: None,
            }
        );
    }

    #[test]
    fn per_tool_disable_blocks_tool() {
        let tool = make_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "issues/create",
            Some("connector_123"),
            None,
            None,
        );
        let apps = AppsConfigToml {
            default: Default::default(),
            apps: HashMap::from([(
                "connector_123".to_string(),
                AppConfig {
                    enabled: true,
                    disabled_reason: None,
                    disable_destructive: None,
                    disable_open_world: None,
                    tools: Some(AppToolsConfigToml {
                        default: AppToolDefaults { approval: None },
                        tools: HashMap::from([(
                            "issues/create".to_string(),
                            AppToolConfig {
                                enabled: Some(false),
                                disabled_reason: Some(AppDisabledReason::AdminPolicy),
                                approval: None,
                            },
                        )]),
                    }),
                },
            )]),
        };
        assert_eq!(
            resolve_app_tool_policy(Some(&apps), &tool),
            ResolvedAppToolPolicy {
                approval_mode: ResolvedToolApprovalMode::Auto,
                block_reason: Some(AppToolBlockReason::ToolDisabled {
                    disabled_reason: Some(AppDisabledReason::AdminPolicy),
                }),
            }
        );
    }

    #[test]
    fn per_tool_approval_overrides_app_default() {
        let tool = make_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "issues/create",
            Some("connector_123"),
            None,
            None,
        );
        let apps = AppsConfigToml {
            default: Default::default(),
            apps: HashMap::from([(
                "connector_123".to_string(),
                AppConfig {
                    enabled: true,
                    disabled_reason: None,
                    disable_destructive: None,
                    disable_open_world: None,
                    tools: Some(AppToolsConfigToml {
                        default: AppToolDefaults {
                            approval: Some(AppToolApproval::Prompt),
                        },
                        tools: HashMap::from([(
                            "issues/create".to_string(),
                            AppToolConfig {
                                enabled: None,
                                disabled_reason: None,
                                approval: Some(AppToolApproval::Approve),
                            },
                        )]),
                    }),
                },
            )]),
        };
        assert_eq!(
            resolve_app_tool_policy(Some(&apps), &tool),
            ResolvedAppToolPolicy {
                approval_mode: ResolvedToolApprovalMode::Approve,
                block_reason: None,
            }
        );
    }
}
