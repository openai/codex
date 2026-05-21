use crate::resolved_permissions::ResolvedWindowsSandboxPermissions;
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

pub(crate) fn compute_allow_paths_for_permissions(
    permissions: &ResolvedWindowsSandboxPermissions,
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

    for writable_root in permissions.writable_roots_for_cwd(command_cwd, env_map) {
        let canonical = canonicalize(&writable_root.root).unwrap_or(writable_root.root);
        add_allow_path(canonical);
        for read_only_subpath in writable_root.read_only_subpaths {
            add_deny_path(read_only_subpath);
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

    fn compute_allow_paths(
        policy: &SandboxPolicy,
        policy_cwd: &Path,
        command_cwd: &Path,
        env_map: &HashMap<String, String>,
    ) -> AllowDenyPaths {
        let permissions =
            ResolvedWindowsSandboxPermissions::from_legacy_policy_for_cwd(policy, policy_cwd);
        compute_allow_paths_for_permissions(&permissions, command_cwd, env_map)
    }

    #[test]
    fn includes_additional_writable_roots() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let extra_root = tmp.path().join("extra");
        let _ = fs::create_dir_all(&command_cwd);
        let _ = fs::create_dir_all(&extra_root);

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![AbsolutePathBuf::try_from(extra_root.as_path()).unwrap()],
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &HashMap::new());

        assert!(
            paths
                .allow
                .contains(&dunce::canonicalize(&command_cwd).unwrap())
        );
        assert!(
            paths
                .allow
                .contains(&dunce::canonicalize(&extra_root).unwrap())
        );
        assert!(paths.deny.is_empty(), "no deny paths expected");
    }

    #[test]
    fn uses_policy_cwd_for_legacy_workspace_root() {
        let tmp = TempDir::new().expect("tempdir");
        let policy_cwd = tmp.path().join("workspace");
        let command_cwd = policy_cwd.join("subdir");
        fs::create_dir_all(&command_cwd).expect("create command cwd");

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let paths = compute_allow_paths(&policy, &policy_cwd, &command_cwd, &HashMap::new());

        assert!(
            paths
                .allow
                .contains(&dunce::canonicalize(&policy_cwd).unwrap())
        );
        assert!(
            !paths
                .allow
                .contains(&dunce::canonicalize(&command_cwd).unwrap())
        );
        assert!(paths.deny.is_empty(), "no deny paths expected");
    }

    #[test]
    fn excludes_tmp_env_vars_when_requested() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let temp_dir = tmp.path().join("temp");
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
        env_map.insert("TMP".into(), temp_dir.to_string_lossy().to_string());

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &env_map);

        assert!(
            paths
                .allow
                .contains(&dunce::canonicalize(&command_cwd).unwrap())
        );
        assert!(
            !paths
                .allow
                .contains(&dunce::canonicalize(&temp_dir).unwrap())
        );
        assert!(paths.deny.is_empty(), "no deny paths expected");
    }

    #[test]
    fn includes_tmp_env_vars_when_requested() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let temp_dir = tmp.path().join("temp");
        let _ = fs::create_dir_all(&command_cwd);
        let _ = fs::create_dir_all(&temp_dir);

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };
        let mut env_map = HashMap::new();
        env_map.insert("TEMP".into(), temp_dir.to_string_lossy().to_string());
        env_map.insert("TMP".into(), temp_dir.to_string_lossy().to_string());

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &env_map);

        let expected_allow: HashSet<PathBuf> = [
            dunce::canonicalize(&command_cwd).unwrap(),
            dunce::canonicalize(&temp_dir).unwrap(),
        ]
        .into_iter()
        .collect();

        assert_eq!(expected_allow, paths.allow);
        assert!(paths.deny.is_empty(), "no deny paths expected");
    }

    #[test]
    fn ignores_unix_slash_tmp_for_windows_allow_roots() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let _ = fs::create_dir_all(&command_cwd);

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        };

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &HashMap::new());
        let expected_allow: HashSet<PathBuf> = [dunce::canonicalize(&command_cwd).unwrap()]
            .into_iter()
            .collect();

        assert_eq!(expected_allow, paths.allow);
        assert!(paths.deny.is_empty(), "no deny paths expected");
    }

    #[test]
    fn denies_git_dir_inside_writable_root() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let git_dir = command_cwd.join(".git");
        let _ = fs::create_dir_all(&git_dir);

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        };

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

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        };

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
    fn denies_codex_and_agents_inside_writable_root() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let codex_dir = command_cwd.join(".codex");
        let agents_dir = command_cwd.join(".agents");
        let _ = fs::create_dir_all(&codex_dir);
        let _ = fs::create_dir_all(&agents_dir);

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        };

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &HashMap::new());
        let expected_allow: HashSet<PathBuf> = [dunce::canonicalize(&command_cwd).unwrap()]
            .into_iter()
            .collect();
        let expected_deny: HashSet<PathBuf> = [
            dunce::canonicalize(&codex_dir).unwrap(),
            dunce::canonicalize(&agents_dir).unwrap(),
        ]
        .into_iter()
        .collect();

        assert_eq!(expected_allow, paths.allow);
        assert_eq!(expected_deny, paths.deny);
    }

    #[test]
    fn skips_protected_subdirs_when_missing() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let _ = fs::create_dir_all(&command_cwd);

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        };

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &HashMap::new());
        assert_eq!(paths.allow.len(), 1);
        assert!(
            paths.deny.is_empty(),
            "no deny when protected dirs are absent"
        );
    }
}
