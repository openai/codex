use crate::policy::SandboxPolicy;
use dunce::canonicalize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct AllowDenyPaths {
    pub allow: HashSet<PathBuf>,
    pub deny: HashSet<PathBuf>,
}

pub fn compute_allow_paths(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
) -> AllowDenyPaths {
    let mut allow: HashSet<PathBuf> = HashSet::new();
    let mut deny: HashSet<PathBuf> = HashSet::new();

    let mut add_allow_path = |p: PathBuf| {
        if p.exists() {
            allow.insert(p);
        }
    };
    let mut add_deny_path = |p: PathBuf| {
        if p.exists() {
            deny.insert(p);
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
        let add_writable_root =
            |root: PathBuf,
             policy_cwd: &Path,
             add_allow: &mut dyn FnMut(PathBuf),
             add_deny: &mut dyn FnMut(PathBuf)| {
                let candidate = if root.is_absolute() {
                    root
                } else {
                    policy_cwd.join(root)
                };
                let canonical = canonicalize(&candidate).unwrap_or(candidate);
                add_allow(canonical.clone());

                let git_entry = canonical.join(".git");
                if git_entry.exists() {
                    add_deny(git_entry);
                }
            };

        add_writable_root(
            command_cwd.to_path_buf(),
            policy_cwd,
            &mut add_allow_path,
            &mut add_deny_path,
        );

        if let SandboxPolicy::WorkspaceWrite { writable_roots, .. } = policy {
            for root in writable_roots {
                add_writable_root(
                    root.clone().into(),
                    policy_cwd,
                    &mut add_allow_path,
                    &mut add_deny_path,
                );
            }
        }
    }
    if include_tmp_env_vars {
        for key in ["TEMP", "TMP"] {
            if let Some(v) = env_map.get(key) {
                let abs = PathBuf::from(v);
                add_allow_path(abs);
            } else if let Ok(v) = std::env::var(key) {
                let abs = PathBuf::from(v);
                add_allow_path(abs);
            }
        }
    }
    AllowDenyPaths { allow, deny }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use std::fs;
    use tempfile::TempDir;

    fn workspace_write_policy(
        writable_roots: Vec<AbsolutePathBuf>,
        network_access: bool,
        exclude_tmpdir_env_var: bool,
        exclude_slash_tmp: bool,
    ) -> SandboxPolicy {
        let mut policy = SandboxPolicy::new_workspace_write_policy();
        let SandboxPolicy::WorkspaceWrite {
            writable_roots: policy_writable_roots,
            network_access: policy_network_access,
            exclude_tmpdir_env_var: policy_exclude_tmpdir_env_var,
            exclude_slash_tmp: policy_exclude_slash_tmp,
            ..
        } = &mut policy
        else {
            panic!("workspace-write policy expected");
        };

        *policy_writable_roots = writable_roots;
        *policy_network_access = network_access;
        *policy_exclude_tmpdir_env_var = exclude_tmpdir_env_var;
        *policy_exclude_slash_tmp = exclude_slash_tmp;
        policy
    }

    #[test]
    fn includes_additional_writable_roots() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let extra_root = tmp.path().join("extra");
        let _ = fs::create_dir_all(&command_cwd);
        let _ = fs::create_dir_all(&extra_root);

        let policy = workspace_write_policy(
            vec![AbsolutePathBuf::try_from(extra_root.as_path()).unwrap()],
            false,
            false,
            false,
        );

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &HashMap::new());

        assert!(paths
            .allow
            .contains(&dunce::canonicalize(&command_cwd).unwrap()));
        assert!(paths
            .allow
            .contains(&dunce::canonicalize(&extra_root).unwrap()));
        assert!(paths.deny.is_empty(), "no deny paths expected");
    }

    #[test]
    fn excludes_tmp_env_vars_when_requested() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let temp_dir = tmp.path().join("temp");
        let _ = fs::create_dir_all(&command_cwd);
        let _ = fs::create_dir_all(&temp_dir);

        let policy = workspace_write_policy(vec![], false, true, false);
        let mut env_map = HashMap::new();
        env_map.insert("TEMP".into(), temp_dir.to_string_lossy().to_string());

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &env_map);

        assert!(paths
            .allow
            .contains(&dunce::canonicalize(&command_cwd).unwrap()));
        assert!(!paths
            .allow
            .contains(&dunce::canonicalize(&temp_dir).unwrap()));
        assert!(paths.deny.is_empty(), "no deny paths expected");
    }

    #[test]
    fn denies_git_dir_inside_writable_root() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let git_dir = command_cwd.join(".git");
        let _ = fs::create_dir_all(&git_dir);

        let policy = workspace_write_policy(vec![], false, true, false);

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &HashMap::new());
        let expected_allow: HashSet<PathBuf> = [dunce::canonicalize(&command_cwd).unwrap()]
            .into_iter()
            .collect();
        let expected_deny: HashSet<PathBuf> = [dunce::canonicalize(&git_dir).unwrap()]
            .into_iter()
            .collect();

        assert_eq!(expected_allow, paths.allow);
        assert_eq!(expected_deny, paths.deny);
    }

    #[test]
    fn denies_git_file_inside_writable_root() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let git_file = command_cwd.join(".git");
        let _ = fs::create_dir_all(&command_cwd);
        let _ = fs::write(&git_file, "gitdir: .git/worktrees/example");

        let policy = workspace_write_policy(vec![], false, true, false);

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &HashMap::new());
        let expected_allow: HashSet<PathBuf> = [dunce::canonicalize(&command_cwd).unwrap()]
            .into_iter()
            .collect();
        let expected_deny: HashSet<PathBuf> = [dunce::canonicalize(&git_file).unwrap()]
            .into_iter()
            .collect();

        assert_eq!(expected_allow, paths.allow);
        assert_eq!(expected_deny, paths.deny);
    }

    #[test]
    fn skips_git_dir_when_missing() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let _ = fs::create_dir_all(&command_cwd);

        let policy = workspace_write_policy(vec![], false, true, false);

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &HashMap::new());
        assert_eq!(paths.allow.len(), 1);
        assert!(paths.deny.is_empty(), "no deny when .git is absent");
    }
}
