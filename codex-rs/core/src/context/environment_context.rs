use crate::session::turn_context::TurnContext;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::TurnContextNetworkItem;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EnvironmentContext {
    pub(crate) environments: EnvironmentContextEnvironments,
    pub(crate) current_date: Option<String>,
    pub(crate) timezone: Option<String>,
    pub(crate) network: Option<NetworkContext>,
    pub(crate) subagents: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnvironmentContextEnvironment {
    pub(crate) id: String,
    pub(crate) cwd: AbsolutePathBuf,
    pub(crate) shell: String,
}

impl EnvironmentContextEnvironment {
    fn legacy(cwd: AbsolutePathBuf, shell: String) -> Self {
        Self {
            id: String::new(),
            cwd,
            shell,
        }
    }

    fn from_turn_environments(
        environments: &[crate::session::turn_context::TurnEnvironment],
    ) -> Vec<Self> {
        environments
            .iter()
            .map(|environment| Self {
                id: environment.environment_id.clone(),
                cwd: environment.cwd.clone(),
                shell: environment.shell.clone(),
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EnvironmentContextEnvironments {
    None,
    Single(EnvironmentContextEnvironment),
    Multiple(Vec<EnvironmentContextEnvironment>),
}

impl EnvironmentContextEnvironments {
    fn from_vec(environments: Vec<EnvironmentContextEnvironment>) -> Self {
        let mut environments = environments;
        match environments.pop() {
            None => Self::None,
            Some(environment) if environments.is_empty() => Self::Single(environment),
            Some(environment) => {
                environments.push(environment);
                Self::Multiple(environments)
            }
        }
    }

    fn equals_except_shell(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None) => true,
            (Self::Single(left), Self::Single(right)) => left.cwd == right.cwd,
            (Self::Multiple(left), Self::Multiple(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.id == right.id && left.cwd == right.cwd)
            }
            _ => false,
        }
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
    pub(crate) fn new(
        environments: Vec<EnvironmentContextEnvironment>,
        current_date: Option<String>,
        timezone: Option<String>,
        network: Option<NetworkContext>,
        subagents: Option<String>,
    ) -> Self {
        Self {
            environments: EnvironmentContextEnvironments::from_vec(environments),
            current_date,
            timezone,
            network,
            subagents,
        }
    }

    fn new_with_environments(
        environments: EnvironmentContextEnvironments,
        current_date: Option<String>,
        timezone: Option<String>,
        network: Option<NetworkContext>,
        subagents: Option<String>,
    ) -> Self {
        Self {
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
        self.environments.equals_except_shell(&other.environments)
            && self.current_date == other.current_date
            && self.timezone == other.timezone
            && self.network == other.network
            && self.subagents == other.subagents
    }

    pub(crate) fn diff_from_turn_context_item(
        before: &TurnContextItem,
        after: &EnvironmentContext,
    ) -> Self {
        let before_network = Self::network_from_turn_context_item(before);
        let environments = match &after.environments {
            EnvironmentContextEnvironments::Single(environment) => {
                if before.cwd.as_path() != environment.cwd.as_path() {
                    EnvironmentContextEnvironments::Single(EnvironmentContextEnvironment::legacy(
                        environment.cwd.clone(),
                        environment.shell.clone(),
                    ))
                } else {
                    EnvironmentContextEnvironments::None
                }
            }
            EnvironmentContextEnvironments::Multiple(environments) => {
                EnvironmentContextEnvironments::Multiple(environments.clone())
            }
            EnvironmentContextEnvironments::None => EnvironmentContextEnvironments::None,
        };
        let network = if before_network != after.network {
            after.network.clone()
        } else {
            before_network
        };
        EnvironmentContext::new_with_environments(
            environments,
            after.current_date.clone(),
            after.timezone.clone(),
            network,
            /*subagents*/ None,
        )
    }

    pub(crate) fn from_turn_context(turn_context: &TurnContext) -> Self {
        Self::new(
            EnvironmentContextEnvironment::from_turn_environments(&turn_context.environments),
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
        let cwd = match AbsolutePathBuf::try_from(turn_context_item.cwd.clone()) {
            Ok(cwd) => cwd,
            Err(_) => AbsolutePathBuf::resolve_path_against_base(&turn_context_item.cwd, "/"),
        };
        Self::new(
            vec![EnvironmentContextEnvironment::legacy(cwd, shell)],
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
        match &self.environments {
            EnvironmentContextEnvironments::Single(environment) => {
                lines.push(format!(
                    "  <cwd>{}</cwd>",
                    environment.cwd.to_string_lossy()
                ));
                lines.push(format!("  <shell>{}</shell>", environment.shell));
            }
            EnvironmentContextEnvironments::Multiple(environments) => {
                lines.push("  <environments>".to_string());
                for environment in environments {
                    lines.push(format!("    <environment id=\"{}\">", environment.id));
                    lines.push(format!(
                        "      <cwd>{}</cwd>",
                        environment.cwd.to_string_lossy()
                    ));
                    lines.push(format!("      <shell>{}</shell>", environment.shell));
                    lines.push("    </environment>".to_string());
                }
                lines.push("  </environments>".to_string());
            }
            EnvironmentContextEnvironments::None => {}
        }
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
