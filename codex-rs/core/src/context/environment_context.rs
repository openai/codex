use crate::session::turn_context::TurnContext;
use crate::shell::Shell;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::TurnContextNetworkItem;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EnvironmentContext {
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
    pub(crate) cwd: AbsolutePathBuf,
}

impl EnvironmentContextEnvironment {
    fn legacy(cwd: AbsolutePathBuf) -> Self {
        Self {
            id: String::new(),
            cwd,
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
    pub(crate) fn new(
        shell: String,
        environments: Vec<EnvironmentContextEnvironment>,
        current_date: Option<String>,
        timezone: Option<String>,
        network: Option<NetworkContext>,
        subagents: Option<String>,
    ) -> Self {
        Self {
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
            environments,
            current_date,
            timezone,
            network,
            subagents,
            shell: _,
        } = other;
        self.model_facing_single_cwd() == Self::single_environment_cwd(environments)
            && self.model_facing_multiple_environments()
                == Self::multiple_environments(environments)
            && self.current_date == *current_date
            && self.timezone == *timezone
            && self.network == *network
            && self.subagents == *subagents
    }

    pub(crate) fn diff_from_turn_context_item(
        before: &TurnContextItem,
        after: &EnvironmentContext,
    ) -> Self {
        let before_context = Self::from_turn_context_item(before, after.shell.clone());
        let before_network = Self::network_from_turn_context_item(before);
        let environments = after
            .model_facing_multiple_environments()
            .map(<[_]>::to_vec)
            .unwrap_or_else(|| {
                after
                    .model_facing_single_cwd()
                    .filter(|cwd| before_context.model_facing_single_cwd() != Some(*cwd))
                    .cloned()
                    .map(EnvironmentContextEnvironment::legacy)
                    .into_iter()
                    .collect()
            });
        let network = if before_network != after.network {
            after.network.clone()
        } else {
            before_network
        };
        EnvironmentContext::new(
            after.shell.clone(),
            environments,
            after.current_date.clone(),
            after.timezone.clone(),
            network,
            /*subagents*/ None,
        )
    }

    pub(crate) fn from_turn_context(turn_context: &TurnContext, shell: &Shell) -> Self {
        Self::new(
            shell.name().to_string(),
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
        Self::new(
            shell,
            vec![EnvironmentContextEnvironment::legacy(
                AbsolutePathBuf::try_from(turn_context_item.cwd.clone())
                    .expect("turn context item cwd must be absolute"),
            )],
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

    fn model_facing_single_cwd(&self) -> Option<&AbsolutePathBuf> {
        Self::single_environment_cwd(&self.environments)
    }

    fn model_facing_multiple_environments(&self) -> Option<&[EnvironmentContextEnvironment]> {
        Self::multiple_environments(&self.environments)
    }

    fn single_environment_cwd(
        environments: &[EnvironmentContextEnvironment],
    ) -> Option<&AbsolutePathBuf> {
        (environments.len() == 1).then(|| &environments[0].cwd)
    }

    fn multiple_environments(
        environments: &[EnvironmentContextEnvironment],
    ) -> Option<&[EnvironmentContextEnvironment]> {
        (environments.len() > 1).then_some(environments)
    }
}

impl ContextualUserFragment for EnvironmentContext {
    const ROLE: &'static str = "user";
    const START_MARKER: &'static str = codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
    const END_MARKER: &'static str = codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;

    fn body(&self) -> String {
        let mut lines = Vec::new();
        if let Some(environments) = self.model_facing_multiple_environments() {
            lines.push("  <environments>".to_string());
            for environment in environments {
                lines.push(format!("    <environment id=\"{}\">", environment.id));
                lines.push(format!(
                    "      <cwd>{}</cwd>",
                    environment.cwd.to_string_lossy()
                ));
                lines.push("    </environment>".to_string());
            }
            lines.push("  </environments>".to_string());
        } else if let Some(cwd) = self.model_facing_single_cwd() {
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
