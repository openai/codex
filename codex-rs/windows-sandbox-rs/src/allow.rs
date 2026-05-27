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
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use std::fs;
    use tempfile::TempDir;

    fn workspace_write_profile(
        writable_roots: &[AbsolutePathBuf],
        exclude_tmpdir_env_var: bool,
        exclude_slash_tmp: bool,
    ) -> PermissionProfile {
        PermissionProfile::workspace_write_with(
            writable_roots,
            NetworkSandboxPolicy::Restricted,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        )
    }

    fn compute_allow_paths(
        permission_profile: &PermissionProfile,
        permission_profile_cwd: &Path,
        command_cwd: &Path,
        env_map: &HashMap<String, String>,
    ) -> AllowDenyPaths {
        let permissions = ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_cwd(
            permission_profile,
            permission_profile_cwd,
        )
        .expect("managed permission profile");
        compute_allow_paths_for_permissions(&permissions, command_cwd, env_map)
    }

    #[test]
    fn includes_additional_writable_roots() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let extra_root = tmp.path().join("extra");
        let _ = fs::create_dir_all(&command_cwd);
        let _ = fs::create_dir_all(&extra_root);

        let writable_roots = vec![AbsolutePathBuf::try_from(extra_root.as_path()).unwrap()];
        let permission_profile = workspace_write_profile(
            &writable_roots,
            /*exclude_tmpdir_env_var*/ false,
            /*exclude_slash_tmp*/ false,
        );

        let paths = compute_allow_paths(
            &permission_profile,
            &command_cwd,
            &command_cwd,
            &HashMap::new(),
        );

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
    fn uses_profile_cwd_for_workspace_root() {
        let tmp = TempDir::new().expect("tempdir");
        let permission_profile_cwd = tmp.path().join("workspace");
        let command_cwd = permission_profile_cwd.join("subdir");
        fs::create_dir_all(&command_cwd).expect("create command cwd");

        let permission_profile = workspace_write_profile(
            &[],
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ true,
        );

        let paths = compute_allow_paths(
            &permission_profile,
            &permission_profile_cwd,
            &command_cwd,
            &HashMap::new(),
        );

        assert!(
            paths
                .allow
                .contains(&dunce::canonicalize(&permission_profile_cwd).unwrap())
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

        let permission_profile = workspace_write_profile(
            &[],
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ false,
        );
        let mut env_map = HashMap::new();
        env_map.insert("TEMP".into(), temp_dir.to_string_lossy().to_string());
        env_map.insert("TMP".into(), temp_dir.to_string_lossy().to_string());

        let paths = compute_allow_paths(&permission_profile, &command_cwd, &command_cwd, &env_map);

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

        let permission_profile = workspace_write_profile(
            &[],
            /*exclude_tmpdir_env_var*/ false,
            /*exclude_slash_tmp*/ false,
        );
        let mut env_map = HashMap::new();
        env_map.insert("TEMP".into(), temp_dir.to_string_lossy().to_string());
        env_map.insert("TMP".into(), temp_dir.to_string_lossy().to_string());

        let paths = compute_allow_paths(&permission_profile, &command_cwd, &command_cwd, &env_map);

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

        let permission_profile = workspace_write_profile(
            &[],
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ false,
        );

        let paths = compute_allow_paths(
            &permission_profile,
            &command_cwd,
            &command_cwd,
            &HashMap::new(),
        );
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

        let permission_profile = workspace_write_profile(
            &[],
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ false,
        );

        let paths = compute_allow_paths(
            &permission_profile,
            &command_cwd,
            &command_cwd,
            &HashMap::new(),
        );
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

        let permission_profile = workspace_write_profile(
            &[],
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ false,
        );

        let paths = compute_allow_paths(
            &permission_profile,
            &command_cwd,
            &command_cwd,
            &HashMap::new(),
        );
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

        let permission_profile = workspace_write_profile(
            &[],
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ false,
        );

        let paths = compute_allow_paths(
            &permission_profile,
            &command_cwd,
            &command_cwd,
            &HashMap::new(),
        );
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

        let permission_profile = workspace_write_profile(
            &[],
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ false,
        );

        let paths = compute_allow_paths(
            &permission_profile,
            &command_cwd,
            &command_cwd,
            &HashMap::new(),
        );
        assert_eq!(paths.allow.len(), 1);
        assert!(
            paths.deny.is_empty(),
            "no deny when protected dirs are absent"
        );
    }
}
