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

fn resolve_gitdir_from_file(dot_git: &Path) -> Option<PathBuf> {
    let contents = std::fs::read_to_string(dot_git).ok()?;
    let (_, gitdir_raw) = contents.trim().split_once(':')?;
    let gitdir_raw = gitdir_raw.trim();
    if gitdir_raw.is_empty() {
        return None;
    }
    let base = dot_git.parent()?;
    let gitdir = if Path::new(gitdir_raw).is_absolute() {
        PathBuf::from(gitdir_raw)
    } else {
        base.join(gitdir_raw)
    };
    Some(canonicalize(&gitdir).unwrap_or(gitdir))
}

fn add_git_metadata_paths(
    root: &Path,
    add_allow: &mut dyn FnMut(PathBuf),
    add_deny: &mut dyn FnMut(PathBuf),
) {
    let top_level_git = root.join(".git");
    let git_dirs = if top_level_git.is_dir() {
        vec![canonicalize(&top_level_git).unwrap_or(top_level_git)]
    } else if top_level_git.is_file() {
        resolve_gitdir_from_file(&top_level_git)
            .into_iter()
            .inspect(|gitdir| add_allow(gitdir.clone()))
            .collect()
    } else {
        Vec::new()
    };

    for git_dir in git_dirs {
        for protected_subpath in ["config", "hooks"] {
            let protected_path = git_dir.join(protected_subpath);
            if protected_path.exists() {
                add_deny(protected_path);
            }
        }
    }
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

                add_git_metadata_paths(&canonical, add_allow, add_deny);

                for protected_subdir in [".codex", ".agents"] {
                    let protected_entry = canonical.join(protected_subdir);
                    if protected_entry.exists() {
                        add_deny(protected_entry);
                    }
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

    #[test]
    fn includes_additional_writable_roots() {
        let tmp = TempDir::new().expect("tempdir");
        let command_cwd = tmp.path().join("workspace");
        let extra_root = tmp.path().join("extra");
        let _ = fs::create_dir_all(&command_cwd);
        let _ = fs::create_dir_all(&extra_root);

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![AbsolutePathBuf::try_from(extra_root.as_path()).unwrap()],
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };

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

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        };
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
        let git_config = git_dir.join("config");
        let git_hooks = git_dir.join("hooks");
        let _ = fs::create_dir_all(&git_dir);
        let _ = fs::write(&git_config, "[core]\n");
        let _ = fs::create_dir_all(&git_hooks);

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        };

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &HashMap::new());
        let expected_allow: HashSet<PathBuf> = [dunce::canonicalize(&command_cwd).unwrap()]
            .into_iter()
            .collect();
        let expected_deny: HashSet<PathBuf> = [
            dunce::canonicalize(&git_config).unwrap(),
            dunce::canonicalize(&git_hooks).unwrap(),
        ]
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
        let gitdir = tmp.path().join("repo.git").join("worktrees").join("example");
        let git_config = gitdir.join("config");
        let git_hooks = gitdir.join("hooks");
        let _ = fs::create_dir_all(&command_cwd);
        let _ = fs::create_dir_all(&gitdir);
        let _ = fs::write(&git_config, "[core]\n");
        let _ = fs::create_dir_all(&git_hooks);
        let _ = fs::write(&git_file, format!("gitdir: {}\n", gitdir.display()));

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        };

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &HashMap::new());
        let expected_allow: HashSet<PathBuf> = [
            dunce::canonicalize(&command_cwd).unwrap(),
            dunce::canonicalize(&gitdir).unwrap(),
        ]
        .into_iter()
        .collect();
        let expected_deny: HashSet<PathBuf> = [
            dunce::canonicalize(&git_config).unwrap(),
            dunce::canonicalize(&git_hooks).unwrap(),
        ]
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
            read_only_access: Default::default(),
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
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        };

        let paths = compute_allow_paths(&policy, &command_cwd, &command_cwd, &HashMap::new());
        assert_eq!(paths.allow.len(), 1);
        assert!(paths.deny.is_empty(), "no deny when protected dirs are absent");
    }
}
