use crate::codex::TurnContext;
use crate::shell::Shell;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
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
    pub network: Option<NetworkContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub(crate) struct NetworkContext {
    allowed_domains: Vec<String>,
    denied_domains: Vec<String>,
}

impl EnvironmentContext {
    pub fn new(cwd: Option<PathBuf>, shell: Shell, network: Option<NetworkContext>) -> Self {
        Self {
            cwd,
            shell,
            network,
        }
    }

    /// Compares two environment contexts, ignoring the shell. Useful when
    /// comparing turn to turn, since the initial environment_context will
    /// include the shell, and then it is not configurable from turn to turn.
    pub fn equals_except_shell(&self, other: &EnvironmentContext) -> bool {
        let EnvironmentContext {
            cwd,
            network,
            // should compare all fields except shell
            shell: _,
        } = other;
        self.cwd == *cwd && self.network == *network
    }

    pub fn diff_from_turn_context_item(
        before: &TurnContextItem,
        after: &TurnContext,
        shell: &Shell,
    ) -> Self {
        let before_network = Self::network_from_turn_context_item(before);
        let after_network = Self::network_from_turn_context(after);
        let cwd = if before.cwd != after.cwd {
            Some(after.cwd.clone())
        } else {
            None
        };
        let network = if before_network != after_network {
            after_network
        } else {
            before_network
        };
        EnvironmentContext::new(cwd, shell.clone(), network)
    }

    pub fn from_turn_context(turn_context: &TurnContext, shell: &Shell) -> Self {
        Self::new(
            Some(turn_context.cwd.clone()),
            shell.clone(),
            Self::network_from_turn_context(turn_context),
        )
    }

    pub fn from_turn_context_item(turn_context_item: &TurnContextItem, shell: &Shell) -> Self {
        Self::new(
            Some(turn_context_item.cwd.clone()),
            shell.clone(),
            Self::network_from_turn_context_item(turn_context_item),
        )
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
        let mut lines = vec![ENVIRONMENT_CONTEXT_OPEN_TAG.to_string()];
        if let Some(cwd) = self.cwd {
            lines.push(format!("  <cwd>{}</cwd>", cwd.to_string_lossy()));
        }

        let shell_name = self.shell.name();
        lines.push(format!("  <shell>{shell_name}</shell>"));
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
        lines.push(ENVIRONMENT_CONTEXT_CLOSE_TAG.to_string());
        lines.join("\n")
    }
}

impl From<EnvironmentContext> for ResponseItem {
    fn from(ec: EnvironmentContext) -> Self {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: ec.serialize_to_xml(),
            }],
            end_turn: None,
            phase: None,
        }
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
        let context = EnvironmentContext::new(Some(cwd.clone()), fake_shell(), None);

        let expected = format!(
            r#"<environment_context>
  <cwd>{cwd}</cwd>
  <shell>bash</shell>
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
        let context =
            EnvironmentContext::new(Some(test_path_buf("/repo")), fake_shell(), Some(network));

        let expected = format!(
            r#"<environment_context>
  <cwd>{}</cwd>
  <shell>bash</shell>
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
        let context = EnvironmentContext::new(None, fake_shell(), None);

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_external_sandbox_environment_context() {
        let context = EnvironmentContext::new(None, fake_shell(), None);

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_external_sandbox_with_restricted_network_environment_context() {
        let context = EnvironmentContext::new(None, fake_shell(), None);

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_full_access_environment_context() {
        let context = EnvironmentContext::new(None, fake_shell(), None);

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn equals_except_shell_compares_cwd() {
        let context1 = EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell(), None);
        let context2 = EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell(), None);
        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_ignores_sandbox_policy() {
        let context1 = EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell(), None);
        let context2 = EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell(), None);

        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_compares_cwd_differences() {
        let context1 = EnvironmentContext::new(Some(PathBuf::from("/repo1")), fake_shell(), None);
        let context2 = EnvironmentContext::new(Some(PathBuf::from("/repo2")), fake_shell(), None);

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
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Shell {
                shell_type: ShellType::Zsh,
                shell_path: "/bin/zsh".into(),
                shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
            },
            None,
        );

        assert!(context1.equals_except_shell(&context2));
    }
}
