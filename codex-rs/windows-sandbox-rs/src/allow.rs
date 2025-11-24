use crate::policy::SandboxPolicy;
use dunce::canonicalize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

pub fn compute_allow_paths(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
) -> Vec<PathBuf> {
    let mut allow: Vec<PathBuf> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut add_path = |p: PathBuf| {
        if seen.insert(p.to_string_lossy().to_string()) && p.exists() {
            allow.push(p);
        }
    };
    let include_tmp_env_vars = matches!(
        policy,
        SandboxPolicy::WorkspaceWrite {
            exclude_tmpdir_env_var: false,
            ..
        }
    );

    if matches!(policy, SandboxPolicy::WorkspaceWrite { .. }) {
        add_path(command_cwd.to_path_buf());
        if let SandboxPolicy::WorkspaceWrite { writable_roots, .. } = policy {
            for root in writable_roots {
                let candidate = if root.is_absolute() {
                    root.clone()
                } else {
                    policy_cwd.join(root)
                };
                let canonical = canonicalize(&candidate).unwrap_or(candidate);
                add_path(canonical);
            }
        }
    }
    if include_tmp_env_vars {
        for key in ["TEMP", "TMP"] {
            if let Some(v) = env_map.get(key) {
                let abs = PathBuf::from(v);
                add_path(abs);
            } else if let Ok(v) = std::env::var(key) {
                let abs = PathBuf::from(v);
                add_path(abs);
            }
        }
    }
    allow
}

#[cfg(test)]
mod tests {
    use super::compute_allow_paths;
    use codex_protocol::protocol::SandboxPolicy;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn includes_additional_writable_roots() {
        let command_cwd = PathBuf::from(r"C:\Workspace");
        let extra_root = PathBuf::from(r"C:\AdditionalWritableRoot");
        let _ = fs::create_dir_all(&command_cwd);
        let _ = fs::create_dir_all(&extra_root);

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![extra_root.clone()],
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };

        let allow = compute_allow_paths(&policy, &command_cwd, &command_cwd, &HashMap::new());

        assert!(
            allow.iter().any(|p| p == &command_cwd),
            "command cwd should be allowed"
        );
        assert!(
            allow.iter().any(|p| p == &extra_root),
            "additional writable root should be allowed"
        );
    }

    #[test]
    fn excludes_tmp_env_vars_when_requested() {
        let command_cwd = PathBuf::from(r"C:\Workspace");
        let temp_dir = PathBuf::from(r"C:\TempDir");
        let _ = fs::create_dir_all(&command_cwd);
        let _ = fs::create_dir_all(&temp_dir);

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        };
        let mut env_map = HashMap::new();
        env_map.insert("TEMP".into(), temp_dir.to_string_lossy().to_string());

        let allow = compute_allow_paths(&policy, &command_cwd, &command_cwd, &env_map);

        assert!(
            allow.iter().any(|p| p == &command_cwd),
            "command cwd should be allowed"
        );
        assert!(
            !allow.iter().any(|p| p == &temp_dir),
            "TEMP should be excluded when exclude_tmpdir_env_var is true"
        );
    }
}
