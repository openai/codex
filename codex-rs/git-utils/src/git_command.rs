use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

#[cfg(windows)]
use same_file::Handle;

use crate::errors::GitReadError;
use crate::git_config_environment::GitConfigEnvironmentSnapshot;
#[cfg(test)]
use crate::git_executable::git_executable_name;
use crate::git_executable::harden_git_launch_environment;
#[cfg(test)]
use crate::git_executable::path_is_untrusted;
#[cfg(test)]
use crate::git_executable::search_directory_is_untrusted;
use crate::git_executable::select_git_executable;
#[cfg(all(test, windows))]
use crate::git_executable::windows_path_requires_fail_closed;
use crate::repository_authority::RepositoryAuthority;
#[cfg(test)]
use crate::repository_authority::parse_marker_path as parse_git_marker_path;
use crate::safe_git::isolate_git_command_environment;

/// A Git executable outside the repository-controlled roots for one operation.
#[derive(Debug)]
pub(crate) struct GitRunner {
    /// Canonical executable target pinned at selection time. Never execute the
    /// mutable PATH spelling after validation.
    executable: PathBuf,
    #[cfg(any(unix, test))]
    argv0: PathBuf,
    safe_path: std::ffi::OsString,
    authority: RepositoryAuthority,
    config_environment: GitConfigEnvironmentSnapshot,
}

/// A Git command that can only be spawned through [`GitRunner::output`],
/// keeping metadata revalidation and launch hardening at one choke point.
pub(crate) struct GitCommand {
    inner: Command,
}

/// App-owned common repository metadata for one final three-way apply.
///
/// The real per-worktree Git directory remains selected for HEAD and index
/// state, while this directory replaces every common config and attribute
/// source that could define a repository-selected executable helper.
pub(crate) struct IsolatedGitCommonDir {
    root: tempfile::TempDir,
}

impl IsolatedGitCommonDir {
    pub(crate) fn config_path(&self) -> PathBuf {
        self.root.path().join("config")
    }

    fn system_config_path(&self) -> PathBuf {
        self.root.path().join("system.gitconfig")
    }

    fn global_config_path(&self) -> PathBuf {
        self.root.path().join("global.gitconfig")
    }

    fn home_path(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn xdg_config_home(&self) -> PathBuf {
        self.root.path().join("xdg")
    }

    fn validate(&self) -> io::Result<()> {
        for path in [
            self.config_path(),
            self.system_config_path(),
            self.global_config_path(),
            self.root.path().join("info/attributes"),
        ] {
            let metadata = std::fs::symlink_metadata(&path)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("isolated Git metadata file changed at {}", path.display()),
                ));
            }
        }
        for path in [
            self.root.path().join("objects"),
            self.root.path().join("refs"),
            self.root.path().join("info"),
            self.home_path(),
            self.xdg_config_home(),
        ] {
            let metadata = std::fs::symlink_metadata(&path)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "isolated Git metadata directory changed at {}",
                        path.display()
                    ),
                ));
            }
        }
        Ok(())
    }
}

impl GitCommand {
    pub(crate) fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    pub(crate) fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(args);
        self
    }

    pub(crate) fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env(key, value);
        self
    }

    pub(crate) fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env_remove(key);
        self
    }

    pub(crate) fn stdin(&mut self, config: impl Into<Stdio>) -> &mut Self {
        self.inner.stdin(config);
        self
    }
}

impl GitRunner {
    pub(crate) fn for_cwd(cwd: &Path) -> Result<Self, GitReadError> {
        #[cfg(test)]
        GIT_RUNNER_CONSTRUCTION_COUNT.with(|count| count.set(count.get() + 1));
        let authority = repository_authority_for_cwd(cwd)?;
        let search_path = std::env::var_os("PATH").ok_or(GitReadError::NoTrustedGit)?;
        Self::from_search_path(authority, &search_path)
    }

    pub(crate) fn for_cwd_io(cwd: &Path) -> io::Result<Self> {
        Self::for_cwd(cwd).map_err(GitReadError::into_io_error)
    }

    pub(crate) fn command(&self) -> GitCommand {
        let mut command = Command::new(&self.executable);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;

            command.arg0(&self.argv0);
        }
        if let Some(parent) = self.executable.parent() {
            command.current_dir(parent);
        }
        harden_git_launch_environment(&mut command, &self.safe_path);
        self.config_environment.apply_to(&mut command);
        GitCommand { inner: command }
    }

    pub(crate) fn command_for_cwd(&self, cwd: &Path) -> io::Result<GitCommand> {
        let cwd = if cwd.is_absolute() {
            cwd.to_path_buf()
        } else {
            std::env::current_dir()?.join(cwd)
        };
        let cwd = self.authority.canonical_command_cwd(&cwd)?;
        let mut command = self.command();
        command.arg("-C").arg(cwd);
        Ok(command)
    }

    pub(crate) fn ensure_config_source_is_not_worktree_controlled(
        &self,
        path: &Path,
        description: &str,
    ) -> io::Result<()> {
        self.authority
            .ensure_config_source_is_not_worktree_controlled(path, description)
    }

    pub(crate) fn ensure_active_worktree_root(&self, root: &Path) -> io::Result<()> {
        self.authority.ensure_active_worktree_root(root)
    }

    pub(crate) fn ensure_repository_root_route(&self, root: &Path) -> io::Result<()> {
        self.authority.ensure_repository_root_route(root)
    }

    pub(crate) fn config_environment_value(&self, name: &str) -> Option<&OsStr> {
        self.config_environment.value(name)
    }

    pub(crate) fn create_isolated_common_dir(&self) -> io::Result<IsolatedGitCommonDir> {
        let root = tempfile::tempdir()?;
        self.authority
            .ensure_config_source_is_not_worktree_controlled(
                root.path(),
                "owned isolated Git common directory",
            )?;
        for path in ["objects", "refs", "info", "home", "xdg"] {
            std::fs::create_dir_all(root.path().join(path))?;
        }
        for path in [
            "config",
            "system.gitconfig",
            "global.gitconfig",
            "info/attributes",
        ] {
            std::fs::write(root.path().join(path), [])?;
        }
        let isolated = IsolatedGitCommonDir { root };
        isolated.validate()?;
        Ok(isolated)
    }

    pub(crate) fn output(&self, mut command: GitCommand) -> io::Result<std::process::Output> {
        self.revalidate_active_repository_metadata()?;
        isolate_git_command_environment(&mut command.inner);
        command.inner.envs(crate::local_only_git_env());
        harden_git_launch_environment(&mut command.inner, &self.safe_path);
        command.inner.output()
    }

    pub(crate) fn output_in_isolated_common_dir(
        &self,
        mut command: GitCommand,
        isolated: &IsolatedGitCommonDir,
    ) -> io::Result<std::process::Output> {
        self.revalidate_active_repository_metadata()?;
        isolated.validate()?;
        self.authority
            .ensure_config_source_is_not_worktree_controlled(
                isolated.root.path(),
                "owned isolated Git common directory",
            )?;
        isolate_git_command_environment(&mut command.inner);
        scrub_repository_and_config_environment(&mut command.inner);

        let canonical_git_dir = self.authority.active_git_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git directory is unavailable for isolated three-way apply",
            )
        })?;
        let canonical_common_dir = self.authority.active_common_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git common directory is unavailable for isolated three-way apply",
            )
        })?;
        let canonical_worktree = self.authority.active_worktree_root();

        // Repository authority retains canonical paths for identity and route
        // validation. Git for Windows does not recognize the verbatim
        // `\\?\` spellings returned by `std::fs::canonicalize` in these
        // environment variables, so simplify only the child-facing spelling
        // after proving that it names the same existing directory.
        let git_dir = git_child_directory(canonical_git_dir, "active Git directory")?;
        let common_dir = git_child_directory(canonical_common_dir, "active Git common directory")?;
        let worktree = git_child_directory(canonical_worktree, "active Git worktree")?;
        #[cfg(windows)]
        let isolated_root_base = std::fs::canonicalize(isolated.root.path())?;
        #[cfg(not(windows))]
        let isolated_root_base = isolated.root.path().to_path_buf();
        let isolated_root =
            git_child_directory(&isolated_root_base, "owned isolated Git common directory")?;
        let index_file = git_dir.join("index");
        let object_directory = common_dir.join("objects");
        let system_config = isolated_root.join("system.gitconfig");
        let global_config = isolated_root.join("global.gitconfig");
        let home = isolated_root.join("home");
        let xdg_config_home = isolated_root.join("xdg");
        command
            .inner
            .env("GIT_DIR", git_dir.as_path())
            .env("GIT_COMMON_DIR", isolated_root.as_path())
            .env("GIT_WORK_TREE", worktree.as_path())
            .env("GIT_INDEX_FILE", index_file)
            .env("GIT_OBJECT_DIRECTORY", object_directory)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_SYSTEM", system_config)
            .env("GIT_CONFIG_GLOBAL", global_config)
            .env("GIT_CONFIG_COUNT", "0")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", &xdg_config_home);
        #[cfg(windows)]
        command
            .inner
            .env("APPDATA", &home)
            .env("PROGRAMDATA", &home)
            .env("USERPROFILE", &home);
        command.inner.envs(crate::local_only_git_env());
        harden_git_launch_environment(&mut command.inner, &self.safe_path);
        self.output_after_isolated_child_validation(
            &mut command.inner,
            isolated,
            [&git_dir, &common_dir, &worktree, &isolated_root],
        )
    }

    fn output_after_isolated_child_validation(
        &self,
        command: &mut Command,
        isolated: &IsolatedGitCommonDir,
        child_directories: [&GitChildDirectory; 4],
    ) -> io::Result<std::process::Output> {
        self.revalidate_active_repository_metadata()?;
        isolated.validate()?;
        let _child_directory_validations = child_directories
            .into_iter()
            .map(GitChildDirectory::revalidate)
            .collect::<io::Result<Vec<_>>>()?;
        command.output()
    }

    fn revalidate_active_repository_metadata(&self) -> io::Result<()> {
        self.authority.revalidate_active_repository_metadata()
    }

    fn from_search_path(
        authority: RepositoryAuthority,
        search_path: &OsStr,
    ) -> Result<Self, GitReadError> {
        authority.ensure_primary_authority()?;
        let selected = select_git_executable(&authority, search_path)?;
        let config_environment = GitConfigEnvironmentSnapshot::capture().map_err(|error| {
            GitReadError::InvalidConfigEnvironment {
                reason: error.to_string(),
            }
        })?;
        Ok(Self {
            executable: selected.executable,
            #[cfg(any(unix, test))]
            argv0: selected.argv0,
            safe_path: selected.safe_path,
            authority,
            config_environment,
        })
    }
}

struct GitChildDirectory {
    child_path: PathBuf,
    #[cfg(windows)]
    canonical_path: PathBuf,
    #[cfg(windows)]
    canonical_identity: Handle,
    #[cfg(windows)]
    child_identity: Handle,
    #[cfg(windows)]
    description: &'static str,
}

struct GitChildDirectoryValidation {
    #[cfg(windows)]
    _canonical_identity: Handle,
    #[cfg(windows)]
    _child_identity: Handle,
}

impl GitChildDirectory {
    fn as_path(&self) -> &Path {
        &self.child_path
    }

    fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.child_path.join(path)
    }

    #[cfg(windows)]
    fn new(
        canonical_path: PathBuf,
        child_path: PathBuf,
        description: &'static str,
    ) -> io::Result<Self> {
        let canonical_metadata = std::fs::metadata(&canonical_path)?;
        let child_metadata = std::fs::metadata(&child_path)?;
        if !canonical_metadata.is_dir() || !child_metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{description} is not an existing directory"),
            ));
        }
        let canonical_identity = Handle::from_path(&canonical_path)?;
        let child_identity = Handle::from_path(&child_path)?;
        if canonical_identity != child_identity {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("Git-compatible spelling changed the identity of {description}"),
            ));
        }
        Ok(Self {
            child_path,
            canonical_path,
            canonical_identity,
            child_identity,
            description,
        })
    }

    #[cfg(windows)]
    fn revalidate(&self) -> io::Result<GitChildDirectoryValidation> {
        let canonical_metadata = std::fs::metadata(&self.canonical_path)?;
        let child_metadata = std::fs::metadata(&self.child_path)?;
        if !canonical_metadata.is_dir() || !child_metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} is no longer an existing directory", self.description),
            ));
        }
        let canonical_identity = Handle::from_path(&self.canonical_path)?;
        let child_identity = Handle::from_path(&self.child_path)?;
        if canonical_identity != self.canonical_identity
            || child_identity != self.child_identity
            || canonical_identity != child_identity
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "Git-compatible spelling no longer identifies the original {}",
                    self.description
                ),
            ));
        }
        Ok(GitChildDirectoryValidation {
            _canonical_identity: canonical_identity,
            _child_identity: child_identity,
        })
    }

    #[cfg(not(windows))]
    fn revalidate(&self) -> io::Result<GitChildDirectoryValidation> {
        Ok(GitChildDirectoryValidation {})
    }
}

#[cfg(windows)]
fn git_child_directory(path: &Path, description: &'static str) -> io::Result<GitChildDirectory> {
    let child_path = git_compatible_windows_path(path, description)?;
    GitChildDirectory::new(path.to_path_buf(), child_path, description)
}

#[cfg(not(windows))]
fn git_child_directory(path: &Path, _description: &'static str) -> io::Result<GitChildDirectory> {
    Ok(GitChildDirectory {
        child_path: path.to_path_buf(),
    })
}

#[cfg(windows)]
fn git_compatible_windows_path(path: &Path, description: &'static str) -> io::Result<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::ffi::OsStringExt;

    let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
    match classify_windows_git_path_units(&units) {
        WindowsGitPathUnits::Unchanged => Ok(path.to_path_buf()),
        WindowsGitPathUnits::Converted(simplified) => {
            Ok(PathBuf::from(OsString::from_wide(&simplified)))
        }
        WindowsGitPathUnits::Rejected => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("refusing unsupported Windows namespace for {description}"),
        )),
    }
}

#[cfg(any(windows, test))]
#[derive(Debug, Eq, PartialEq)]
enum WindowsGitPathUnits {
    Unchanged,
    Converted(Vec<u16>),
    Rejected,
}

#[cfg(any(windows, test))]
fn classify_windows_git_path_units(path: &[u16]) -> WindowsGitPathUnits {
    const BACKSLASH: u16 = b'\\' as u16;
    const VERBATIM_PREFIX: [u16; 4] = [BACKSLASH, BACKSLASH, b'?' as u16, BACKSLASH];
    const DEVICE_PREFIX: [u16; 4] = [BACKSLASH, BACKSLASH, b'.' as u16, BACKSLASH];
    const NT_PREFIX: [u16; 4] = [BACKSLASH, b'?' as u16, b'?' as u16, BACKSLASH];
    const FORWARD_VERBATIM_PREFIX: [u16; 4] = [b'/' as u16, b'/' as u16, b'?' as u16, b'/' as u16];
    const FORWARD_DEVICE_PREFIX: [u16; 4] = [b'/' as u16, b'/' as u16, b'.' as u16, b'/' as u16];

    if path.contains(&0) {
        return WindowsGitPathUnits::Rejected;
    }
    if path.starts_with(&DEVICE_PREFIX)
        || path.starts_with(&NT_PREFIX)
        || path.starts_with(&FORWARD_VERBATIM_PREFIX)
        || path.starts_with(&FORWARD_DEVICE_PREFIX)
    {
        return WindowsGitPathUnits::Rejected;
    }
    let Some(path) = path.strip_prefix(&VERBATIM_PREFIX) else {
        return WindowsGitPathUnits::Unchanged;
    };

    if path.len() >= 3
        && ascii_u16_is_alphabetic(path[0])
        && path[1] == b':' as u16
        && path[2] == BACKSLASH
    {
        return WindowsGitPathUnits::Converted(path.to_vec());
    }

    if path.len() < 4
        || !ascii_u16_eq_ignore_case(path[0], /*ascii*/ b'U')
        || !ascii_u16_eq_ignore_case(path[1], /*ascii*/ b'N')
        || !ascii_u16_eq_ignore_case(path[2], /*ascii*/ b'C')
        || path[3] != BACKSLASH
    {
        return WindowsGitPathUnits::Rejected;
    }
    let unc_path = &path[4..];
    let Some(server_end) = unc_path.iter().position(|unit| *unit == BACKSLASH) else {
        return WindowsGitPathUnits::Rejected;
    };
    if server_end == 0 {
        return WindowsGitPathUnits::Rejected;
    }
    let share = &unc_path[server_end + 1..];
    let share_end = share
        .iter()
        .position(|unit| *unit == BACKSLASH)
        .unwrap_or(share.len());
    if share_end == 0 {
        return WindowsGitPathUnits::Rejected;
    }

    let mut simplified = Vec::with_capacity(unc_path.len() + 2);
    simplified.extend([BACKSLASH, BACKSLASH]);
    simplified.extend_from_slice(unc_path);
    WindowsGitPathUnits::Converted(simplified)
}

#[cfg(any(windows, test))]
fn ascii_u16_is_alphabetic(unit: u16) -> bool {
    (b'A' as u16..=b'Z' as u16).contains(&unit) || (b'a' as u16..=b'z' as u16).contains(&unit)
}

#[cfg(any(windows, test))]
fn ascii_u16_eq_ignore_case(unit: u16, ascii: u8) -> bool {
    unit == ascii.to_ascii_lowercase() as u16 || unit == ascii.to_ascii_uppercase() as u16
}

fn scrub_repository_and_config_environment(command: &mut Command) {
    let mut names = std::env::vars_os()
        .map(|(name, _)| name)
        .filter(|name| isolated_launch_variable(name))
        .collect::<Vec<_>>();
    names.extend(
        command
            .get_envs()
            .filter(|&(name, _)| isolated_launch_variable(name))
            .map(|(name, _)| name.to_os_string()),
    );
    names.sort();
    names.dedup();
    for name in names {
        command.env_remove(name);
    }
}

fn isolated_launch_variable(name: &OsStr) -> bool {
    let name = name.to_string_lossy().to_ascii_uppercase();
    matches!(
        name.as_str(),
        "GIT_DIR"
            | "GIT_COMMON_DIR"
            | "GIT_WORK_TREE"
            | "GIT_INDEX_FILE"
            | "GIT_INDEX_VERSION"
            | "GIT_OBJECT_DIRECTORY"
            | "GIT_ALTERNATE_OBJECT_DIRECTORIES"
            | "GIT_NAMESPACE"
            | "GIT_QUARANTINE_PATH"
            | "GIT_GRAFT_FILE"
            | "GIT_SHALLOW_FILE"
            | "GIT_REPLACE_REF_BASE"
            | "GIT_NO_REPLACE_OBJECTS"
            | "GIT_ATTR_SOURCE"
            | "GIT_ATTR_NOSYSTEM"
            | "GIT_CONFIG"
            | "GIT_CONFIG_GLOBAL"
            | "GIT_CONFIG_SYSTEM"
            | "GIT_CONFIG_NOSYSTEM"
            | "GIT_CONFIG_COUNT"
            | "GIT_CONFIG_PARAMETERS"
            | "GIT_DEFAULT_HASH"
            | "GIT_DEFAULT_REF_FORMAT"
            | "GIT_REFERENCE_BACKEND"
            | "HOME"
            | "XDG_CONFIG_HOME"
            | "APPDATA"
            | "PROGRAMDATA"
            | "USERPROFILE"
            | "HOMEDRIVE"
            | "HOMEPATH"
    ) || name.starts_with("GIT_CONFIG_KEY_")
        || name.starts_with("GIT_CONFIG_VALUE_")
}

#[cfg(test)]
thread_local! {
    static GIT_RUNNER_CONSTRUCTION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_git_runner_construction_count() {
    GIT_RUNNER_CONSTRUCTION_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn git_runner_construction_count() -> usize {
    GIT_RUNNER_CONSTRUCTION_COUNT.with(std::cell::Cell::get)
}

pub(crate) fn repository_authority_for_cwd(
    cwd: &Path,
) -> Result<RepositoryAuthority, GitReadError> {
    RepositoryAuthority::discover(cwd)
}

#[cfg(test)]
#[path = "git_command_tests.rs"]
mod tests;
