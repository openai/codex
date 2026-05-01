use crate::session::turn_context::TurnContext;
use crate::shell::Shell;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::TurnContextNetworkItem;
use std::path::PathBuf;

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EnvironmentContext {
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) shell: String,
    pub(crate) environments: Vec<EnvironmentContextEnvironment>,
    pub(crate) current_date: Option<String>,
    pub(crate) timezone: Option<String>,
    pub(crate) network: Option<NetworkContext>,
    pub(crate) subagents: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnvironmentContextEnvironment {
    pub(crate) id: String,
    pub(crate) cwd: PathBuf,
    pub(crate) default: bool,
}

impl EnvironmentContextEnvironment {
    fn from_turn_environments(
        environments: &[crate::session::turn_context::TurnEnvironment],
    ) -> Vec<Self> {
        environments
            .iter()
            .enumerate()
            .map(|(index, environment)| Self {
                id: environment.environment_id.clone(),
                cwd: environment.cwd.to_path_buf(),
                default: index == 0,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct NetworkContext {
    allowed_domains: Vec<String>,
    denied_domains: Vec<String>,
}

impl NetworkContext {
    pub(crate) fn new(allowed_domains: Vec<String>, denied_domains: Vec<String>) -> Self {
        Self {
            allowed_domains,
            denied_domains,
        }
    }
}

impl EnvironmentContext {
    fn should_render_environments(environment_count: usize) -> bool {
        environment_count > 1
    }

    fn model_facing_environments(
        environments: Vec<EnvironmentContextEnvironment>,
    ) -> Vec<EnvironmentContextEnvironment> {
        if Self::should_render_environments(environments.len()) {
            environments
        } else {
            Vec::new()
        }
    }

    pub(crate) fn new(
        cwd: Option<PathBuf>,
        shell: String,
        environments: Vec<EnvironmentContextEnvironment>,
        current_date: Option<String>,
        timezone: Option<String>,
        network: Option<NetworkContext>,
        subagents: Option<String>,
    ) -> Self {
        Self {
            cwd,
            shell,
            environments,
            current_date,
            timezone,
            network,
            subagents,
        }
    }

    /// Compares two environment contexts, ignoring the shell. Useful when
    /// comparing turn to turn, since the initial environment_context will
    /// include the shell, and then it is not configurable from turn to turn.
    pub(crate) fn equals_except_shell(&self, other: &EnvironmentContext) -> bool {
        let EnvironmentContext {
            cwd,
            environments,
            current_date,
            timezone,
            network,
            subagents,
            shell: _,
        } = other;
        self.cwd == *cwd
            && self.environments == *environments
            && self.current_date == *current_date
            && self.timezone == *timezone
            && self.network == *network
            && self.subagents == *subagents
    }

    pub(crate) fn diff_from_turn_context_item(
        before: &TurnContextItem,
        after: &EnvironmentContext,
    ) -> Self {
        let before_network = Self::network_from_turn_context_item(before);
        let environments = Self::model_facing_environments(after.environments.clone());
        let cwd = match &after.cwd {
            Some(cwd) if before.cwd.as_path() != cwd.as_path() => Some(cwd.clone()),
            _ => None,
        };
        let network = if before_network != after.network {
            after.network.clone()
        } else {
            before_network
        };
        EnvironmentContext::new(
            cwd,
            after.shell.clone(),
            environments,
            after.current_date.clone(),
            after.timezone.clone(),
            network,
            /*subagents*/ None,
        )
    }

    pub(crate) fn from_turn_context(turn_context: &TurnContext, shell: &Shell) -> Self {
        let environments =
            EnvironmentContextEnvironment::from_turn_environments(&turn_context.environments);
        let environments = Self::model_facing_environments(environments);
        Self::new(
            Some(turn_context.cwd.to_path_buf()),
            shell.name().to_string(),
            environments,
            turn_context.current_date.clone(),
            turn_context.timezone.clone(),
            Self::network_from_turn_context(turn_context),
            /*subagents*/ None,
        )
    }

    pub(crate) fn from_turn_context_item(
        turn_context_item: &TurnContextItem,
        shell: String,
    ) -> Self {
        Self::new(
            Some(turn_context_item.cwd.clone()),
            shell,
            Vec::new(),
            turn_context_item.current_date.clone(),
            turn_context_item.timezone.clone(),
            Self::network_from_turn_context_item(turn_context_item),
            /*subagents*/ None,
        )
    }

    pub(crate) fn with_subagents(mut self, subagents: String) -> Self {
        if !subagents.is_empty() {
            self.subagents = Some(subagents);
        }
        self
    }

    fn network_from_turn_context(turn_context: &TurnContext) -> Option<NetworkContext> {
        let network = turn_context
            .config
            .config_layer_stack
            .requirements()
            .network
            .as_ref()?;

        Some(NetworkContext::new(
            network
                .domains
                .as_ref()
                .and_then(codex_config::NetworkDomainPermissionsToml::allowed_domains)
                .unwrap_or_default(),
            network
                .domains
                .as_ref()
                .and_then(codex_config::NetworkDomainPermissionsToml::denied_domains)
                .unwrap_or_default(),
        ))
    }

    fn network_from_turn_context_item(
        turn_context_item: &TurnContextItem,
    ) -> Option<NetworkContext> {
        let TurnContextNetworkItem {
            allowed_domains,
            denied_domains,
        } = turn_context_item.network.as_ref()?;
        Some(NetworkContext::new(
            allowed_domains.clone(),
            denied_domains.clone(),
        ))
    }
}

impl ContextualUserFragment for EnvironmentContext {
    const ROLE: &'static str = "user";
    const START_MARKER: &'static str = codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
    const END_MARKER: &'static str = codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;

    fn body(&self) -> String {
        let mut lines = Vec::new();
        if !self.environments.is_empty() {
            lines.push("  <environments>".to_string());
            for environment in &self.environments {
                lines.push(format!(
                    "    <environment id=\"{}\" default=\"{}\">",
                    environment.id, environment.default
                ));
                lines.push(format!(
                    "      <cwd>{}</cwd>",
                    environment.cwd.to_string_lossy()
                ));
                lines.push("    </environment>".to_string());
            }
            lines.push("  </environments>".to_string());
        } else if let Some(cwd) = &self.cwd {
            lines.push(format!("  <cwd>{}</cwd>", cwd.to_string_lossy()));
        }

        lines.push(format!("  <shell>{}</shell>", self.shell));
        if let Some(current_date) = &self.current_date {
            lines.push(format!("  <current_date>{current_date}</current_date>"));
        }
        if let Some(timezone) = &self.timezone {
            lines.push(format!("  <timezone>{timezone}</timezone>"));
        }
        match &self.network {
            Some(network) => {
                lines.push("  <network enabled=\"true\">".to_string());
                for allowed in &network.allowed_domains {
                    lines.push(format!("    <allowed>{allowed}</allowed>"));
                }
                for denied in &network.denied_domains {
                    lines.push(format!("    <denied>{denied}</denied>"));
                }
                lines.push("  </network>".to_string());
            }
            None => {
                // TODO(mbolin): Include this line if it helps the model.
                // lines.push("  <network enabled=\"false\" />".to_string());
            }
        }
        if let Some(subagents) = &self.subagents {
            lines.push("  <subagents>".to_string());
            lines.extend(subagents.lines().map(|line| format!("    {line}")));
            lines.push("  </subagents>".to_string());
        }
        format!("\n{}\n", lines.join("\n"))
    }
}

#[cfg(test)]
#[path = "environment_context_tests.rs"]
mod tests;
