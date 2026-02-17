use codex_core::protocol::SandboxPolicy;
use std::path::PathBuf;

/// Returns a warning describing why `--add-dir` entries will be ignored for the
/// resolved sandbox policy. The caller is responsible for presenting the
/// warning to the user (for example, printing to stderr).
pub fn add_dir_warning_message(
    additional_dirs: &[PathBuf],
    sandbox_policy: &SandboxPolicy,
) -> Option<String> {
    if additional_dirs.is_empty() {
        return None;
    }

    match sandbox_policy {
        SandboxPolicy::WorkspaceWrite { .. }
        | SandboxPolicy::DangerFullAccess
        | SandboxPolicy::ExternalSandbox { .. } => None,
        SandboxPolicy::ReadOnly { .. } => Some(format_warning(additional_dirs)),
    }
}

fn format_warning(additional_dirs: &[PathBuf]) -> String {
    let joined_paths = additional_dirs
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Ignoring --add-dir ({joined_paths}) because the effective sandbox mode is read-only. Switch to workspace-write or danger-full-access to allow additional writable roots."
    )
}

#[cfg(test)]
mod tests {
    use super::add_dir_warning_message;
    use codex_core::protocol::NetworkAccess;
    use codex_core::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn returns_none_for_workspace_write() {
        let sandbox = SandboxPolicy::new_workspace_write_policy();
        let dirs = vec![PathBuf::from("/tmp/example")];
        assert_eq!(add_dir_warning_message(&dirs, &sandbox), None);
    }

    #[test]
    fn returns_none_for_danger_full_access() {
        let sandbox = SandboxPolicy::DangerFullAccess;
        let dirs = vec![PathBuf::from("/tmp/example")];
        assert_eq!(add_dir_warning_message(&dirs, &sandbox), None);
    }

    #[test]
    fn returns_none_for_external_sandbox() {
        let sandbox = SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Enabled,
        };
        let dirs = vec![PathBuf::from("/tmp/example")];
        assert_eq!(add_dir_warning_message(&dirs, &sandbox), None);
    }

    #[test]
    fn warns_for_read_only() {
        let sandbox = SandboxPolicy::new_read_only_policy();
        let dirs = vec![PathBuf::from("relative"), PathBuf::from("/abs")];
        let message = add_dir_warning_message(&dirs, &sandbox)
            .expect("expected warning for read-only sandbox");
        assert_eq!(
            message,
            "Ignoring --add-dir (relative, /abs) because the effective sandbox mode is read-only. Switch to workspace-write or danger-full-access to allow additional writable roots."
        );
    }

    #[test]
    fn returns_none_when_no_additional_dirs() {
        let sandbox = SandboxPolicy::new_read_only_policy();
        let dirs: Vec<PathBuf> = Vec::new();
        assert_eq!(add_dir_warning_message(&dirs, &sandbox), None);
    }
}
