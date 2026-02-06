use crate::history_cell::PlainHistoryCell;
use codex_app_server_protocol::ConfigLayerSource;
use codex_core::config::Config;
use codex_core::config_loader::ConfigLayerStack;
use codex_core::config_loader::ConfigLayerStackOrdering;
use codex_core::config_loader::RequirementSource;
use codex_core::config_loader::ResidencyRequirement;
use codex_core::config_loader::SandboxModeRequirement;
use ratatui::style::Stylize;
use ratatui::text::Line;

pub(crate) fn new_debug_config_output(config: &Config) -> PlainHistoryCell {
    PlainHistoryCell::new(render_debug_config_lines(&config.config_layer_stack))
}

fn render_debug_config_lines(stack: &ConfigLayerStack) -> Vec<Line<'static>> {
    let mut lines = vec!["/debug-config".magenta().into(), "".into()];

    lines.push(
        "Config layer stack (lowest precedence first):"
            .bold()
            .into(),
    );
    let layers = stack.get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, true);
    if layers.is_empty() {
        lines.push("  <none>".dim().into());
    } else {
        for (index, layer) in layers.iter().enumerate() {
            let source = format_config_layer_source(&layer.name);
            let status = if layer.is_disabled() {
                "disabled"
            } else {
                "enabled"
            };
            lines.push(format!("  {}. {source} ({status})", index + 1).into());
            if let Some(reason) = &layer.disabled_reason {
                lines.push(format!("     reason: {reason}").dim().into());
            }
        }
    }

    let requirements = stack.requirements();
    let requirements_toml = stack.requirements_toml();

    lines.push("".into());
    lines.push("Requirements:".bold().into());
    let mut requirement_lines = Vec::new();

    if let Some(policies) = requirements_toml.allowed_approval_policies.as_ref() {
        let value = join_or_empty(policies.iter().map(ToString::to_string).collect::<Vec<_>>());
        requirement_lines.push(requirement_line(
            "allowed_approval_policies",
            value,
            requirements.approval_policy.source.as_ref(),
        ));
    }

    if let Some(modes) = requirements_toml.allowed_sandbox_modes.as_ref() {
        let value = join_or_empty(
            modes
                .iter()
                .copied()
                .map(format_sandbox_mode_requirement)
                .collect::<Vec<_>>(),
        );
        requirement_lines.push(requirement_line(
            "allowed_sandbox_modes",
            value,
            requirements.sandbox_policy.source.as_ref(),
        ));
    }

    if let Some(servers) = requirements_toml.mcp_servers.as_ref() {
        let value = join_or_empty(servers.keys().cloned().collect::<Vec<_>>());
        requirement_lines.push(requirement_line(
            "mcp_servers",
            value,
            requirements
                .mcp_servers
                .as_ref()
                .map(|sourced| &sourced.source),
        ));
    }

    // TODO(gt): Expand this debug output with detailed skills and rules display.
    if requirements_toml.rules.is_some() {
        requirement_lines.push(requirement_line(
            "rules",
            "configured".to_string(),
            requirements.exec_policy_source(),
        ));
    }

    if let Some(residency) = requirements_toml.enforce_residency {
        requirement_lines.push(requirement_line(
            "enforce_residency",
            format_residency_requirement(residency),
            requirements.enforce_residency.source.as_ref(),
        ));
    }

    if requirement_lines.is_empty() {
        lines.push("  <none>".dim().into());
    } else {
        lines.extend(requirement_lines);
    }

    lines
}

fn requirement_line(
    name: &str,
    value: String,
    source: Option<&RequirementSource>,
) -> Line<'static> {
    let source = source
        .map(ToString::to_string)
        .unwrap_or_else(|| "<unspecified>".to_string());
    format!("  - {name}: {value} (source: {source})").into()
}

fn join_or_empty(values: Vec<String>) -> String {
    if values.is_empty() {
        "<empty>".to_string()
    } else {
        values.join(", ")
    }
}

fn format_config_layer_source(source: &ConfigLayerSource) -> String {
    match source {
        ConfigLayerSource::Mdm { domain, key } => {
            format!("mdm ({domain}:{key})")
        }
        ConfigLayerSource::System { file } => {
            format!("system ({})", file.as_path().display())
        }
        ConfigLayerSource::User { file } => {
            format!("user ({})", file.as_path().display())
        }
        ConfigLayerSource::Project { dot_codex_folder } => {
            format!(
                "project ({}/config.toml)",
                dot_codex_folder.as_path().display()
            )
        }
        ConfigLayerSource::SessionFlags => "session-flags".to_string(),
        ConfigLayerSource::LegacyManagedConfigTomlFromFile { file } => {
            format!("legacy managed_config.toml ({})", file.as_path().display())
        }
        ConfigLayerSource::LegacyManagedConfigTomlFromMdm => {
            "legacy managed_config.toml (mdm)".to_string()
        }
    }
}

fn format_sandbox_mode_requirement(mode: SandboxModeRequirement) -> String {
    match mode {
        SandboxModeRequirement::ReadOnly => "read-only".to_string(),
        SandboxModeRequirement::WorkspaceWrite => "workspace-write".to_string(),
        SandboxModeRequirement::DangerFullAccess => "danger-full-access".to_string(),
        SandboxModeRequirement::ExternalSandbox => "external-sandbox".to_string(),
    }
}

fn format_residency_requirement(requirement: ResidencyRequirement) -> String {
    match requirement {
        ResidencyRequirement::Us => "us".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::render_debug_config_lines;
    use codex_app_server_protocol::ConfigLayerSource;
    use codex_core::config::Constrained;
    use codex_core::config_loader::ConfigLayerEntry;
    use codex_core::config_loader::ConfigLayerStack;
    use codex_core::config_loader::ConfigRequirements;
    use codex_core::config_loader::ConfigRequirementsToml;
    use codex_core::config_loader::ConstrainedWithSource;
    use codex_core::config_loader::McpServerIdentity;
    use codex_core::config_loader::McpServerRequirement;
    use codex_core::config_loader::RequirementSource;
    use codex_core::config_loader::ResidencyRequirement;
    use codex_core::config_loader::SandboxModeRequirement;
    use codex_core::config_loader::Sourced;
    use codex_core::protocol::AskForApproval;
    use codex_core::protocol::SandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use ratatui::text::Line;
    use std::collections::BTreeMap;
    use toml::Value as TomlValue;

    fn empty_toml_table() -> TomlValue {
        TomlValue::Table(toml::map::Map::new())
    }

    fn absolute_path(path: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::from_absolute_path(path).expect("absolute path")
    }

    fn render_to_text(lines: &[Line<'static>]) -> String {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn debug_config_output_lists_all_layers_including_disabled() {
        let system_file = if cfg!(windows) {
            absolute_path("C:\\etc\\codex\\config.toml")
        } else {
            absolute_path("/etc/codex/config.toml")
        };
        let project_folder = if cfg!(windows) {
            absolute_path("C:\\repo\\.codex")
        } else {
            absolute_path("/repo/.codex")
        };

        let layers = vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::System { file: system_file },
                empty_toml_table(),
            ),
            ConfigLayerEntry::new_disabled(
                ConfigLayerSource::Project {
                    dot_codex_folder: project_folder,
                },
                empty_toml_table(),
                "project is untrusted",
            ),
        ];
        let stack = ConfigLayerStack::new(
            layers,
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )
        .expect("config layer stack");

        let rendered = render_to_text(&render_debug_config_lines(&stack));
        assert!(rendered.contains("(enabled)"));
        assert!(rendered.contains("(disabled)"));
        assert!(rendered.contains("reason: project is untrusted"));
        assert!(rendered.contains("Requirements:"));
        assert!(rendered.contains("  <none>"));
    }

    #[test]
    fn debug_config_output_lists_requirement_sources() {
        let requirements_file = if cfg!(windows) {
            absolute_path("C:\\ProgramData\\OpenAI\\Codex\\requirements.toml")
        } else {
            absolute_path("/etc/codex/requirements.toml")
        };
        let mut requirements = ConfigRequirements::default();
        requirements.approval_policy = ConstrainedWithSource::new(
            Constrained::allow_any(AskForApproval::OnRequest),
            Some(RequirementSource::CloudRequirements),
        );
        requirements.sandbox_policy = ConstrainedWithSource::new(
            Constrained::allow_any(SandboxPolicy::ReadOnly),
            Some(RequirementSource::SystemRequirementsToml {
                file: requirements_file.clone(),
            }),
        );
        requirements.mcp_servers = Some(Sourced::new(
            BTreeMap::from([(
                "docs".to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Command {
                        command: "codex-mcp".to_string(),
                    },
                },
            )]),
            RequirementSource::LegacyManagedConfigTomlFromMdm,
        ));
        requirements.enforce_residency = ConstrainedWithSource::new(
            Constrained::allow_any(Some(ResidencyRequirement::Us)),
            Some(RequirementSource::CloudRequirements),
        );

        let requirements_toml = ConfigRequirementsToml {
            allowed_approval_policies: Some(vec![AskForApproval::OnRequest]),
            allowed_sandbox_modes: Some(vec![SandboxModeRequirement::ReadOnly]),
            mcp_servers: Some(BTreeMap::from([(
                "docs".to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Command {
                        command: "codex-mcp".to_string(),
                    },
                },
            )])),
            rules: None,
            enforce_residency: Some(ResidencyRequirement::Us),
        };

        let user_file = if cfg!(windows) {
            absolute_path("C:\\users\\alice\\.codex\\config.toml")
        } else {
            absolute_path("/home/alice/.codex/config.toml")
        };
        let stack = ConfigLayerStack::new(
            vec![ConfigLayerEntry::new(
                ConfigLayerSource::User { file: user_file },
                empty_toml_table(),
            )],
            requirements,
            requirements_toml,
        )
        .expect("config layer stack");

        let rendered = render_to_text(&render_debug_config_lines(&stack));
        assert!(
            rendered.contains("allowed_approval_policies: on-request (source: cloud requirements)")
        );
        assert!(
            rendered.contains(
                format!(
                    "allowed_sandbox_modes: read-only (source: {})",
                    requirements_file.as_path().display()
                )
                .as_str(),
            )
        );
        assert!(rendered.contains("mcp_servers: docs (source: MDM managed_config.toml (legacy))"));
        assert!(rendered.contains("enforce_residency: us (source: cloud requirements)"));
        assert!(!rendered.contains("  - rules:"));
    }
}
