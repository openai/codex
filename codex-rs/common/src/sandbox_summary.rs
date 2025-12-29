use codex_core::protocol::NetworkAccess;
use codex_core::protocol::SandboxPolicy;

pub fn summarize_sandbox_policy(sandbox_policy: &SandboxPolicy) -> String {
    match sandbox_policy {
        SandboxPolicy::DangerFullAccess => "danger-full-access".to_string(),
        SandboxPolicy::ReadOnly => "read-only".to_string(),
        SandboxPolicy::ExternalSandbox { network_access } => {
            let mut summary = "external-sandbox".to_string();
            if matches!(network_access, NetworkAccess::Enabled) {
                summary.push_str(" (network access enabled)");
            }
            summary
        }
        SandboxPolicy::WorkspaceWrite {
            writable_roots,
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        } => {
            let mut summary = "workspace-write".to_string();

            let mut writable_entries = Vec::<String>::new();
            writable_entries.push("workdir".to_string());
            if !*exclude_slash_tmp {
                writable_entries.push("/tmp".to_string());
            }
            if !*exclude_tmpdir_env_var {
                writable_entries.push("$TMPDIR".to_string());
            }
            writable_entries.extend(
                writable_roots
                    .iter()
                    .map(|p| p.to_string_lossy().to_string()),
            );

            summary.push_str(&format!(" [{}]", writable_entries.join(", ")));
            if *network_access {
                summary.push_str(" (network access enabled)");
            }
            summary
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    #[test]
    fn summarizes_external_sandbox_without_network_access_suffix() {
        let summary = summarize_sandbox_policy(&SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Restricted,
        });
        assert_eq!(summary, "external-sandbox");
    }

    #[test]
    fn summarizes_external_sandbox_with_enabled_network() {
        let summary = summarize_sandbox_policy(&SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Enabled,
        });
        assert_eq!(summary, "external-sandbox (network access enabled)");
    }

    #[test]
    fn workspace_write_summary_still_includes_network_access() {
        let root = if cfg!(windows) { "C:\\repo" } else { "/repo" };
        let writable_root = AbsolutePathBuf::try_from(root).unwrap();
        let summary = summarize_sandbox_policy(&SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![writable_root.clone()],
            network_access: true,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        });
        assert_eq!(
            summary,
            format!(
                "workspace-write [workdir, {}] (network access enabled)",
                writable_root.to_string_lossy()
            )
        );
    }
}
