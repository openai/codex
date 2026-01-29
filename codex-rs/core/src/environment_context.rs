use crate::codex::TurnContext;
use crate::shell::Shell;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "environment_context", rename_all = "snake_case")]
pub(crate) struct EnvironmentContext {
    pub cwd: Option<PathBuf>,
    pub shell: Shell,
    /// Workspace metadata, structured to support future multi-root workspaces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_configuration: Option<WorkspaceConfiguration>,
}

impl EnvironmentContext {
    pub fn new(
        cwd: Option<PathBuf>,
        shell: Shell,
        workspace_configuration: Option<WorkspaceConfiguration>,
    ) -> Self {
        Self {
            cwd,
            shell,
            workspace_configuration,
        }
    }

    /// Compares two environment contexts, ignoring the shell. Useful when
    /// comparing turn to turn, since the initial environment_context will
    /// include the shell, and then it is not configurable from turn to turn.
    pub fn equals_except_shell(&self, other: &EnvironmentContext) -> bool {
        let EnvironmentContext {
            cwd,
            // should compare all fields except shell
            shell: _,
            ..
        } = other;

        self.cwd == *cwd
    }

    pub fn diff(before: &TurnContext, after: &TurnContext, shell: &Shell) -> Self {
        let cwd = if before.cwd != after.cwd {
            Some(after.cwd.clone())
        } else {
            None
        };
        // Only include workspace configuration on the initial prefix message.
        EnvironmentContext::new(cwd, shell.clone(), None)
    }

    pub fn from_turn_context(turn_context: &TurnContext, shell: &Shell) -> Self {
        // Only include workspace configuration on the initial prefix message.
        Self::new(Some(turn_context.cwd.clone()), shell.clone(), None)
    }
}

/// Multi-root-friendly workspace metadata modeled after Cline's structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct WorkspaceConfiguration {
    pub workspaces: BTreeMap<String, WorkspaceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct WorkspaceEntry {
    pub hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_remote_urls: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_git_commit_hash: Option<String>,
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

        if let Some(workspace_configuration) = self.workspace_configuration {
            lines.push("  <workspace_configuration>".to_string());
            for (path, workspace) in workspace_configuration.workspaces {
                lines.push(format!(
                    "    <workspace path=\"{path}\" hint=\"{}\">",
                    workspace.hint
                ));

                if let Some(latest_git_commit_hash) = workspace.latest_git_commit_hash {
                    lines.push(format!(
                        "      <latest_git_commit_hash>{latest_git_commit_hash}</latest_git_commit_hash>"
                    ));
                }

                if let Some(associated_remote_urls) = workspace.associated_remote_urls
                    && !associated_remote_urls.is_empty()
                {
                    lines.push("      <associated_remote_urls>".to_string());
                    for (name, url) in associated_remote_urls {
                        lines.push(format!("        <remote name=\"{name}\">{url}</remote>"));
                    }
                    lines.push("      </associated_remote_urls>".to_string());
                }

                lines.push("    </workspace>".to_string());
            }
            lines.push("  </workspace_configuration>".to_string());
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
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::shell::ShellType;

    use super::*;
    use core_test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;

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

    #[test]
    fn serialize_environment_context_with_workspace_configuration() {
        let cwd = test_path_buf("/repo");
        let mut workspaces = BTreeMap::new();
        workspaces.insert(
            cwd.to_string_lossy().to_string(),
            WorkspaceEntry {
                hint: "repo".to_string(),
                associated_remote_urls: Some(BTreeMap::from([(
                    "origin".to_string(),
                    "https://example.com/repo.git".to_string(),
                )])),
                latest_git_commit_hash: Some("abc123".to_string()),
            },
        );
        let workspace_configuration = WorkspaceConfiguration { workspaces };
        let context = EnvironmentContext::new(
            Some(cwd.clone()),
            fake_shell(),
            Some(workspace_configuration),
        );

        let expected = r#"<environment_context>
  <cwd>/repo</cwd>
  <shell>bash</shell>
  <workspace_configuration>
    <workspace path="/repo" hint="repo">
      <latest_git_commit_hash>abc123</latest_git_commit_hash>
      <associated_remote_urls>
        <remote name="origin">https://example.com/repo.git</remote>
      </associated_remote_urls>
    </workspace>
  </workspace_configuration>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }
}
