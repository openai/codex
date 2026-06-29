use std::process::Command;
use tempfile::TempDir;

/// Environment variables that can redirect Git away from the repository
/// selected by `current_dir` or `-C`, or inject command-scoped configuration.
///
/// This follows `git rev-parse --local-env-vars` and also clears discovery and
/// namespace controls that affect repository selection. Indexed
/// `GIT_CONFIG_KEY_*` / `GIT_CONFIG_VALUE_*` entries are inert once both
/// command-scope entry points are removed.
const REPOSITORY_LOCAL_GIT_ENVIRONMENT_VARIABLES: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CEILING_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_CONFIG",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_DIR",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_GRAFT_FILE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_NAMESPACE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_PREFIX",
    "GIT_REPLACE_REF_BASE",
    "GIT_SHALLOW_FILE",
    "GIT_WORK_TREE",
];

/// Make an internal Git command honor its explicitly selected repository
/// instead of ambient repository state inherited by the Codex process.
///
/// User-global and system configuration, authentication helpers, and transport
/// configuration remain available.
pub(crate) fn sanitize_repository_environment(command: &mut Command) {
    for name in REPOSITORY_LOCAL_GIT_ENVIRONMENT_VARIABLES {
        command.env_remove(name);
    }
}

/// Runs transport commands from a clean, minimal repository so Git cannot
/// discover repository-local configuration from the directory where Codex was
/// launched. User-level Git configuration remains available.
pub(crate) struct NeutralGitCwd {
    directory: TempDir,
}

impl NeutralGitCwd {
    pub(crate) fn new() -> std::io::Result<Self> {
        let directory = tempfile::tempdir()?;
        initialize_empty_repository(directory.path())?;
        Ok(Self { directory })
    }

    #[cfg(all(test, unix))]
    fn from_prebuilt_directory(directory: TempDir) -> Self {
        Self { directory }
    }

    pub(crate) fn configure_transport_command(&self, command: &mut Command) {
        sanitize_repository_environment(command);
        command
            .current_dir(self.directory.path())
            .env("GIT_CEILING_DIRECTORIES", self.directory.path());
    }
}

fn initialize_empty_repository(root: &std::path::Path) -> std::io::Result<()> {
    // Build the discovery boundary without invoking Git. Running `git init`
    // before this repository exists could rediscover and honor a hostile
    // parent repository, which is exactly the behavior this type prevents.
    let git_dir = root.join(".git");
    std::fs::create_dir(&git_dir)?;
    std::fs::create_dir(git_dir.join("objects"))?;
    std::fs::create_dir_all(git_dir.join("refs/heads"))?;
    std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n")?;
    std::fs::write(
        git_dir.join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tbare = false\n",
    )
}

#[cfg(test)]
mod tests {
    use super::NeutralGitCwd;
    use super::REPOSITORY_LOCAL_GIT_ENVIRONMENT_VARIABLES;
    #[cfg(unix)]
    use super::initialize_empty_repository;
    use pretty_assertions::assert_eq;
    use std::ffi::OsStr;
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::path::Path;
    #[cfg(unix)]
    use std::path::PathBuf;
    use std::process::Command;

    #[test]
    fn configures_an_isolated_repository_discovery_boundary() {
        let neutral_cwd = NeutralGitCwd::new().expect("create neutral Git working directory");
        let mut command = Command::new("git");
        for name in REPOSITORY_LOCAL_GIT_ENVIRONMENT_VARIABLES {
            command.env(name, "hostile");
        }
        neutral_cwd.configure_transport_command(&mut command);

        assert_eq!(
            command.get_current_dir(),
            Some(neutral_cwd.directory.path())
        );
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == OsStr::new("GIT_CEILING_DIRECTORIES"))
                .and_then(|(_, value)| value),
            Some(neutral_cwd.directory.path().as_os_str())
        );
        for name in REPOSITORY_LOCAL_GIT_ENVIRONMENT_VARIABLES
            .iter()
            .filter(|name| **name != "GIT_CEILING_DIRECTORIES")
        {
            assert_eq!(
                command
                    .get_envs()
                    .find(|(key, _)| *key == OsStr::new(name))
                    .map(|(_, value)| value),
                Some(None),
                "{name} should be removed"
            );
        }
    }

    #[test]
    fn preserves_trusted_global_system_and_auth_environment() {
        let neutral_cwd = NeutralGitCwd::new().expect("create neutral Git working directory");
        let trusted = [
            ("GIT_ASKPASS", "/trusted/askpass"),
            ("GIT_CONFIG_GLOBAL", "/trusted/global.gitconfig"),
            ("GIT_CONFIG_SYSTEM", "/trusted/system.gitconfig"),
            ("GIT_SSH", "/trusted/ssh"),
            ("GIT_SSH_COMMAND", "/trusted/ssh-wrapper"),
            ("HOME", "/trusted/home"),
            ("SSH_AUTH_SOCK", "/trusted/ssh-agent.sock"),
            ("XDG_CONFIG_HOME", "/trusted/config"),
        ];
        let mut command = Command::new("git");
        for (name, value) in trusted {
            command.env(name, value);
        }
        neutral_cwd.configure_transport_command(&mut command);

        for (name, value) in trusted {
            assert_eq!(
                command
                    .get_envs()
                    .find(|(key, _)| *key == OsStr::new(name))
                    .and_then(|(_, value)| value),
                Some(OsStr::new(value)),
                "{name} should be preserved"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn nested_neutral_directory_ignores_parent_repository_transport_config() {
        let root = tempfile::tempdir().expect("create test root");
        let fixture = create_transport_fixture(root.path());

        let directory = tempfile::Builder::new()
            .prefix("neutral-")
            .tempdir_in(&fixture.hostile_repo)
            .expect("create nested neutral directory");
        initialize_empty_repository(directory.path()).expect("initialize neutral repository");
        let neutral_cwd = NeutralGitCwd::from_prebuilt_directory(directory);
        let empty_config = root.path().join("empty.gitconfig");
        fs::write(&empty_config, "").expect("write empty Git config");

        let mut config_command = Command::new("git");
        config_command
            .env("GIT_CONFIG_GLOBAL", &empty_config)
            .env("GIT_CONFIG_SYSTEM", &empty_config);
        neutral_cwd.configure_transport_command(&mut config_command);
        let config_output = config_command
            .args([
                "config",
                "--get-regexp",
                "^(core\\.sshCommand|credential\\.helper)$",
            ])
            .output()
            .expect("inspect effective transport configuration");
        assert_eq!(config_output.status.code(), Some(1));
        assert!(config_output.stdout.is_empty());

        let mut command = Command::new("git");
        command
            .env("GIT_CONFIG_GLOBAL", &empty_config)
            .env("GIT_CONFIG_SYSTEM", &empty_config);
        neutral_cwd.configure_transport_command(&mut command);
        let output = command
            .args([
                "ls-remote",
                fixture.source.to_string_lossy().as_ref(),
                "HEAD",
            ])
            .output()
            .expect("run git ls-remote");

        assert!(
            output.status.success(),
            "ls-remote should ignore the parent repository's transport rewrite: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.is_empty());
        assert!(
            !fixture.marker.exists(),
            "repository-selected transport helper must not run"
        );
    }

    #[cfg(unix)]
    #[test]
    fn neutral_directory_honors_trusted_global_and_system_transport_config() {
        let root = tempfile::tempdir().expect("create test root");
        let fixture = create_transport_fixture(root.path());
        let global_config = root.path().join("global.gitconfig");
        let system_config = root.path().join("system.gitconfig");
        fs::write(
            &global_config,
            format!(
                "[url \"file://{}\"]\n\tinsteadOf = https://trusted.example/repository\n[core]\n\tsshCommand = trusted-ssh-command\n",
                fixture.source.display()
            ),
        )
        .expect("write trusted global Git config");
        fs::write(
            &system_config,
            "[credential]\n\thelper = trusted-credential-helper\n",
        )
        .expect("write trusted system Git config");
        let neutral_cwd = NeutralGitCwd::new().expect("create neutral Git working directory");

        let mut ssh_config = Command::new("git");
        ssh_config
            .env("GIT_CONFIG_GLOBAL", &global_config)
            .env("GIT_CONFIG_SYSTEM", &system_config);
        neutral_cwd.configure_transport_command(&mut ssh_config);
        let ssh_output = ssh_config
            .args(["config", "--get", "core.sshCommand"])
            .output()
            .expect("read trusted SSH configuration");
        assert!(ssh_output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&ssh_output.stdout).trim(),
            "trusted-ssh-command"
        );

        let mut credential_config = Command::new("git");
        credential_config
            .env("GIT_CONFIG_GLOBAL", &global_config)
            .env("GIT_CONFIG_SYSTEM", &system_config);
        neutral_cwd.configure_transport_command(&mut credential_config);
        let credential_output = credential_config
            .args(["config", "--get", "credential.helper"])
            .output()
            .expect("read trusted credential configuration");
        assert!(credential_output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&credential_output.stdout).trim(),
            "trusted-credential-helper"
        );

        let mut ls_remote = Command::new("git");
        ls_remote
            .env("GIT_CONFIG_GLOBAL", &global_config)
            .env("GIT_CONFIG_SYSTEM", &system_config);
        neutral_cwd.configure_transport_command(&mut ls_remote);
        let output = ls_remote
            .args(["ls-remote", "https://trusted.example/repository", "HEAD"])
            .output()
            .expect("run git ls-remote through trusted global rewrite");
        assert!(
            output.status.success(),
            "trusted global URL rewrite should remain available: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn neutral_directory_ignores_inherited_git_common_dir() {
        let root = tempfile::tempdir().expect("create test root");
        let fixture = create_transport_fixture(root.path());
        let neutral_cwd = NeutralGitCwd::new().expect("create neutral Git working directory");
        let mut command = Command::new("git");
        command.env("GIT_COMMON_DIR", fixture.hostile_repo.join(".git"));
        neutral_cwd.configure_transport_command(&mut command);

        let output = command
            .args([
                "ls-remote",
                fixture.source.to_string_lossy().as_ref(),
                "HEAD",
            ])
            .output()
            .expect("run git ls-remote");

        assert!(
            output.status.success(),
            "ls-remote should ignore inherited GIT_COMMON_DIR: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.is_empty());
        assert!(
            !fixture.marker.exists(),
            "inherited common-dir transport helper must not run"
        );
    }

    #[cfg(unix)]
    #[test]
    fn neutral_directory_ignores_inherited_command_scope_config() {
        let root = tempfile::tempdir().expect("create test root");
        let fixture = create_transport_fixture(root.path());
        let neutral_cwd = NeutralGitCwd::new().expect("create neutral Git working directory");
        let rewrite_key = format!("url.ext::{}.insteadOf", fixture.helper.display());
        let mut command = Command::new("git");
        command
            .env("GIT_CONFIG_COUNT", "2")
            .env("GIT_CONFIG_KEY_0", "protocol.ext.allow")
            .env("GIT_CONFIG_VALUE_0", "always")
            .env("GIT_CONFIG_KEY_1", rewrite_key)
            .env(
                "GIT_CONFIG_VALUE_1",
                fixture.source.to_string_lossy().as_ref(),
            );
        neutral_cwd.configure_transport_command(&mut command);

        let output = command
            .args([
                "ls-remote",
                fixture.source.to_string_lossy().as_ref(),
                "HEAD",
            ])
            .output()
            .expect("run git ls-remote");

        assert!(
            output.status.success(),
            "ls-remote should ignore inherited command-scope config: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.is_empty());
        assert!(
            !fixture.marker.exists(),
            "command-scope transport helper must not run"
        );
    }

    #[cfg(unix)]
    struct TransportFixture {
        source: PathBuf,
        hostile_repo: PathBuf,
        helper: PathBuf,
        marker: PathBuf,
    }

    #[cfg(unix)]
    fn create_transport_fixture(root: &Path) -> TransportFixture {
        let source = root.join("source");
        fs::create_dir(&source).expect("create source repository");
        run_git(&source, &["init"]);
        run_git(&source, &["config", "user.email", "codex-test@example.com"]);
        run_git(&source, &["config", "user.name", "Codex Test"]);
        fs::write(source.join("README.md"), "safe source\n").expect("write source file");
        run_git(&source, &["add", "README.md"]);
        run_git(&source, &["commit", "-m", "initial"]);

        let hostile_repo = root.join("hostile");
        fs::create_dir(&hostile_repo).expect("create hostile repository");
        run_git(&hostile_repo, &["init"]);
        let marker = root.join("transport-ran");
        let helper = root.join("transport-helper.sh");
        fs::write(
            &helper,
            format!("#!/bin/sh\nprintf ran > \"{}\"\nexit 1\n", marker.display()),
        )
        .expect("write transport helper");
        let mut permissions = fs::metadata(&helper)
            .expect("read transport helper metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&helper, permissions).expect("mark transport helper executable");
        run_git(&hostile_repo, &["config", "protocol.ext.allow", "always"]);
        run_git(
            &hostile_repo,
            &["config", "core.sshCommand", "hostile-ssh-command"],
        );
        run_git(
            &hostile_repo,
            &["config", "credential.helper", "!hostile-credential-helper"],
        );
        let rewrite_key = format!("url.ext::{}.insteadOf", helper.display());
        run_git(
            &hostile_repo,
            &["config", &rewrite_key, source.to_string_lossy().as_ref()],
        );

        TransportFixture {
            source,
            hostile_repo,
            helper,
            marker,
        }
    }

    #[cfg(unix)]
    fn run_git(cwd: &std::path::Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
