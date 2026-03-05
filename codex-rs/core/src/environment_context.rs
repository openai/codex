use crate::codex::TurnContext;
use crate::model_visible_context::ContextualUserContextRole;
use crate::model_visible_context::ENVIRONMENT_CONTEXT_FRAGMENT_SPEC;
use crate::model_visible_context::ModelVisibleContextFragment;
use crate::model_visible_context::TurnBackedContextFragment;
use crate::shell::Shell;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::TurnContextNetworkItem;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "environment_context", rename_all = "snake_case")]
pub(crate) struct EnvironmentContext {
    pub cwd: Option<PathBuf>,
    pub shell: Shell,
    pub current_date: Option<String>,
    pub timezone: Option<String>,
    pub network: Option<NetworkContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub(crate) struct NetworkContext {
    allowed_domains: Vec<String>,
    denied_domains: Vec<String>,
}

impl EnvironmentContext {
    pub fn new(
        cwd: Option<PathBuf>,
        shell: Shell,
        current_date: Option<String>,
        timezone: Option<String>,
        network: Option<NetworkContext>,
    ) -> Self {
        Self {
            cwd,
            shell,
            current_date,
            timezone,
            network,
        }
    }

    /// Compares two environment contexts, ignoring the shell. Useful when
    /// comparing turn to turn, since the initial environment_context will
    /// include the shell, and then it is not configurable from turn to turn.
    pub fn equals_except_shell(&self, other: &EnvironmentContext) -> bool {
        let EnvironmentContext {
            cwd,
            current_date,
            timezone,
            network,
            shell: _,
        } = other;
        self.cwd == *cwd
            && self.current_date == *current_date
            && self.timezone == *timezone
            && self.network == *network
    }

    fn network_from_turn_context(turn_context: &TurnContext) -> Option<NetworkContext> {
        let network = turn_context
            .config
            .config_layer_stack
            .requirements()
            .network
            .as_ref()?;

        Some(NetworkContext {
            allowed_domains: network.allowed_domains.clone().unwrap_or_default(),
            denied_domains: network.denied_domains.clone().unwrap_or_default(),
        })
    }

    fn network_from_turn_context_item(
        turn_context_item: &TurnContextItem,
    ) -> Option<NetworkContext> {
        let TurnContextNetworkItem {
            allowed_domains,
            denied_domains,
        } = turn_context_item.network.as_ref()?;
        Some(NetworkContext {
            allowed_domains: allowed_domains.clone(),
            denied_domains: denied_domains.clone(),
        })
    }
}

impl ModelVisibleContextFragment for EnvironmentContext {
    type Role = ContextualUserContextRole;

    fn spec(&self) -> crate::model_visible_context::ModelVisibleContextFragmentSpec {
        ENVIRONMENT_CONTEXT_FRAGMENT_SPEC
    }

    fn render_text(&self) -> String {
        let mut lines = Vec::new();
        if let Some(cwd) = &self.cwd {
            lines.push(format!("  <cwd>{}</cwd>", cwd.to_string_lossy()));
        }

        let shell_name = self.shell.name();
        lines.push(format!("  <shell>{shell_name}</shell>"));
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
        ENVIRONMENT_CONTEXT_FRAGMENT_SPEC.wrap_body(lines.join("\n"))
    }
}

impl TurnBackedContextFragment for EnvironmentContext {
    fn from_turn_context(turn_context: &TurnContext, shell: &Shell) -> Option<Self> {
        Some(Self::new(
            Some(turn_context.cwd.clone()),
            shell.clone(),
            turn_context.current_date.clone(),
            turn_context.timezone.clone(),
            Self::network_from_turn_context(turn_context),
        ))
    }

    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        shell: &Shell,
    ) -> Option<Self> {
        let previous_context = Self::new(
            Some(previous.cwd.clone()),
            shell.clone(),
            previous.current_date.clone(),
            previous.timezone.clone(),
            Self::network_from_turn_context_item(previous),
        );
        let next_context = Self::from_turn_context(turn_context, shell)?;
        if previous_context.equals_except_shell(&next_context) {
            return None;
        }

        let previous_network = Self::network_from_turn_context_item(previous);
        let current_network = Self::network_from_turn_context(turn_context);
        let cwd = if previous.cwd != turn_context.cwd {
            Some(turn_context.cwd.clone())
        } else {
            None
        };
        let network = if previous_network != current_network {
            current_network
        } else {
            previous_network
        };

        Some(Self::new(
            cwd,
            shell.clone(),
            turn_context.current_date.clone(),
            turn_context.timezone.clone(),
            network,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::shell::ShellType;

    use super::*;
    use core_test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    fn fake_shell() -> Shell {
        Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
        }
    }

    #[test]
    fn serialize_workspace_write_environment_context() {
        let cwd = test_path_buf("/repo");
        let context = EnvironmentContext::new(
            Some(cwd.clone()),
            fake_shell(),
            Some("2026-02-26".to_string()),
            Some("America/Los_Angeles".to_string()),
            None,
        );

        let expected = format!(
            r#"<environment_context>
  <cwd>{cwd}</cwd>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#,
            cwd = cwd.display(),
        );

        assert_eq!(context.render_text(), expected);
    }

    #[test]
    fn serialize_environment_context_with_network() {
        let network = NetworkContext {
            allowed_domains: vec!["api.example.com".to_string(), "*.openai.com".to_string()],
            denied_domains: vec!["blocked.example.com".to_string()],
        };
        let context = EnvironmentContext::new(
            Some(test_path_buf("/repo")),
            fake_shell(),
            Some("2026-02-26".to_string()),
            Some("America/Los_Angeles".to_string()),
            Some(network),
        );

        let expected = format!(
            r#"<environment_context>
  <cwd>{}</cwd>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
  <network enabled="true">
    <allowed>api.example.com</allowed>
    <allowed>*.openai.com</allowed>
    <denied>blocked.example.com</denied>
  </network>
</environment_context>"#,
            test_path_buf("/repo").display()
        );

        assert_eq!(context.render_text(), expected);
    }

    #[test]
    fn serialize_read_only_environment_context() {
        let context = EnvironmentContext::new(
            None,
            fake_shell(),
            Some("2026-02-26".to_string()),
            Some("America/Los_Angeles".to_string()),
            None,
        );

        let expected = r#"<environment_context>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

        assert_eq!(context.render_text(), expected);
    }

    #[test]
    fn serialize_external_sandbox_environment_context() {
        let context = EnvironmentContext::new(
            None,
            fake_shell(),
            Some("2026-02-26".to_string()),
            Some("America/Los_Angeles".to_string()),
            None,
        );

        let expected = r#"<environment_context>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

        assert_eq!(context.render_text(), expected);
    }

    #[test]
    fn serialize_external_sandbox_with_restricted_network_environment_context() {
        let context = EnvironmentContext::new(
            None,
            fake_shell(),
            Some("2026-02-26".to_string()),
            Some("America/Los_Angeles".to_string()),
            None,
        );

        let expected = r#"<environment_context>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

        assert_eq!(context.render_text(), expected);
    }

    #[test]
    fn serialize_full_access_environment_context() {
        let context = EnvironmentContext::new(
            None,
            fake_shell(),
            Some("2026-02-26".to_string()),
            Some("America/Los_Angeles".to_string()),
            None,
        );

        let expected = r#"<environment_context>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

        assert_eq!(context.render_text(), expected);
    }

    #[test]
    fn equals_except_shell_compares_cwd() {
        let context1 =
            EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell(), None, None, None);
        let context2 =
            EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell(), None, None, None);
        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_ignores_sandbox_policy() {
        let context1 =
            EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell(), None, None, None);
        let context2 =
            EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell(), None, None, None);

        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_compares_cwd_differences() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo1")),
            fake_shell(),
            None,
            None,
            None,
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo2")),
            fake_shell(),
            None,
            None,
            None,
        );

        assert!(!context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_ignores_shell() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Shell {
                shell_type: ShellType::Bash,
                shell_path: "/bin/bash".into(),
                shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
            },
            None,
            None,
            None,
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Shell {
                shell_type: ShellType::Zsh,
                shell_path: "/bin/zsh".into(),
                shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
            },
            None,
            None,
            None,
        );

        assert!(context1.equals_except_shell(&context2));
    }
}
