use chrono::Local;
use os_info::Type as OsType;
use os_info::Version;
use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display as DeriveDisplay;
use which::which;

use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use crate::shell::Shell;
use code_protocol::config_types::SandboxMode;
use code_protocol::models::ContentItem;
use code_protocol::models::ResponseItem;
use code_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use code_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, DeriveDisplay)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum NetworkAccess {
    Restricted,
    Enabled,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "environment_context", rename_all = "snake_case")]
pub(crate) struct EnvironmentContext {
    pub cwd: Option<PathBuf>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_mode: Option<SandboxMode>,
    pub network_access: Option<NetworkAccess>,
    pub writable_roots: Option<Vec<PathBuf>>,
    pub operating_system: Option<OperatingSystemInfo>,
    pub common_tools: Option<Vec<String>>,
    pub shell: Option<Shell>,
    pub current_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) struct OperatingSystemInfo {
    pub family: Option<String>,
    pub version: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ToolCandidate {
    pub label: &'static str,
    pub detection_names: &'static [&'static str],
}

pub const TOOL_CANDIDATES: &[ToolCandidate] = &[
    ToolCandidate { label: "git", detection_names: &["git"] },
    ToolCandidate { label: "gh", detection_names: &["gh"] },
    ToolCandidate { label: "rg", detection_names: &["rg"] },
    ToolCandidate { label: "fd", detection_names: &["fd", "fdfind"] },
    ToolCandidate { label: "fzf", detection_names: &["fzf"] },
    ToolCandidate { label: "jq", detection_names: &["jq"] },
    ToolCandidate { label: "yq", detection_names: &["yq"] },
    ToolCandidate { label: "sed", detection_names: &["sed"] },
    ToolCandidate { label: "awk", detection_names: &["awk"] },
    ToolCandidate { label: "xargs", detection_names: &["xargs"] },
    ToolCandidate { label: "parallel", detection_names: &["parallel"] },
    ToolCandidate { label: "curl", detection_names: &["curl"] },
    ToolCandidate { label: "wget", detection_names: &["wget"] },
    ToolCandidate { label: "tar", detection_names: &["tar"] },
    ToolCandidate { label: "unzip", detection_names: &["unzip"] },
    ToolCandidate { label: "gzip", detection_names: &["gzip"] },
    ToolCandidate { label: "zstd", detection_names: &["zstd"] },
    ToolCandidate { label: "make", detection_names: &["make"] },
    ToolCandidate { label: "just", detection_names: &["just"] },
    ToolCandidate { label: "node", detection_names: &["node"] },
    ToolCandidate { label: "npm", detection_names: &["npm"] },
    ToolCandidate { label: "pnpm", detection_names: &["pnpm"] },
    ToolCandidate { label: "python3", detection_names: &["python3"] },
    ToolCandidate { label: "pipx", detection_names: &["pipx"] },
    ToolCandidate { label: "go", detection_names: &["go"] },
    ToolCandidate { label: "rustup", detection_names: &["rustup"] },
    ToolCandidate { label: "cargo", detection_names: &["cargo"] },
    ToolCandidate { label: "rustc", detection_names: &["rustc"] },
    ToolCandidate { label: "shellcheck", detection_names: &["shellcheck"] },
    ToolCandidate { label: "shfmt", detection_names: &["shfmt"] },
    ToolCandidate { label: "docker", detection_names: &["docker"] },
    ToolCandidate { label: "docker compose", detection_names: &["docker", "docker-compose"] },
    ToolCandidate { label: "sqlite3", detection_names: &["sqlite3"] },
    ToolCandidate { label: "duckdb", detection_names: &["duckdb"] },
    ToolCandidate { label: "rsync", detection_names: &["rsync"] },
    ToolCandidate { label: "openssl", detection_names: &["openssl"] },
    ToolCandidate { label: "ssh", detection_names: &["ssh"] },
    ToolCandidate { label: "dig", detection_names: &["dig"] },
    ToolCandidate { label: "nc", detection_names: &["nc", "netcat"] },
    ToolCandidate { label: "lsof", detection_names: &["lsof"] },
    ToolCandidate { label: "ripgrep-all", detection_names: &["ripgrep-all", "rga"] },
    ToolCandidate { label: "entr", detection_names: &["entr"] },
    ToolCandidate { label: "watchexec", detection_names: &["watchexec"] },
    ToolCandidate { label: "hyperfine", detection_names: &["hyperfine"] },
    ToolCandidate { label: "pv", detection_names: &["pv"] },
    ToolCandidate { label: "bat", detection_names: &["bat"] },
    ToolCandidate { label: "delta", detection_names: &["delta"] },
    ToolCandidate { label: "tree", detection_names: &["tree"] },
    ToolCandidate { label: "python", detection_names: &["python"] },
    ToolCandidate { label: "deno", detection_names: &["deno"] },
    ToolCandidate { label: "bun", detection_names: &["bun"] },
    ToolCandidate { label: "js", detection_names: &["js"] },
];

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
                        Some(writable_roots.clone())
                    }
                }
                _ => None,
            },
            operating_system: detect_operating_system_info(),
            common_tools: detect_common_tools(),
            shell,
            current_date: Some(Local::now().format("%Y-%m-%d").to_string()),
        }
    }

    /// Compares two environment contexts, ignoring the shell. Useful when
    /// comparing turn to turn, since the initial environment_context will
    /// include the shell, and then it is not configurable from turn to turn.
    #[cfg(test)]
    pub fn equals_except_shell(&self, other: &EnvironmentContext) -> bool {
        let EnvironmentContext {
            cwd,
            approval_policy,
            sandbox_mode,
            network_access,
            writable_roots,
            operating_system,
            common_tools,
            current_date,
            // should compare all fields except shell
            shell: _,
        } = other;

        self.cwd == *cwd
            && self.approval_policy == *approval_policy
            && self.sandbox_mode == *sandbox_mode
            && self.network_access == *network_access
            && self.writable_roots == *writable_roots
            && self.operating_system == *operating_system
            && self.common_tools == *common_tools
            && self.current_date == *current_date
    }
}

// Note: The core no longer exposes `TurnContext` here; callers construct
// `EnvironmentContext` directly via `EnvironmentContext::new(...)`.

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
    ///   <operating_system>
    ///     <family>...</family>
    ///     <version>...</version>
    ///     <architecture>...</architecture>
    ///   </operating_system>
    ///   <common_tools>...</common_tools>
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
        if let Some(operating_system) = self.operating_system {
            lines.push("  <operating_system>".to_string());
            if let Some(family) = operating_system.family {
                lines.push(format!("    <family>{family}</family>"));
            }
            if let Some(version) = operating_system.version {
                lines.push(format!("    <version>{version}</version>"));
            }
            if let Some(architecture) = operating_system.architecture {
                lines.push(format!("    <architecture>{architecture}</architecture>"));
            }
            lines.push("  </operating_system>".to_string());
        }
        if let Some(common_tools) = self.common_tools {
            if !common_tools.is_empty() {
                lines.push("  <common_tools>".to_string());
                for tool in common_tools {
                    lines.push(format!("    <tool>{tool}</tool>"));
                }
                lines.push("  </common_tools>".to_string());
            }
        }
        if let Some(current_date) = self.current_date {
            lines.push(format!("  <current_date>{current_date}</current_date>"));
        }
        if let Some(shell) = self.shell
            && let Some(shell_name) = shell.name()
        {
            lines.push(format!("  <shell>{shell_name}</shell>"));
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
        }
    }
}

fn detect_operating_system_info() -> Option<OperatingSystemInfo> {
    let info = os_info::get();
    let family = match info.os_type() {
        OsType::Unknown => None,
        other => Some(other.to_string()),
    };
    let version = match info.version() {
        Version::Unknown => None,
        other => {
            let text = other.to_string();
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        }
    };
    let architecture = {
        let arch = std::env::consts::ARCH;
        if arch.is_empty() {
            None
        } else {
            Some(arch.to_string())
        }
    };

    if family.is_none() && version.is_none() && architecture.is_none() {
        return None;
    }

    Some(OperatingSystemInfo {
        family,
        version,
        architecture,
    })
}

fn detect_common_tools() -> Option<Vec<String>> {
    let mut available = Vec::new();
    for candidate in TOOL_CANDIDATES {
        let detection_names = if candidate.detection_names.is_empty() {
            &[candidate.label][..]
        } else {
            candidate.detection_names
        };

        if detection_names
            .iter()
            .any(|name| which(name).is_ok())
        {
            available.push(candidate.label.to_string());
        }
    }

    if available.is_empty() {
        None
    } else {
        Some(available)
    }
}

#[cfg(test)]
mod tests {
    use crate::shell::BashShell;
    use crate::shell::ZshShell;

    use super::*;
    use pretty_assertions::assert_eq;

    fn workspace_write_policy(writable_roots: Vec<&str>, network_access: bool) -> SandboxPolicy {
        SandboxPolicy::WorkspaceWrite {
            writable_roots: writable_roots.into_iter().map(PathBuf::from).collect(),
            network_access,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
            allow_git_writes: true,
        }
    }

    #[test]
    fn serialize_workspace_write_environment_context() {
        let mut context = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(vec!["/repo", "/tmp"], false)),
            None,
        );
        context.operating_system = None;
        context.common_tools = None;
        context.current_date = Some("2025-01-02".to_string());

        let expected = r#"<environment_context>
  <cwd>/repo</cwd>
  <approval_policy>on-request</approval_policy>
  <sandbox_mode>workspace-write</sandbox_mode>
  <network_access>restricted</network_access>
  <writable_roots>
    <root>/repo</root>
    <root>/tmp</root>
  </writable_roots>
  <current_date>2025-01-02</current_date>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_read_only_environment_context() {
        let mut context = EnvironmentContext::new(
            None,
            Some(AskForApproval::Never),
            Some(SandboxPolicy::ReadOnly),
            None,
        );
        context.operating_system = None;
        context.common_tools = None;
        context.current_date = Some("2025-01-02".to_string());

        let expected = r#"<environment_context>
  <approval_policy>never</approval_policy>
  <sandbox_mode>read-only</sandbox_mode>
  <network_access>restricted</network_access>
  <current_date>2025-01-02</current_date>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_full_access_environment_context() {
        let mut context = EnvironmentContext::new(
            None,
            Some(AskForApproval::OnFailure),
            Some(SandboxPolicy::DangerFullAccess),
            None,
        );
        context.operating_system = None;
        context.common_tools = None;
        context.current_date = Some("2025-01-02".to_string());

        let expected = r#"<environment_context>
  <approval_policy>on-failure</approval_policy>
  <sandbox_mode>danger-full-access</sandbox_mode>
  <network_access>enabled</network_access>
  <current_date>2025-01-02</current_date>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_environment_context_includes_os_and_tools() {
        let mut context = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Some(AskForApproval::OnRequest),
            Some(workspace_write_policy(vec!["/repo"], true)),
            None,
        );
        context.operating_system = Some(OperatingSystemInfo {
            family: Some("macos".to_string()),
            version: Some("14.0".to_string()),
            architecture: Some("aarch64".to_string()),
        });
        context.common_tools = Some(vec!["rg".to_string(), "git".to_string()]);
        context.current_date = Some("2025-01-02".to_string());

        let xml = context.serialize_to_xml();
        assert!(xml.contains("<operating_system>"));
        assert!(xml.contains("<family>macos</family>"));
        assert!(xml.contains("<version>14.0</version>"));
        assert!(xml.contains("<architecture>aarch64</architecture>"));
        assert!(xml.contains("<common_tools>"));
        assert!(xml.contains("<tool>rg</tool>"));
        assert!(xml.contains("<tool>git</tool>"));
        assert!(xml.contains("<current_date>2025-01-02</current_date>"));
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
        // ensure current_date doesn't influence this comparison
        let fixed_date = Some("2025-01-02".to_string());
        let mut context1 = context1;
        context1.current_date = fixed_date.clone();
        let mut context2 = context2;
        context2.current_date = fixed_date;
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
        let mut context1 = context1;
        context1.current_date = Some("2025-01-02".to_string());
        let mut context2 = context2;
        context2.current_date = Some("2025-01-02".to_string());

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
        let mut context1 = context1;
        context1.current_date = Some("2025-01-02".to_string());
        let mut context2 = context2;
        context2.current_date = Some("2025-01-02".to_string());

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
        let mut context1 = context1;
        context1.current_date = Some("2025-01-02".to_string());
        let mut context2 = context2;
        context2.current_date = Some("2025-01-02".to_string());

        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn serialize_environment_context_includes_current_date() {
        let mut context = EnvironmentContext::new(None, None, None, None);
        context.current_date = Some("2025-01-02".to_string());

        let xml = context.serialize_to_xml();
        assert!(xml.contains("<current_date>2025-01-02</current_date>"));
    }

    #[test]
    fn current_date_format_is_iso8601() {
        let context = EnvironmentContext::new(None, None, None, None);
        let date = context
            .current_date
            .expect("current_date should be populated");
        assert_eq!(date.len(), 10);
        assert_eq!(date.chars().nth(4), Some('-'));
        assert_eq!(date.chars().nth(7), Some('-'));
    }
}
