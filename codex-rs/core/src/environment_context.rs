use crate::codex::TurnContext;
use crate::protocol::SandboxPolicy;
use crate::shell::Shell;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "environment_context", rename_all = "snake_case")]
pub(crate) struct EnvironmentContext {
    pub cwd: Option<PathBuf>,
    pub writable_roots: Option<Vec<AbsolutePathBuf>>,
    pub shell: Shell,
}

impl EnvironmentContext {
    pub fn new(
        cwd: Option<PathBuf>,
        sandbox_policy: Option<SandboxPolicy>,
        shell: Shell,
    ) -> Self {
        Self {
            cwd,
            writable_roots: match sandbox_policy {
                Some(SandboxPolicy::WorkspaceWrite { writable_roots, .. }) => {
                    if writable_roots.is_empty() {
                        None
                    } else {
                        Some(writable_roots)
                    }
                }
                _ => None,
            },
            shell,
        }
    }

    /// Compares two environment contexts, ignoring the shell. Useful when
    /// comparing turn to turn, since the initial environment_context will
    /// include the shell, and then it is not configurable from turn to turn.
    pub fn equals_except_shell(&self, other: &EnvironmentContext) -> bool {
        let EnvironmentContext {
            cwd,
            writable_roots,
            // should compare all fields except shell
            shell: _,
        } = other;

        self.cwd == *cwd
            && self.writable_roots == *writable_roots
    }

    pub fn diff(before: &TurnContext, after: &TurnContext, shell: &Shell) -> Self {
        let cwd = if before.cwd != after.cwd {
            Some(after.cwd.clone())
        } else {
            None
        };
        let sandbox_policy = if before.sandbox_policy != after.sandbox_policy {
            Some(after.sandbox_policy.clone())
        } else {
            None
        };
        EnvironmentContext::new(cwd, sandbox_policy, shell.clone())
    }

    pub fn from_turn_context(turn_context: &TurnContext, shell: &Shell) -> Self {
        Self::new(
            Some(turn_context.cwd.clone()),
            Some(turn_context.sandbox_policy.clone()),
            shell.clone(),
        )
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
    ///   <writable_roots>...</writable_roots>
    ///   <shell>...</shell>
    /// </environment_context>
    /// ```
    pub fn serialize_to_xml(self) -> String {
        let mut lines = vec![ENVIRONMENT_CONTEXT_OPEN_TAG.to_string()];
        if let Some(cwd) = self.cwd {
            lines.push(format!("  <cwd>{}</cwd>", cwd.to_string_lossy()));
        }
        if let Some(writable_roots) = self.writable_roots {
            lines.push("  <writable_roots>".to_string());
            for writable_root in writable_roots {
                lines.push(format!(
                    "    <root>{}</root>",
                    writable_root.to_string_lossy()
                ));
            }
            lines.push("  </writable_roots>".to_string());
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
    use crate::protocol::NetworkAccess;

    use super::*;
    use core_test_support::test_path_buf;
    use core_test_support::test_tmp_path_buf;
    use pretty_assertions::assert_eq;

    fn fake_shell() -> Shell {
        Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_snapshot: None,
        }
    }

    fn workspace_write_policy(writable_roots: Vec<&str>, network_access: bool) -> SandboxPolicy {
        SandboxPolicy::WorkspaceWrite {
            writable_roots: writable_roots
                .into_iter()
                .map(|s| AbsolutePathBuf::try_from(s).unwrap())
                .collect(),
            network_access,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }
    }

    #[test]
    fn serialize_workspace_write_environment_context() {
        let cwd = test_path_buf("/repo");
        let writable_root = test_tmp_path_buf();
        let cwd_str = cwd.to_str().expect("cwd is valid utf-8");
        let writable_root_str = writable_root
            .to_str()
            .expect("writable root is valid utf-8");
        let context = EnvironmentContext::new(
            Some(cwd.clone()),
            Some(workspace_write_policy(
                vec![cwd_str, writable_root_str],
                false,
            )),
            fake_shell(),
        );

        let expected = format!(
            r#"<environment_context>
  <cwd>{cwd}</cwd>
  <writable_roots>
    <root>{cwd}</root>
    <root>{writable_root}</root>
  </writable_roots>
  <shell>bash</shell>
</environment_context>"#,
            cwd = cwd.display(),
            writable_root = writable_root.display(),
        );

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_read_only_environment_context() {
        let context = EnvironmentContext::new(
            None,
            Some(SandboxPolicy::ReadOnly),
            fake_shell(),
        );

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_external_sandbox_environment_context() {
        let context = EnvironmentContext::new(
            None,
            Some(SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Enabled,
            }),
            fake_shell(),
        );

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_external_sandbox_with_restricted_network_environment_context() {
        let context = EnvironmentContext::new(
            None,
            Some(SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted,
            }),
            fake_shell(),
        );

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_full_access_environment_context() {
        let context = EnvironmentContext::new(
            None,
            Some(SandboxPolicy::DangerFullAccess),
            fake_shell(),
        );

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn equals_except_shell_compares_approval_policy() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(workspace_write_policy(vec!["/repo"], false)),
            fake_shell(),
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(workspace_write_policy(vec!["/repo"], true)),
            fake_shell(),
        );
        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_compares_sandbox_policy() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(SandboxPolicy::new_read_only_policy()),
            fake_shell(),
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(SandboxPolicy::new_workspace_write_policy()),
            fake_shell(),
        );

        assert!(!context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_compares_workspace_write_policy() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(workspace_write_policy(vec!["/repo", "/tmp", "/var"], false)),
            fake_shell(),
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(workspace_write_policy(vec!["/repo", "/tmp"], true)),
            fake_shell(),
        );

        assert!(!context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_ignores_shell() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(workspace_write_policy(vec!["/repo"], false)),
            Shell {
                shell_type: ShellType::Bash,
                shell_path: "/bin/bash".into(),
                shell_snapshot: None,
            },
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(workspace_write_policy(vec!["/repo"], false)),
            Shell {
                shell_type: ShellType::Zsh,
                shell_path: "/bin/zsh".into(),
                shell_snapshot: None,
            },
        );

        assert!(context1.equals_except_shell(&context2));
    }
}
