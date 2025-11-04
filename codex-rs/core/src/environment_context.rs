use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display as DeriveDisplay;

use crate::codex::TurnContext;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use crate::shell::Shell;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, DeriveDisplay)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum NetworkAccess {
    Restricted,
    Enabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OperatingSystemInfo {
    pub name: String,
    pub version: String,
    pub is_likely_windows_subsystem_for_linux: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "environment_context", rename_all = "snake_case")]
pub(crate) struct EnvironmentContext {
    pub cwd: Option<PathBuf>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_mode: Option<SandboxMode>,
    pub network_access: Option<NetworkAccess>,
    pub writable_roots: Option<Vec<PathBuf>>,
    pub shell: Option<Shell>,
    pub operating_system: Option<OperatingSystemInfo>,
}

impl EnvironmentContext {
    pub fn new(
        cwd: Option<PathBuf>,
        approval_policy: Option<AskForApproval>,
        sandbox_policy: Option<SandboxPolicy>,
        shell: Option<Shell>,
    ) -> Self {
        Self {
            cwd,
            approval_policy,
            sandbox_mode: match sandbox_policy {
                Some(SandboxPolicy::DangerFullAccess) => Some(SandboxMode::DangerFullAccess),
                Some(SandboxPolicy::ReadOnly) => Some(SandboxMode::ReadOnly),
                Some(SandboxPolicy::WorkspaceWrite { .. }) => Some(SandboxMode::WorkspaceWrite),
                None => None,
            },
            network_access: match sandbox_policy {
                Some(SandboxPolicy::DangerFullAccess) => Some(NetworkAccess::Enabled),
                Some(SandboxPolicy::ReadOnly) => Some(NetworkAccess::Restricted),
                Some(SandboxPolicy::WorkspaceWrite { network_access, .. }) => {
                    if network_access {
                        Some(NetworkAccess::Enabled)
                    } else {
                        Some(NetworkAccess::Restricted)
                    }
                }
                None => None,
            },
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
            operating_system: Self::operating_system_info(),
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
            operating_system,
            // should compare all fields except shell
            shell: _,
        } = other;

        self.cwd == *cwd
            && self.approval_policy == *approval_policy
            && self.sandbox_mode == *sandbox_mode
            && self.network_access == *network_access
            && self.writable_roots == *writable_roots
            && self.operating_system == *operating_system
    }

    pub fn diff(before: &TurnContext, after: &TurnContext) -> Self {
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
        // Diff messages should only include fields that changed between turns.
        // Operating system is a static property of the host and should not be
        // emitted as part of a per-turn diff.
        let mut ec = EnvironmentContext::new(cwd, approval_policy, sandbox_policy, None);
        ec.operating_system = None;
        ec
    }
}

impl From<&TurnContext> for EnvironmentContext {
    fn from(turn_context: &TurnContext) -> Self {
        Self::new(
            Some(turn_context.cwd.clone()),
            Some(turn_context.approval_policy),
            Some(turn_context.sandbox_policy.clone()),
            // Shell is not configurable from turn to turn
            None,
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
        if let Some(shell) = self.shell
            && let Some(shell_name) = shell.name()
        {
            lines.push(format!("  <shell>{shell_name}</shell>"));
        }
        if let Some(operating_system) = self.operating_system {
            lines.push("  <operating_system>".to_string());
            lines.push(format!("    <name>{}</name>", operating_system.name));
            lines.push(format!(
                "    <version>{}</version>",
                operating_system.version
            ));
            if let Some(is_wsl) = operating_system.is_likely_windows_subsystem_for_linux {
                lines.push(format!(
                    "    <is_likely_windows_subsystem_for_linux>{is_wsl}</is_likely_windows_subsystem_for_linux>"
                ));
            }
            lines.push("  </operating_system>".to_string());
        }
        lines.push(ENVIRONMENT_CONTEXT_CLOSE_TAG.to_string());
        lines.join("\n")
    }

    fn operating_system_info() -> Option<OperatingSystemInfo> {
        operating_system_info_impl()
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

// Restrict Operating System Info to Windows and Linux inside WSL for now
#[cfg(target_os = "windows")]
fn operating_system_info_impl() -> Option<OperatingSystemInfo> {
    let os_info = os_info::get();
    Some(OperatingSystemInfo {
        name: os_info.os_type().to_string(),
        version: os_info.version().to_string(),
        is_likely_windows_subsystem_for_linux: Some(has_wsl_env_markers()),
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn operating_system_info_impl() -> Option<OperatingSystemInfo> {
    match has_wsl_env_markers() {
        true => Some(OperatingSystemInfo {
            name: "Windows Subsystem for Linux".to_string(),
            version: "".to_string(),
            is_likely_windows_subsystem_for_linux: Some(true),
        }),
        false => None,
    }
}

#[cfg(target_os = "macos")]
fn operating_system_info_impl() -> Option<OperatingSystemInfo> {
    None
}

#[cfg(not(target_os = "macos"))]
fn has_wsl_env_markers() -> bool {
    std::env::var_os("WSL_INTEROP").is_some()
        || std::env::var_os("WSLENV").is_some()
        || std::env::var_os("WSL_DISTRO_NAME").is_some()
}

#[cfg(test)]
mod tests {
    use crate::shell::BashShell;
    use crate::shell::ZshShell;

    use super::*;
    use pretty_assertions::assert_eq;
    fn expected_environment_context(mut body_lines: Vec<String>) -> String {
        let mut lines = vec!["<environment_context>".to_string()];
        lines.append(&mut body_lines);
        if let Some(os) = EnvironmentContext::operating_system_info() {
            lines.push("  <operating_system>".to_string());
            lines.push(format!("    <name>{}</name>", os.name));
            lines.push(format!("    <version>{}</version>", os.version));
            if let Some(is_wsl) = os.is_likely_windows_subsystem_for_linux {
                lines.push(format!(
                    "    <is_likely_windows_subsystem_for_linux>{is_wsl}</is_likely_windows_subsystem_for_linux>"
                ));
            }
            lines.push("  </operating_system>".to_string());
        }
        lines.push("</environment_context>".to_string());
        lines.join("\n")
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn operating_system_info_on_windows_includes_os_details() {
        let info = operating_system_info_impl().expect("expected Windows operating system info");
        let os_details = os_info::get();

        assert_eq!(info.name, os_details.os_type().to_string());
        assert_eq!(info.version, os_details.version().to_string());
        assert_eq!(
            info.is_likely_windows_subsystem_for_linux,
            Some(has_wsl_env_markers())
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn operating_system_info_matches_wsl_detection_on_unix() {
        let info = operating_system_info_impl();
        if has_wsl_env_markers() {
            let info = info.expect("expected WSL operating system info");
            assert_eq!(info.name, "Windows Subsystem for Linux");
            assert_eq!(info.version, "");
            assert_eq!(info.is_likely_windows_subsystem_for_linux, Some(true));
        } else {
            assert_eq!(info, None);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn operating_system_info_is_none_on_macos() {
        assert_eq!(operating_system_info_impl(), None);
    }

    fn workspace_write_policy(writable_roots: Vec<&str>, network_access: bool) -> SandboxPolicy {
        SandboxPolicy::WorkspaceWrite {
            writable_roots: writable_roots.into_iter().map(PathBuf::from).collect(),
            network_access,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }
    }

    #[test]
    fn serialize_workspace_write_environment_context() {
        let context = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(vec!["/repo", "/tmp"], false)),
            None,
        );

        let expected = expected_environment_context(vec![
            "  <cwd>/repo</cwd>".to_string(),
            "  <approval_policy>on-request</approval_policy>".to_string(),
            "  <sandbox_mode>workspace-write</sandbox_mode>".to_string(),
            "  <network_access>restricted</network_access>".to_string(),
            "  <writable_roots>".to_string(),
            "    <root>/repo</root>".to_string(),
            "    <root>/tmp</root>".to_string(),
            "  </writable_roots>".to_string(),
        ]);

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_read_only_environment_context() {
        let context = EnvironmentContext::new(
            None,
            Some(AskForApproval::Never),
            Some(SandboxPolicy::ReadOnly),
            None,
        );

        let expected = expected_environment_context(vec![
            "  <approval_policy>never</approval_policy>".to_string(),
            "  <sandbox_mode>read-only</sandbox_mode>".to_string(),
            "  <network_access>restricted</network_access>".to_string(),
        ]);

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_full_access_environment_context() {
        let context = EnvironmentContext::new(
            None,
            Some(AskForApproval::OnFailure),
            Some(SandboxPolicy::DangerFullAccess),
            None,
        );

        let expected = expected_environment_context(vec![
            "  <approval_policy>on-failure</approval_policy>".to_string(),
            "  <sandbox_mode>danger-full-access</sandbox_mode>".to_string(),
            "  <network_access>enabled</network_access>".to_string(),
        ]);

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn equals_except_shell_compares_approval_policy() {
        // Approval policy
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(vec!["/repo"], false)),
            None,
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::Never),
            Some(workspace_write_policy(vec!["/repo"], true)),
            None,
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
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(SandboxPolicy::new_workspace_write_policy()),
            None,
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
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(vec!["/repo", "/tmp"], true)),
            None,
        );

        assert!(!context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_ignores_shell() {
        let context1 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(vec!["/repo"], false)),
            Some(Shell::Bash(BashShell {
                shell_path: "/bin/bash".into(),
                bashrc_path: "/home/user/.bashrc".into(),
            })),
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(vec!["/repo"], false)),
            Some(Shell::Zsh(ZshShell {
                shell_path: "/bin/zsh".into(),
                zshrc_path: "/home/user/.zshrc".into(),
            })),
        );

        assert!(context1.equals_except_shell(&context2));
    }
}
