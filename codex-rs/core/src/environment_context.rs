use crate::codex::TurnContext;
use crate::contextual_user_message::ENVIRONMENT_CONTEXT_FRAGMENT;
use crate::shell::Shell;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::TurnContextNetworkItem;
use codex_utils_absolute_path::AbsolutePathBuf;
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
    pub deny_read_paths: Vec<AbsolutePathBuf>,
    pub subagents: Option<String>,
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
        deny_read_paths: Vec<AbsolutePathBuf>,
        subagents: Option<String>,
    ) -> Self {
        Self {
            cwd,
            shell,
            current_date,
            timezone,
            network,
            deny_read_paths,
            subagents,
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
            deny_read_paths,
            subagents,
            shell: _,
        } = other;
        self.cwd == *cwd
            && self.current_date == *current_date
            && self.timezone == *timezone
            && self.network == *network
            && self.deny_read_paths == *deny_read_paths
            && self.subagents == *subagents
    }

    pub fn diff_from_turn_context_item(
        before: &TurnContextItem,
        after: &TurnContext,
        shell: &Shell,
    ) -> Self {
        let before_network = Self::network_from_turn_context_item(before);
        let after_network = Self::network_from_turn_context(after);
        let before_deny_read_paths = before.sandbox_policy.denied_read_paths();
        let after_deny_read_paths = after.sandbox_policy.denied_read_paths();
        let cwd = if before.cwd != after.cwd {
            Some(after.cwd.clone())
        } else {
            None
        };
        let current_date = after.current_date.clone();
        let timezone = after.timezone.clone();
        let network = if before_network != after_network {
            after_network
        } else {
            before_network
        };
        let deny_read_paths = if before_deny_read_paths != after_deny_read_paths {
            after_deny_read_paths
        } else {
            before_deny_read_paths
        };
        EnvironmentContext::new(
            cwd,
            shell.clone(),
            current_date,
            timezone,
            network,
            deny_read_paths,
            None,
        )
    }

    pub fn from_turn_context(turn_context: &TurnContext, shell: &Shell) -> Self {
        Self::new(
            Some(turn_context.cwd.clone()),
            shell.clone(),
            turn_context.current_date.clone(),
            turn_context.timezone.clone(),
            Self::network_from_turn_context(turn_context),
            turn_context.sandbox_policy.denied_read_paths(),
            None,
        )
    }

    pub fn from_turn_context_item(turn_context_item: &TurnContextItem, shell: &Shell) -> Self {
        Self::new(
            Some(turn_context_item.cwd.clone()),
            shell.clone(),
            turn_context_item.current_date.clone(),
            turn_context_item.timezone.clone(),
            Self::network_from_turn_context_item(turn_context_item),
            turn_context_item.sandbox_policy.denied_read_paths(),
            None,
        )
    }

    pub fn with_subagents(mut self, subagents: String) -> Self {
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

impl EnvironmentContext {
    /// Serializes the environment context to XML. Libraries like `quick-xml`
    /// require custom macros to handle Enums with newtypes, so we just do it
    /// manually, to keep things simple. Output looks like:
    ///
    /// ```xml
    /// <environment_context>
    ///   <cwd>...</cwd>
    ///   <shell>...</shell>
    /// </environment_context>
    /// ```
    pub fn serialize_to_xml(self) -> String {
        let mut lines = Vec::new();
        if let Some(cwd) = self.cwd {
            lines.push(format!("  <cwd>{}</cwd>", cwd.to_string_lossy()));
        }

        let shell_name = self.shell.name();
        lines.push(format!("  <shell>{shell_name}</shell>"));
        if let Some(current_date) = self.current_date {
            lines.push(format!("  <current_date>{current_date}</current_date>"));
        }
        if let Some(timezone) = self.timezone {
            lines.push(format!("  <timezone>{timezone}</timezone>"));
        }
        match self.network {
            Some(ref network) => {
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
        if !self.deny_read_paths.is_empty() {
            lines.push("  <deny_read_paths>".to_string());
            for path in &self.deny_read_paths {
                lines.push(format!("    <path>{}</path>", path.to_string_lossy()));
            }
            lines.push("  </deny_read_paths>".to_string());
        }
        if let Some(subagents) = self.subagents {
            lines.push("  <subagents>".to_string());
            lines.extend(subagents.lines().map(|line| format!("    {line}")));
            lines.push("  </subagents>".to_string());
        }
        ENVIRONMENT_CONTEXT_FRAGMENT.wrap(lines.join("\n"))
    }
}

impl From<EnvironmentContext> for ResponseItem {
    fn from(ec: EnvironmentContext) -> Self {
        ENVIRONMENT_CONTEXT_FRAGMENT.into_message(ec.serialize_to_xml())
    }
}

#[cfg(test)]
mod tests {
    use crate::shell::ShellType;

    use super::*;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use core_test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    fn fake_shell() -> Shell {
        Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
        }
    }

    fn deny_path(path: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::try_from(path).expect("deny path should be absolute")
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
            Vec::new(),
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

        assert_eq!(context.serialize_to_xml(), expected);
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
            Vec::new(),
            None,
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

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_read_only_environment_context() {
        let context = EnvironmentContext::new(
            None,
            fake_shell(),
            Some("2026-02-26".to_string()),
            Some("America/Los_Angeles".to_string()),
            None,
            Vec::new(),
            None,
        );

        let expected = r#"<environment_context>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_external_sandbox_environment_context() {
        let context = EnvironmentContext::new(
            None,
            fake_shell(),
            Some("2026-02-26".to_string()),
            Some("America/Los_Angeles".to_string()),
            None,
            Vec::new(),
            None,
        );

        let expected = r#"<environment_context>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_external_sandbox_with_restricted_network_environment_context() {
        let context = EnvironmentContext::new(
            None,
            fake_shell(),
            Some("2026-02-26".to_string()),
            Some("America/Los_Angeles".to_string()),
            None,
            Vec::new(),
            None,
        );

        let expected = r#"<environment_context>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_full_access_environment_context() {
        let context = EnvironmentContext::new(
            None,
            fake_shell(),
            Some("2026-02-26".to_string()),
            Some("America/Los_Angeles".to_string()),
            None,
            Vec::new(),
            None,
        );

        let expected = r#"<environment_context>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn equals_except_shell_compares_cwd() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            fake_shell(),
            None,
            None,
            None,
            Vec::new(),
            None,
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            fake_shell(),
            None,
            None,
            None,
            Vec::new(),
            None,
        );
        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_ignores_sandbox_policy() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            fake_shell(),
            None,
            None,
            None,
            Vec::new(),
            None,
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            fake_shell(),
            None,
            None,
            None,
            Vec::new(),
            None,
        );

        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn serialize_environment_context_with_deny_read_paths() {
        let denied = vec![deny_path("/repo/.gitconfig"), deny_path("/repo/.ssh")];
        let context = EnvironmentContext::new(
            Some(test_path_buf("/repo")),
            fake_shell(),
            Some("2026-02-26".to_string()),
            Some("America/Los_Angeles".to_string()),
            None,
            denied,
            None,
        );

        let expected = format!(
            r#"<environment_context>
  <cwd>{}</cwd>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
  <deny_read_paths>
    <path>/repo/.gitconfig</path>
    <path>/repo/.ssh</path>
  </deny_read_paths>
</environment_context>"#,
            test_path_buf("/repo").display()
        );

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn equals_except_shell_compares_cwd_differences() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo1")),
            fake_shell(),
            None,
            None,
            None,
            Vec::new(),
            None,
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo2")),
            fake_shell(),
            None,
            None,
            None,
            Vec::new(),
            None,
        );

        assert!(!context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_compares_deny_read_paths() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            fake_shell(),
            None,
            None,
            None,
            vec![deny_path("/repo/.gitconfig")],
            None,
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            fake_shell(),
            None,
            None,
            None,
            vec![deny_path("/repo/.ssh")],
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
            Vec::new(),
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
            Vec::new(),
            None,
        );

        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn serialize_environment_context_with_subagents() {
        let context = EnvironmentContext::new(
            Some(test_path_buf("/repo")),
            fake_shell(),
            Some("2026-02-26".to_string()),
            Some("America/Los_Angeles".to_string()),
            None,
            Vec::new(),
            Some("- agent-1: atlas\n- agent-2".to_string()),
        );

        let expected = format!(
            r#"<environment_context>
  <cwd>{}</cwd>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
  <subagents>
    - agent-1: atlas
    - agent-2
  </subagents>
</environment_context>"#,
            test_path_buf("/repo").display()
        );

        assert_eq!(context.serialize_to_xml(), expected);
    }
}
