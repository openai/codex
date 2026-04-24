use std::path::Path;
use std::path::PathBuf;

use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::forbidden_agent_preserved_path_write;

pub fn preserved_path_write_forbidden_reason(
    command: &[String],
    cwd: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
) -> Option<String> {
    let commands = crate::bash::parse_shell_lc_plain_commands(command)
        .or_else(|| crate::bash::parse_shell_lc_command_word_prefixes(command))
        .unwrap_or_else(|| vec![command.to_vec()]);

    for simple_command in commands {
        if let Some(name) =
            simple_command_preserved_path_write(&simple_command, cwd, file_system_sandbox_policy)
        {
            return Some(preserved_path_write_reason(name));
        }
    }

    if let Some(targets) = crate::bash::parse_shell_lc_write_redirection_targets(command) {
        for target in targets {
            if let Some(name) = forbidden_agent_preserved_path_write(
                Path::new(&target),
                cwd,
                file_system_sandbox_policy,
            ) {
                return Some(preserved_path_write_reason(name));
            }
        }
    }
    None
}

fn preserved_path_write_reason(name: &str) -> String {
    format!("command targets preserved workspace metadata path `{name}`")
}

fn simple_command_preserved_path_write(
    command: &[String],
    cwd: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
) -> Option<&'static str> {
    let program = command.first().map(|program| {
        Path::new(program)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(program)
    })?;

    match program {
        "git" => git_init_preserved_path_write(command, cwd, file_system_sandbox_policy),
        "touch" | "mkdir" | "rm" | "rmdir" | "ln" | "mv" | "cp" | "install" => command
            .iter()
            .skip(1)
            .filter(|arg| !arg.starts_with('-'))
            .find_map(|arg| {
                forbidden_agent_preserved_path_write(
                    Path::new(arg),
                    cwd,
                    file_system_sandbox_policy,
                )
            }),
        _ => None,
    }
}

fn git_init_preserved_path_write(
    command: &[String],
    cwd: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
) -> Option<&'static str> {
    let mut git_cwd = PathBuf::from(cwd);
    let mut index = 1;

    while index < command.len() {
        match command[index].as_str() {
            "-C" => {
                let next = command.get(index + 1)?;
                git_cwd = resolve_shell_operand(Path::new(next), &git_cwd);
                index += 2;
            }
            "--" => {
                index += 1;
                break;
            }
            arg if arg.starts_with('-') => {
                index += 1;
            }
            _ => break,
        }
    }

    if command.get(index).map(String::as_str) != Some("init") {
        return None;
    }

    let init_target = command
        .iter()
        .skip(index + 1)
        .find(|arg| !arg.starts_with('-'))
        .map_or_else(
            || git_cwd.clone(),
            |arg| resolve_shell_operand(Path::new(arg), &git_cwd),
        );

    forbidden_agent_preserved_path_write(
        init_target.join(".git").as_path(),
        &git_cwd,
        file_system_sandbox_policy,
    )
}

fn resolve_shell_operand(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;

    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::protocol::ReadOnlyAccess;
    use codex_protocol::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;

    use super::preserved_path_write_forbidden_reason;

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "codex-preserved-path-write-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir(&path).expect("create tempdir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn legacy_workspace_write_policy(cwd: &Path) -> FileSystemSandboxPolicy {
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: vec![],
            },
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };
        FileSystemSandboxPolicy::from_legacy_sandbox_policy(&policy, cwd)
    }

    #[test]
    fn preserved_path_detector_blocks_git_init_under_parent_repo() {
        let repo = TestDir::new("git-init-under-parent-repo");
        std::fs::create_dir(repo.path().join(".git")).expect("create parent .git");
        let cwd = repo.path().join("sub");
        std::fs::create_dir(&cwd).expect("create cwd");
        let policy = legacy_workspace_write_policy(&cwd);

        let reason = preserved_path_write_forbidden_reason(
            &[
                "/bin/bash".to_string(),
                "-lc".to_string(),
                "git init".to_string(),
            ],
            &cwd,
            &policy,
        );

        assert_eq!(
            reason,
            Some("command targets preserved workspace metadata path `.git`".to_string())
        );
    }

    #[test]
    fn preserved_path_detector_allows_normal_git_under_parent_repo() {
        let repo = TestDir::new("normal-git-under-parent-repo");
        std::fs::create_dir(repo.path().join(".git")).expect("create parent .git");
        let cwd = repo.path().join("sub");
        std::fs::create_dir(&cwd).expect("create cwd");
        let policy = legacy_workspace_write_policy(&cwd);

        let reason = preserved_path_write_forbidden_reason(
            &[
                "/bin/bash".to_string(),
                "-lc".to_string(),
                "git status --short".to_string(),
            ],
            &cwd,
            &policy,
        );

        assert_eq!(reason, None);
    }

    #[test]
    fn preserved_path_detector_blocks_direct_preserved_path_writes() {
        let cwd = TestDir::new("direct-preserved-path-writes");
        let policy = legacy_workspace_write_policy(cwd.path());

        let reason = preserved_path_write_forbidden_reason(
            &[
                "/bin/bash".to_string(),
                "-lc".to_string(),
                "touch .git && mkdir -p .codex".to_string(),
            ],
            cwd.path(),
            &policy,
        );

        assert_eq!(
            reason,
            Some("command targets preserved workspace metadata path `.git`".to_string())
        );
    }

    #[test]
    fn preserved_path_detector_blocks_preserved_path_redirections() {
        let repo = TestDir::new("preserved-path-redirections");
        std::fs::create_dir(repo.path().join(".git")).expect("create parent .git");
        let cwd = repo.path().join("sub");
        std::fs::create_dir(&cwd).expect("create cwd");
        let policy = legacy_workspace_write_policy(&cwd);

        let reason = preserved_path_write_forbidden_reason(
            &[
                "/bin/bash".to_string(),
                "-lc".to_string(),
                "printf pwned > .git".to_string(),
            ],
            &cwd,
            &policy,
        );

        assert_eq!(
            reason,
            Some("command targets preserved workspace metadata path `.git`".to_string())
        );
    }

    #[test]
    fn preserved_path_detector_blocks_git_init_inside_complex_script() {
        let repo = TestDir::new("git-init-inside-complex-script");
        std::fs::create_dir(repo.path().join(".git")).expect("create parent .git");
        let cwd = repo.path().join("sub");
        std::fs::create_dir(&cwd).expect("create cwd");
        let policy = legacy_workspace_write_policy(&cwd);

        let reason = preserved_path_write_forbidden_reason(
            &[
                "/bin/bash".to_string(),
                "-lc".to_string(),
                "set -e\nif git init -q; then\n  exit 22\nfi".to_string(),
            ],
            &cwd,
            &policy,
        );

        assert_eq!(
            reason,
            Some("command targets preserved workspace metadata path `.git`".to_string())
        );
    }
}
