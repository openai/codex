use crate::codex::TurnContext;
use crate::shell::Shell;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "environment_context", rename_all = "snake_case")]
pub(crate) struct EnvironmentContext {
    pub cwd: Option<PathBuf>,
    pub shell: Shell,
}

impl EnvironmentContext {
    pub fn new(cwd: Option<PathBuf>, shell: Shell) -> Self {
        Self { cwd, shell }
    }

    /// Compares two environment contexts, ignoring the shell. Useful when
    /// comparing turn to turn, since the initial environment_context will
    /// include the shell, and then it is not configurable from turn to turn.
    pub fn equals_except_shell(&self, other: &EnvironmentContext) -> bool {
        let EnvironmentContext {
            cwd,
            // should compare all fields except shell
            shell: _,
        } = other;

        self.cwd == *cwd
    }

    pub fn diff(before: &TurnContext, after: &TurnContext, shell: &Shell) -> Self {
        let cwd = if before.cwd != after.cwd {
            Some(after.cwd.clone())
        } else {
            None
        };
        EnvironmentContext::new(cwd, shell.clone())
    }

    pub fn from_turn_context(turn_context: &TurnContext, shell: &Shell) -> Self {
        Self::new(Some(turn_context.cwd.clone()), shell.clone())
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
            shell_snapshot: None,
        }
    }

    #[test]
    fn serialize_workspace_write_environment_context() {
        let cwd = test_path_buf("/repo");
        let context = EnvironmentContext::new(Some(cwd.clone()), fake_shell());

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
    fn serialize_read_only_environment_context() {
        let context = EnvironmentContext::new(None, fake_shell());

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_external_sandbox_environment_context() {
        let context = EnvironmentContext::new(None, fake_shell());

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_external_sandbox_with_restricted_network_environment_context() {
        let context = EnvironmentContext::new(None, fake_shell());

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_full_access_environment_context() {
        let context = EnvironmentContext::new(None, fake_shell());

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn equals_except_shell_compares_cwd() {
        let context1 = EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell());
        let context2 = EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell());
        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_ignores_sandbox_policy() {
        let context1 = EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell());
        let context2 = EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell());

        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_compares_cwd_differences() {
        let context1 = EnvironmentContext::new(Some(PathBuf::from("/repo1")), fake_shell());
        let context2 = EnvironmentContext::new(Some(PathBuf::from("/repo2")), fake_shell());

        assert!(!context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_ignores_shell() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Shell {
                shell_type: ShellType::Bash,
                shell_path: "/bin/bash".into(),
                shell_snapshot: None,
            },
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Shell {
                shell_type: ShellType::Zsh,
                shell_path: "/bin/zsh".into(),
                shell_snapshot: None,
            },
        );

        assert!(context1.equals_except_shell(&context2));
    }
}
