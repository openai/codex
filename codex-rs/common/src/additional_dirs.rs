use codex_core::protocol::SandboxPolicy;
use std::path::PathBuf;

/// Returns a warning describing why `--add-dir` entries will be ignored for the
/// resolved sandbox policy. The caller is responsible for presenting the
/// warning to the user (e.g. printing to stderr).
pub fn add_dir_warning_message(
    additional_dirs: &[PathBuf],
    sandbox_policy: &SandboxPolicy,
) -> Option<String> {
    if additional_dirs.is_empty() {
        return None;
    }

    match sandbox_policy {
        SandboxPolicy::WorkspaceWrite { .. } => None,
        SandboxPolicy::ReadOnly => Some(format_warning(additional_dirs, "read-only")),
        SandboxPolicy::DangerFullAccess => {
            Some(format_warning(additional_dirs, "danger-full-access"))
        }
    }
}

fn format_warning(additional_dirs: &[PathBuf], sandbox_label: &str) -> String {
    let joined_paths = additional_dirs
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Ignoring --add-dir ({joined_paths}) because the effective sandbox mode is {sandbox_label}. Switch to workspace-write to allow additional writable roots."
    )
}

#[cfg(test)]
mod tests {
    use super::add_dir_warning_message;
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
    fn warns_for_read_only() {
        let sandbox = SandboxPolicy::ReadOnly;
        let dirs = vec![PathBuf::from("relative"), PathBuf::from("/abs")];
        let message = add_dir_warning_message(&dirs, &sandbox)
            .expect("expected warning for read-only sandbox");
        assert_eq!(
            message,
            "Ignoring --add-dir (relative, /abs) because the effective sandbox mode is read-only. Switch to workspace-write to allow additional writable roots."
        );
    }

    #[test]
    fn warns_for_danger_full_access() {
        let sandbox = SandboxPolicy::DangerFullAccess;
        let dirs = vec![PathBuf::from("/tmp/abs")];
        let message = add_dir_warning_message(&dirs, &sandbox)
            .expect("expected warning for danger-full-access sandbox");
        assert_eq!(
            message,
            "Ignoring --add-dir (/tmp/abs) because the effective sandbox mode is danger-full-access. Switch to workspace-write to allow additional writable roots."
        );
    }
}
