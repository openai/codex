use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;

use crate::codex::TurnContext;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use crate::shell::Shell;
use codex_protocol::config_types::NetworkAccess;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "environment_context", rename_all = "snake_case")]
pub(crate) struct EnvironmentContext {
    pub cwd: Option<PathBuf>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_mode: Option<SandboxMode>,
    pub network_access: Option<NetworkAccess>,
    pub writable_roots: Option<Vec<AbsolutePathBuf>>,
    pub shell: Shell,
}

impl EnvironmentContext {
    pub fn new(
        cwd: Option<PathBuf>,
        approval_policy: Option<AskForApproval>,
        sandbox_policy: Option<SandboxPolicy>,
        network_access: Option<NetworkAccess>,
        shell: Shell,
    ) -> Self {
        let sandbox_mode = sandbox_policy.as_ref().map(|policy| match policy {
            SandboxPolicy::DangerFullAccess => SandboxMode::DangerFullAccess,
            SandboxPolicy::ReadOnly => SandboxMode::ReadOnly,
            SandboxPolicy::WorkspaceWrite { .. } => SandboxMode::WorkspaceWrite,
        });
        let resolved_network_access =
            Self::resolve_network_access(sandbox_policy.as_ref(), network_access);
        Self {
            cwd,
            approval_policy,
            sandbox_mode,
            network_access: resolved_network_access,
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

    fn resolve_network_access(
        sandbox_policy: Option<&SandboxPolicy>,
        network_access_override: Option<NetworkAccess>,
    ) -> Option<NetworkAccess> {
        match network_access_override {
            Some(access) => Some(access),
            None => match sandbox_policy {
                Some(SandboxPolicy::DangerFullAccess) => Some(NetworkAccess::Enabled),
                Some(SandboxPolicy::ReadOnly) => Some(NetworkAccess::Restricted),
                Some(SandboxPolicy::WorkspaceWrite { network_access, .. }) => {
                    if *network_access {
                        Some(NetworkAccess::Enabled)
                    } else {
                        Some(NetworkAccess::Restricted)
                    }
                }
                None => None,
            },
        }
    }

    /// Compares two environment contexts, ignoring the shell. Useful when
    /// comparing turn to turn, since the initial environment_context will
    /// include the shell, and then it is not configurable from turn to turn.
    pub fn equals_except_shell(&self, other: &EnvironmentContext) -> bool {
        let EnvironmentContext {
            cwd,
            approval_policy,
            sandbox_mode,
            network_access,
            writable_roots,
            // should compare all fields except shell
            shell: _,
        } = other;

        self.cwd == *cwd
            && self.approval_policy == *approval_policy
            && self.sandbox_mode == *sandbox_mode
            && self.network_access == *network_access
            && self.writable_roots == *writable_roots
    }

    pub fn diff(before: &TurnContext, after: &TurnContext, shell: &Shell) -> Self {
        let cwd = if before.cwd != after.cwd {
            Some(after.cwd.clone())
        } else {
            None
        };
        let approval_policy = if before.approval_policy != after.approval_policy {
            Some(after.approval_policy)
        } else {
            None
        };
        let sandbox_policy = if before.sandbox_policy != after.sandbox_policy {
            Some(after.sandbox_policy.clone())
        } else {
            None
        };
        let before_network_access = EnvironmentContext::resolve_network_access(
            Some(&before.sandbox_policy),
            before.network_access,
        );
        let after_network_access = EnvironmentContext::resolve_network_access(
            Some(&after.sandbox_policy),
            after.network_access,
        );
        let network_access = if before_network_access != after_network_access {
            after_network_access
        } else {
            None
        };
        EnvironmentContext::new(
            cwd,
            approval_policy,
            sandbox_policy,
            network_access,
            shell.clone(),
        )
    }

    pub fn from_turn_context(turn_context: &TurnContext, shell: &Shell) -> Self {
        Self::new(
            Some(turn_context.cwd.clone()),
            Some(turn_context.approval_policy),
            Some(turn_context.sandbox_policy.clone()),
            turn_context.network_access,
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
    ///   <approval_policy>...</approval_policy>
    ///   <sandbox_mode>...</sandbox_mode>
    ///   <writable_roots>...</writable_roots>
    ///   <network_access>...</network_access>
    ///   <shell>...</shell>
    /// </environment_context>
    /// ```
    pub fn serialize_to_xml(self) -> String {
        let mut lines = vec![ENVIRONMENT_CONTEXT_OPEN_TAG.to_string()];
        if let Some(cwd) = self.cwd {
            lines.push(format!("  <cwd>{}</cwd>", cwd.to_string_lossy()));
        }
        if let Some(approval_policy) = self.approval_policy {
            lines.push(format!(
                "  <approval_policy>{approval_policy}</approval_policy>"
            ));
        }
        if let Some(sandbox_mode) = self.sandbox_mode {
            lines.push(format!("  <sandbox_mode>{sandbox_mode}</sandbox_mode>"));
        }
        if let Some(network_access) = self.network_access {
            lines.push(format!(
                "  <network_access>{network_access}</network_access>"
            ));
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

    use super::*;
    use crate::codex::make_session_and_context;
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
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(
                vec![cwd_str, writable_root_str],
                false,
            )),
            None,
            fake_shell(),
        );

        let expected = format!(
            r#"<environment_context>
  <cwd>{cwd}</cwd>
  <approval_policy>on-request</approval_policy>
  <sandbox_mode>workspace-write</sandbox_mode>
  <network_access>restricted</network_access>
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
    fn serialize_workspace_write_environment_context_with_network_override() {
        let cwd = test_path_buf("/repo");
        let writable_root = test_tmp_path_buf();
        let cwd_str = cwd.to_str().expect("cwd is valid utf-8");
        let writable_root_str = writable_root
            .to_str()
            .expect("writable root is valid utf-8");
        let context = EnvironmentContext::new(
            Some(cwd.clone()),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(
                vec![cwd_str, writable_root_str],
                false,
            )),
            Some(NetworkAccess::Enabled),
            fake_shell(),
        );

        let expected = format!(
            r#"<environment_context>
  <cwd>{cwd}</cwd>
  <approval_policy>on-request</approval_policy>
  <sandbox_mode>workspace-write</sandbox_mode>
  <network_access>enabled</network_access>
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
            Some(AskForApproval::Never),
            Some(SandboxPolicy::ReadOnly),
            None,
            fake_shell(),
        );

        let expected = r#"<environment_context>
  <approval_policy>never</approval_policy>
  <sandbox_mode>read-only</sandbox_mode>
  <network_access>restricted</network_access>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_full_access_environment_context() {
        let context = EnvironmentContext::new(
            None,
            Some(AskForApproval::OnFailure),
            Some(SandboxPolicy::DangerFullAccess),
            None,
            fake_shell(),
        );

        let expected = r#"<environment_context>
  <approval_policy>on-failure</approval_policy>
  <sandbox_mode>danger-full-access</sandbox_mode>
  <network_access>enabled</network_access>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_full_access_environment_context_with_override() {
        let context = EnvironmentContext::new(
            None,
            Some(AskForApproval::OnFailure),
            Some(SandboxPolicy::DangerFullAccess),
            Some(NetworkAccess::Restricted),
            fake_shell(),
        );

        let expected = r#"<environment_context>
  <approval_policy>on-failure</approval_policy>
  <sandbox_mode>danger-full-access</sandbox_mode>
  <network_access>restricted</network_access>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn diff_detects_network_override_changes() {
        let (_, mut before) = make_session_and_context();
        let (_, mut after) = make_session_and_context();
        before.sandbox_policy = SandboxPolicy::ReadOnly;
        after.sandbox_policy = SandboxPolicy::ReadOnly;
        before.network_access = None;
        after.network_access = Some(NetworkAccess::Enabled);

        let shell = fake_shell();
        let diff = EnvironmentContext::diff(&before, &after, &shell);
        let expected =
            EnvironmentContext::new(None, None, None, Some(NetworkAccess::Enabled), shell);

        assert_eq!(diff, expected);
    }

    #[test]
    fn equals_except_shell_compares_approval_policy() {
        // Approval policy
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(vec!["/repo"], false)),
            None,
            fake_shell(),
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::Never),
            Some(workspace_write_policy(vec!["/repo"], true)),
            None,
            fake_shell(),
        );
        assert!(!context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_compares_sandbox_policy() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(SandboxPolicy::new_read_only_policy()),
            None,
            fake_shell(),
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(SandboxPolicy::new_workspace_write_policy()),
            None,
            fake_shell(),
        );

        assert!(!context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_compares_workspace_write_policy() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(vec!["/repo", "/tmp", "/var"], false)),
            None,
            fake_shell(),
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(vec!["/repo", "/tmp"], true)),
            None,
            fake_shell(),
        );

        assert!(!context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_ignores_shell() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(vec!["/repo"], false)),
            None,
            Shell {
                shell_type: ShellType::Bash,
                shell_path: "/bin/bash".into(),
                shell_snapshot: None,
            },
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(vec!["/repo"], false)),
            None,
            Shell {
                shell_type: ShellType::Zsh,
                shell_path: "/bin/zsh".into(),
                shell_snapshot: None,
            },
        );

        assert!(context1.equals_except_shell(&context2));
    }
}
