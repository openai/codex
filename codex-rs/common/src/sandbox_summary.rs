use codex_core::protocol::SandboxPolicy;

pub fn summarize_sandbox_policy(sandbox_policy: &SandboxPolicy) -> String {
    match sandbox_policy {
        SandboxPolicy::DangerFullAccess => "danger-full-access".to_string(),
        SandboxPolicy::ReadOnly => "read-only".to_string(),
        SandboxPolicy::WorkspaceWrite {
            writable_roots,
            network_access,
            include_default_writable_roots,
        } => {
            let mut summary = "workspace-write".to_string();
            if !writable_roots.is_empty() {
                summary.push_str(&format!(
                    " [{}]",
                    writable_roots
                        .iter()
                        .map(|p| p.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if !*include_default_writable_roots {
                summary.push_str(" (exact writable roots)");
            }
            if *network_access {
                summary.push_str(" (network access enabled)");
            }
            summary
        }
    }
}
