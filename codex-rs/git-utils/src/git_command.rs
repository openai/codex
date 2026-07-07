use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;

#[cfg(windows)]
use same_file::Handle;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;

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
use crate::guarded_config::GuardedOperationIdentity;
use crate::repository_authority::RepositoryAuthority;
#[cfg(test)]
use crate::repository_authority::parse_marker_path as parse_git_marker_path;
use crate::safe_git::DISABLED_HOOKS_PATH;
use crate::safe_git::isolate_git_command_environment;

pub(crate) const MAX_INTERNAL_GIT_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

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
    replace_ref_base: Option<OsString>,
    no_replace_objects: Option<OsString>,
    attr_source: Option<OsString>,
    attr_nosystem: Option<OsString>,
    identity: Arc<GitRunnerIdentity>,
}

#[derive(Debug)]
struct GitRunnerIdentity;

/// A Git command that can only be spawned through [`GitRunner::output`],
/// keeping metadata revalidation and launch hardening at one choke point.
pub(crate) struct GitCommand {
    inner: Command,
}

/// A Tokio Git command that retains the same authority and launch-hardening
/// choke point as [`GitCommand`]. Callers can configure arguments and stdin,
/// but cannot replace the authorized process cwd.
pub(crate) struct GitAsyncCommand {
    inner: tokio::process::Command,
    stdin_configured: bool,
}

/// App-owned common repository metadata for one final three-way apply.
///
/// The real per-worktree Git directory remains selected for HEAD and index
/// state, while this directory replaces every common config and attribute
/// source that could define a repository-selected executable helper.
pub(crate) struct IsolatedGitCommonDir {
    root: tempfile::TempDir,
}

/// Operation-owned writable state for a nonmutating merge-classification
/// probe. The active index is copied here, generated blobs are written only
/// to this object directory, and the real object database is attached as one
/// read-only, authority-derived alternate by `GitRunner`.
pub(crate) struct IsolatedGitStorage {
    root: tempfile::TempDir,
}

/// Operation-owned repository view for read-only status children.
///
/// HEAD, config, and attributes are frozen into owned files. The live index
/// remains selected through the authority-pinned per-worktree Git directory,
/// and the active object database is attached only as an authority-derived
/// alternate. The owned common directory supplies the complete config and
/// attribute view, so a final child cannot reload a late
/// repository-selected helper namespace.
pub(crate) struct IsolatedGitReadContext {
    common: IsolatedGitCommonDir,
    head_contents: Box<[u8]>,
    runner_identity: Arc<GitRunnerIdentity>,
    operation_identity: GuardedOperationIdentity,
    config_contents: Option<Box<[u8]>>,
    attributes_contents: Option<Box<[u8]>>,
}

impl IsolatedGitStorage {
    pub(crate) fn index_path(&self) -> PathBuf {
        self.root.path().join("index")
    }

    fn objects_path(&self) -> PathBuf {
        self.root.path().join("objects")
    }

    fn validate(&self) -> io::Result<()> {
        let index = self.index_path();
        let metadata = std::fs::symlink_metadata(&index)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("isolated Git scratch index changed at {}", index.display()),
            ));
        }
        let objects = self.objects_path();
        let metadata = std::fs::symlink_metadata(&objects)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "isolated Git scratch object directory changed at {}",
                    objects.display()
                ),
            ));
        }
        Ok(())
    }
}

impl IsolatedGitReadContext {
    pub(crate) fn config_path(&self) -> PathBuf {
        self.common.config_path()
    }

    pub(crate) fn attributes_path(&self) -> PathBuf {
        self.common.attributes_path()
    }

    pub(crate) fn seal_projected_files(mut self) -> io::Result<Self> {
        self.config_contents = Some(std::fs::read(self.config_path())?.into_boxed_slice());
        self.attributes_contents = Some(std::fs::read(self.attributes_path())?.into_boxed_slice());
        self.validate()?;
        Ok(self)
    }

    fn validate(&self) -> io::Result<()> {
        self.common.validate()?;
        let head_path = self.common.root.path().join("HEAD");
        let metadata = std::fs::symlink_metadata(&head_path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("isolated Git HEAD changed at {}", head_path.display()),
            ));
        }
        if std::fs::read(&head_path)? != self.head_contents.as_ref() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated Git HEAD contents changed",
            ));
        }
        if let Some(expected) = &self.config_contents
            && std::fs::read(self.config_path())? != expected.as_ref()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated Git config contents changed",
            ));
        }
        if let Some(expected) = &self.attributes_contents
            && std::fs::read(self.attributes_path())? != expected.as_ref()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated Git attributes contents changed",
            ));
        }
        Ok(())
    }
}

impl IsolatedGitCommonDir {
    pub(crate) fn config_path(&self) -> PathBuf {
        self.root.path().join("config")
    }

    pub(crate) fn attributes_path(&self) -> PathBuf {
        self.root.path().join("info/attributes")
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

impl GitAsyncCommand {
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
        self.stdin_configured = true;
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
        match &self.replace_ref_base {
            Some(value) => command.env("GIT_REPLACE_REF_BASE", value),
            None => command.env_remove("GIT_REPLACE_REF_BASE"),
        };
        match &self.no_replace_objects {
            Some(value) => command.env("GIT_NO_REPLACE_OBJECTS", value),
            None => command.env_remove("GIT_NO_REPLACE_OBJECTS"),
        };
        match &self.attr_source {
            Some(value) => command.env("GIT_ATTR_SOURCE", value),
            None => command.env_remove("GIT_ATTR_SOURCE"),
        };
        match &self.attr_nosystem {
            Some(value) => command.env("GIT_ATTR_NOSYSTEM", value),
            None => command.env_remove("GIT_ATTR_NOSYSTEM"),
        };
        GitCommand { inner: command }
    }

    pub(crate) fn command_for_cwd(&self, cwd: &Path) -> io::Result<GitCommand> {
        let cwd = self.canonical_command_cwd(cwd)?;
        let mut command = self.command();
        command.arg("-C").arg(cwd);
        Ok(command)
    }

    fn async_command(&self) -> GitAsyncCommand {
        GitAsyncCommand {
            inner: tokio::process::Command::from(self.command().inner),
            stdin_configured: false,
        }
    }

    pub(crate) fn async_command_for_cwd(&self, cwd: &Path) -> io::Result<GitAsyncCommand> {
        let cwd = self.canonical_command_cwd(cwd)?;
        let mut command = self.async_command();
        command.arg("-C").arg(cwd);
        Ok(command)
    }

    pub(crate) fn async_config_file_write_command(&self, config_path: &Path) -> GitAsyncCommand {
        let mut command = self.async_command();
        scrub_repository_and_config_environment(command.inner.as_std_mut());
        command
            .env("GIT_OPTIONAL_LOCKS", "0")
            .args(["config", "--file"])
            .arg(config_path)
            .arg("--add");
        command
    }

    fn canonical_command_cwd(&self, cwd: &Path) -> io::Result<PathBuf> {
        let cwd = if cwd.is_absolute() {
            cwd.to_path_buf()
        } else {
            std::env::current_dir()?.join(cwd)
        };
        self.authority.canonical_command_cwd(&cwd)
    }

    pub(crate) fn ensure_config_source_is_not_worktree_controlled(
        &self,
        path: &Path,
        description: &str,
    ) -> io::Result<()> {
        self.authority
            .ensure_config_source_is_not_worktree_controlled(path, description)
    }

    pub(crate) fn active_worktree_root(&self) -> Option<&Path> {
        self.authority.active_worktree_root()
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

    pub(crate) fn replacement_ref_base_is_custom(&self) -> bool {
        self.replace_ref_base.is_some()
    }

    pub(crate) fn replacement_refs_are_disabled(&self) -> bool {
        self.no_replace_objects.is_some()
    }

    pub(crate) fn attribute_source_is_custom(&self) -> bool {
        self.attr_source.is_some()
    }

    /// Read repository-format keys from the authority-bound common config.
    /// The common-directory identity remains pinned through the fixed
    /// `--file --no-includes` child and is revalidated after it exits.
    pub(crate) fn read_active_common_config_without_includes(
        &self,
        pattern: &str,
        show_scope: bool,
    ) -> io::Result<(std::process::Output, PathBuf)> {
        self.authority.revalidate_active_repository_metadata()?;
        let common_dir = self.authority.active_common_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git common config is unavailable",
            )
        })?;
        let pinned_common = git_child_directory(common_dir, "active Git common directory")?;
        let config_path = pinned_common.join("config");
        // The absolute --file selector needs no repository cwd. Avoiding one
        // also prevents Git setup from interpreting unrelated repository
        // config before the fixed, no-includes query runs.
        let mut command = self.command();
        command
            .env("GIT_OPTIONAL_LOCKS", "0")
            .args(["config", "--file"])
            .arg(&config_path)
            .args(["--null", "--show-origin", "--no-includes"]);
        if show_scope {
            command.arg("--show-scope");
        }
        command.args(["--get-regexp", pattern]);
        let output = self.output(command)?;
        self.authority.revalidate_active_repository_metadata()?;
        let _post_read_identity = pinned_common.revalidate()?;
        Ok((output, config_path))
    }

    pub(crate) async fn read_active_common_config_without_includes_async(
        &self,
        pattern: &str,
        show_scope: bool,
    ) -> io::Result<(std::process::Output, PathBuf)> {
        self.authority.revalidate_active_repository_metadata()?;
        let common_dir = self.authority.active_common_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git common config is unavailable",
            )
        })?;
        let pinned_common = git_child_directory(common_dir, "active Git common directory")?;
        let config_path = pinned_common.join("config");
        let mut command = self.async_command();
        scrub_repository_and_config_environment(command.inner.as_std_mut());
        command
            .env("GIT_OPTIONAL_LOCKS", "0")
            .args(["config", "--file"])
            .arg(&config_path)
            .args(["--null", "--show-origin", "--no-includes"]);
        if show_scope {
            command.arg("--show-scope");
        }
        command.args(["--get-regexp", pattern]);
        let output = self
            .output_async_bounded(command, MAX_INTERNAL_GIT_OUTPUT_BYTES)
            .await?;
        self.authority.revalidate_active_repository_metadata()?;
        let _post_read_identity = pinned_common.revalidate()?;
        Ok((output, config_path))
    }

    pub(crate) async fn read_active_info_attributes_bounded_async(
        &self,
        max_bytes: usize,
    ) -> io::Result<Vec<u8>> {
        self.revalidate_active_repository_metadata()?;
        let common_dir = self.authority.active_common_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git common attributes are unavailable",
            )
        })?;
        let contents = read_common_info_attributes_bounded_async(common_dir, max_bytes).await?;
        self.revalidate_active_repository_metadata()?;
        Ok(contents)
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

    pub(crate) fn create_isolated_git_storage(&self) -> io::Result<IsolatedGitStorage> {
        self.revalidate_active_repository_metadata()?;
        let canonical_git_dir = self.authority.active_git_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git directory is unavailable for scratch index creation",
            )
        })?;
        let git_dir = git_child_directory(canonical_git_dir, "active Git directory")?;
        let canonical_common_dir = self.authority.active_common_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git common directory is unavailable for scratch index creation",
            )
        })?;
        let common_dir = git_child_directory(canonical_common_dir, "active Git common directory")?;
        let source_index = git_dir.join("index");
        let root = tempfile::tempdir()?;
        self.authority
            .ensure_config_source_is_not_worktree_controlled(
                root.path(),
                "owned isolated Git scratch storage",
            )?;
        std::fs::create_dir(root.path().join("objects"))?;
        let isolated_index = root.path().join("index");
        match copy_regular_file_without_follow(&source_index, &isolated_index) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // An unborn repository may not have materialized an index
                // yet. Create the equivalent empty index through trusted Git
                // in operation-owned storage; an empty file is not a valid
                // index, and hand-encoding one would couple this path to the
                // repository hash algorithm and index checksum format.
                let worktree = self.authority.active_worktree_root().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "active Git worktree is unavailable for empty scratch index creation",
                    )
                })?;
                let mut command = self.command_for_cwd(worktree)?;
                let disabled_hooks = format!("core.hooksPath={DISABLED_HOOKS_PATH}");
                command.args([
                    "-c",
                    &disabled_hooks,
                    "-c",
                    "core.fsmonitor=false",
                    "read-tree",
                    "--empty",
                ]);
                let output = self.output_with_new_isolated_index(command, &isolated_index)?;
                if !output.status.success() {
                    return Err(io::Error::other(format!(
                        "failed to initialize an empty isolated Git index (status {}): {}",
                        output.status,
                        String::from_utf8_lossy(&output.stderr).trim()
                    )));
                }
            }
            Err(error) => return Err(error),
        }
        // A copied split index resolves its content-addressed base beside the
        // active per-worktree index, not in a distinct common directory. Copy
        // only well-formed names from that authoritative Git directory; Git
        // will still verify the base object ID from the link extension.
        copy_shared_indexes(git_dir.as_path(), root.path())?;
        self.revalidate_active_repository_metadata()?;
        let _post_copy_identity = git_dir.revalidate()?;
        let _post_copy_common_identity = common_dir.revalidate()?;
        let storage = IsolatedGitStorage { root };
        storage.validate()?;
        Ok(storage)
    }

    pub(crate) fn create_isolated_read_context(
        &self,
        head_oid: Option<&str>,
        operation_identity: &GuardedOperationIdentity,
    ) -> io::Result<IsolatedGitReadContext> {
        if head_oid.is_some_and(|oid| {
            !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "isolated Git read context requires a full commit object ID",
            ));
        }
        let common = self.create_isolated_common_dir()?;
        let head_contents = head_oid
            .map(|oid| format!("{}\n", oid.to_ascii_lowercase()).into_bytes())
            .unwrap_or_else(|| b"ref: refs/heads/codex-unborn\n".to_vec())
            .into_boxed_slice();
        std::fs::write(common.root.path().join("HEAD"), &head_contents)?;
        let context = IsolatedGitReadContext {
            common,
            head_contents,
            runner_identity: Arc::clone(&self.identity),
            operation_identity: operation_identity.clone(),
            config_contents: None,
            attributes_contents: None,
        };
        context.validate()?;
        Ok(context)
    }

    fn output_with_new_isolated_index(
        &self,
        mut command: GitCommand,
        index_path: &Path,
    ) -> io::Result<std::process::Output> {
        self.revalidate_active_repository_metadata()?;
        self.authority
            .ensure_config_source_is_not_worktree_controlled(
                index_path,
                "owned isolated Git scratch index",
            )?;
        isolate_git_command_environment(&mut command.inner);
        command.inner.env("GIT_INDEX_FILE", index_path);
        command.inner.env("GIT_OPTIONAL_LOCKS", "0");
        command.inner.envs(crate::local_only_git_env());
        harden_git_launch_environment(&mut command.inner, &self.safe_path);
        let output = command.inner.output()?;
        self.revalidate_active_repository_metadata()?;
        Ok(output)
    }

    pub(crate) fn output(&self, mut command: GitCommand) -> io::Result<std::process::Output> {
        self.prepare_command_for_launch(&mut command.inner)?;
        command.inner.output()
    }

    pub(crate) fn output_in_isolated_common_dir(
        &self,
        command: GitCommand,
        isolated: &IsolatedGitCommonDir,
    ) -> io::Result<std::process::Output> {
        self.output_in_isolated_common_dir_with_storage(command, isolated, /*storage*/ None)
    }

    pub(crate) fn output_in_isolated_scratch(
        &self,
        command: GitCommand,
        isolated: &IsolatedGitCommonDir,
        storage: &IsolatedGitStorage,
    ) -> io::Result<std::process::Output> {
        self.output_in_isolated_common_dir_with_storage(command, isolated, Some(storage))
    }

    /// Run against an operation-owned copy of the active index while retaining
    /// the repository's normal config and attribute sources. This is used only
    /// for nonmutating classification and `update-index --info-only` setup;
    /// callers cannot redirect the object database or common directory.
    pub(crate) fn output_with_isolated_index(
        &self,
        mut command: GitCommand,
        storage: &IsolatedGitStorage,
    ) -> io::Result<std::process::Output> {
        self.revalidate_active_repository_metadata()?;
        storage.validate()?;
        self.authority
            .ensure_config_source_is_not_worktree_controlled(
                storage.root.path(),
                "owned isolated Git scratch storage",
            )?;
        isolate_git_command_environment(&mut command.inner);
        command.inner.env("GIT_INDEX_FILE", storage.index_path());
        command.inner.envs(crate::local_only_git_env());
        harden_git_launch_environment(&mut command.inner, &self.safe_path);
        let output = command.inner.output()?;
        self.revalidate_active_repository_metadata()?;
        storage.validate()?;
        Ok(output)
    }

    fn output_in_isolated_common_dir_with_storage(
        &self,
        mut command: GitCommand,
        isolated: &IsolatedGitCommonDir,
        storage: Option<&IsolatedGitStorage>,
    ) -> io::Result<std::process::Output> {
        self.revalidate_active_repository_metadata()?;
        isolated.validate()?;
        if let Some(storage) = storage {
            storage.validate()?;
        }
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
        let canonical_worktree = self.authority.active_worktree_root().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git worktree is unavailable for isolated three-way apply",
            )
        })?;

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
        let real_object_directory = common_dir.join("objects");
        let index_file = storage
            .map(IsolatedGitStorage::index_path)
            .unwrap_or_else(|| git_dir.join("index"));
        let object_directory = storage
            .map(IsolatedGitStorage::objects_path)
            .unwrap_or_else(|| real_object_directory.clone());
        let alternate_object_directories = storage
            .map(|_| {
                std::env::join_paths([&real_object_directory]).map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("cannot encode isolated Git object alternate: {error}"),
                    )
                })
            })
            .transpose()?;
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
        if let Some(alternates) = alternate_object_directories {
            command
                .inner
                .env("GIT_ALTERNATE_OBJECT_DIRECTORIES", alternates);
        }
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
            storage,
            [&git_dir, &common_dir, &worktree, &isolated_root],
        )
    }

    fn output_after_isolated_child_validation(
        &self,
        command: &mut Command,
        isolated: &IsolatedGitCommonDir,
        storage: Option<&IsolatedGitStorage>,
        child_directories: [&GitChildDirectory; 4],
    ) -> io::Result<std::process::Output> {
        self.revalidate_active_repository_metadata()?;
        isolated.validate()?;
        if let Some(storage) = storage {
            storage.validate()?;
        }
        let _child_directory_validations = child_directories
            .into_iter()
            .map(GitChildDirectory::revalidate)
            .collect::<io::Result<Vec<_>>>()?;
        command.output()
    }

    fn revalidate_active_repository_metadata(&self) -> io::Result<()> {
        self.authority.revalidate_active_repository_metadata()
    }

    fn prepare_command_for_launch(&self, command: &mut Command) -> io::Result<()> {
        self.revalidate_active_repository_metadata()?;
        isolate_git_command_environment(command);
        command.envs(crate::local_only_git_env());
        harden_git_launch_environment(command, &self.safe_path);
        Ok(())
    }

    /// Spawn a configured Tokio command after applying the final
    /// repository-selector environment lock.
    #[cfg(all(test, unix))]
    pub(crate) async fn output_async(
        &self,
        mut command: GitAsyncCommand,
    ) -> io::Result<std::process::Output> {
        self.prepare_command_for_launch(command.inner.as_std_mut())?;
        command.inner.kill_on_drop(true);
        command.inner.output().await
    }

    pub(crate) async fn output_async_bounded(
        &self,
        mut command: GitAsyncCommand,
        max_bytes_per_stream: usize,
    ) -> io::Result<std::process::Output> {
        self.prepare_command_for_launch(command.inner.as_std_mut())?;
        output_async_bounded_inner(command, max_bytes_per_stream).await
    }

    pub(crate) async fn output_async_in_isolated_read_context(
        &self,
        mut command: GitAsyncCommand,
        context: &IsolatedGitReadContext,
        operation_identity: &GuardedOperationIdentity,
        max_bytes_per_stream: usize,
    ) -> io::Result<std::process::Output> {
        if !Arc::ptr_eq(&self.identity, &context.runner_identity) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated Git read context belongs to another runner",
            ));
        }
        if !context
            .operation_identity
            .same_operation(operation_identity)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated Git read context belongs to another operation",
            ));
        }
        if context.config_contents.is_none() || context.attributes_contents.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated Git read context is not sealed",
            ));
        }
        self.revalidate_active_repository_metadata()?;
        context.validate()?;
        self.authority
            .ensure_config_source_is_not_worktree_controlled(
                context.common.root.path(),
                "owned isolated Git read context",
            )?;
        isolate_git_command_environment(command.inner.as_std_mut());
        scrub_repository_and_config_environment(command.inner.as_std_mut());

        let canonical_common_dir = self.authority.active_common_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git common directory is unavailable for isolated read",
            )
        })?;
        let canonical_worktree = self.authority.active_worktree_root().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git worktree is unavailable for isolated read",
            )
        })?;
        let common_dir = git_child_directory(canonical_common_dir, "active Git common directory")?;
        let worktree = git_child_directory(canonical_worktree, "active Git worktree")?;
        #[cfg(windows)]
        let isolated_root_base = std::fs::canonicalize(context.common.root.path())?;
        #[cfg(not(windows))]
        let isolated_root_base = context.common.root.path().to_path_buf();
        let isolated_root =
            git_child_directory(&isolated_root_base, "owned isolated Git read directory")?;
        let canonical_git_dir = self.authority.active_git_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "active Git directory is unavailable for isolated read",
            )
        })?;
        let git_dir = git_child_directory(canonical_git_dir, "active Git directory")?;

        let real_object_directory = common_dir.join("objects");
        let alternate_object_directories =
            std::env::join_paths([&real_object_directory]).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("cannot encode isolated Git object alternate: {error}"),
                )
            })?;
        let system_config = isolated_root.join("system.gitconfig");
        let global_config = isolated_root.join("global.gitconfig");
        command
            .inner
            .env("GIT_DIR", isolated_root.as_path())
            .env("GIT_COMMON_DIR", isolated_root.as_path())
            .env("GIT_WORK_TREE", worktree.as_path())
            .env("GIT_INDEX_FILE", git_dir.join("index"))
            .env("GIT_OBJECT_DIRECTORY", isolated_root.join("objects"))
            .env(
                "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                alternate_object_directories,
            )
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_SYSTEM", system_config)
            .env("GIT_CONFIG_GLOBAL", global_config)
            .env("GIT_CONFIG_COUNT", "0")
            .env_remove("GIT_ATTR_SOURCE")
            .env("GIT_NO_REPLACE_OBJECTS", "1");
        self.apply_captured_attribute_environment(command.inner.as_std_mut());
        command.inner.envs(crate::local_only_git_env());
        harden_git_launch_environment(command.inner.as_std_mut(), &self.safe_path);

        self.revalidate_active_repository_metadata()?;
        context.validate()?;
        let _child_directory_validations = [&git_dir, &common_dir, &worktree, &isolated_root]
            .into_iter()
            .map(GitChildDirectory::revalidate)
            .collect::<io::Result<Vec<_>>>()?;
        output_async_bounded_inner(command, max_bytes_per_stream).await
    }

    /// Restore only the runner-captured selectors that choose default global
    /// and system attribute files. Config remains pinned to owned empty files;
    /// `GIT_ATTR_SOURCE` remains removed so it cannot replace worktree
    /// attributes in the synthetic repository view.
    fn apply_captured_attribute_environment(&self, command: &mut Command) {
        for name in ["HOME", "XDG_CONFIG_HOME"] {
            match self.config_environment.value(name) {
                Some(value) => command.env(name, value),
                None => command.env_remove(name),
            };
        }
        #[cfg(windows)]
        for name in [
            "APPDATA",
            "PROGRAMDATA",
            "USERPROFILE",
            "HOMEDRIVE",
            "HOMEPATH",
        ] {
            match self.config_environment.value(name) {
                Some(value) => command.env(name, value),
                None => command.env_remove(name),
            };
        }
        match &self.attr_nosystem {
            Some(value) => command.env("GIT_ATTR_NOSYSTEM", value),
            None => command.env_remove("GIT_ATTR_NOSYSTEM"),
        };
        command.env_remove("GIT_ATTR_SOURCE");
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
            replace_ref_base: std::env::var_os("GIT_REPLACE_REF_BASE"),
            no_replace_objects: std::env::var_os("GIT_NO_REPLACE_OBJECTS"),
            attr_source: std::env::var_os("GIT_ATTR_SOURCE"),
            attr_nosystem: std::env::var_os("GIT_ATTR_NOSYSTEM"),
            identity: Arc::new(GitRunnerIdentity),
        })
    }

    #[cfg(all(test, unix))]
    pub(crate) fn from_executable_for_test(
        cwd: &Path,
        executable: PathBuf,
    ) -> Result<Self, GitReadError> {
        let authority = repository_authority_for_cwd(cwd)?;
        let safe_path = std::env::var_os("PATH").ok_or(GitReadError::NoTrustedGit)?;
        let config_environment = GitConfigEnvironmentSnapshot::capture().map_err(|error| {
            GitReadError::InvalidConfigEnvironment {
                reason: error.to_string(),
            }
        })?;
        Ok(Self {
            argv0: executable.clone(),
            executable,
            safe_path,
            authority,
            config_environment,
            replace_ref_base: std::env::var_os("GIT_REPLACE_REF_BASE"),
            no_replace_objects: std::env::var_os("GIT_NO_REPLACE_OBJECTS"),
            attr_source: std::env::var_os("GIT_ATTR_SOURCE"),
            attr_nosystem: std::env::var_os("GIT_ATTR_NOSYSTEM"),
            identity: Arc::new(GitRunnerIdentity),
        })
    }
}

async fn output_async_bounded_inner(
    mut command: GitAsyncCommand,
    max_bytes_per_stream: usize,
) -> io::Result<std::process::Output> {
    command.inner.kill_on_drop(true);
    if !command.stdin_configured {
        command.inner.stdin(Stdio::null());
    }
    command.inner.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.inner.spawn()?;
    // No caller can obtain the pipe writer through this opaque command.
    // Close it immediately so a child configured with `Stdio::piped()`
    // observes EOF instead of waiting forever for unreachable input.
    drop(child.stdin.take());
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("missing bounded Git stdout pipe"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("missing bounded Git stderr pipe"))?;
    let output = tokio::try_join!(
        read_bounded_output(stdout, max_bytes_per_stream),
        read_bounded_output(stderr, max_bytes_per_stream)
    );
    let (stdout, stderr) = match output {
        Ok(output) => output,
        Err(error) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(error);
        }
    };
    let status = child.wait().await?;
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

async fn read_bounded_output(
    mut reader: impl AsyncRead + Unpin,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(read) > max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Git output exceeded the {max_bytes}-byte stream limit"),
            ));
        }
        output.extend_from_slice(&chunk[..read]);
    }
}

#[cfg(unix)]
async fn read_common_info_attributes_bounded_async(
    root: &Path,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;
    use std::os::fd::FromRawFd;
    use std::os::unix::fs::OpenOptionsExt;

    fn openat(directory: &std::fs::File, name: &str, flags: i32) -> io::Result<std::fs::File> {
        let name = CString::new(name).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Git attribute path component contains NUL",
            )
        })?;
        // SAFETY: the directory descriptor is live and the fixed component is
        // NUL-free. The returned descriptor is immediately owned by File.
        let descriptor = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if descriptor < 0 {
            Err(io::Error::last_os_error())
        } else {
            // SAFETY: openat returned a fresh owned descriptor.
            Ok(unsafe { std::fs::File::from_raw_fd(descriptor) })
        }
    }

    let root = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(root)?;
    let info = match openat(
        &root,
        "info",
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
    ) {
        Ok(info) => info,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let attributes = match openat(
        &info,
        "attributes",
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
    ) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let _pins = (root, info);
    read_regular_file_bounded_async(attributes, max_bytes, "Git info attributes").await
}

#[cfg(windows)]
async fn read_common_info_attributes_bounded_async(
    root: &Path,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    use std::fs::OpenOptions;
    use std::os::windows::fs::MetadataExt;
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;

    let open = |path: &Path, directory: bool| {
        let mut options = OpenOptions::new();
        options
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(
                FILE_FLAG_OPEN_REPARSE_POINT
                    | if directory {
                        FILE_FLAG_BACKUP_SEMANTICS
                    } else {
                        0
                    },
            );
        options.open(path)
    };
    let root_pin = open(root, true)?;
    if root_pin.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Git common directory is a reparse point",
        ));
    }
    let info_path = root.join("info");
    let info_pin = match open(&info_path, true) {
        Ok(info) => info,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    if info_pin.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Git info directory is a reparse point",
        ));
    }
    let attributes = match open(&info_path.join("attributes"), false) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    if attributes.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Git info attributes are a reparse point",
        ));
    }
    let _pins = (root_pin, info_pin);
    read_regular_file_bounded_async(attributes, max_bytes, "Git info attributes").await
}

#[cfg(not(any(unix, windows)))]
async fn read_common_info_attributes_bounded_async(
    root: &Path,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .open(root.join("info/attributes"))
    {
        Ok(file) => file,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error),
    };
    read_regular_file_bounded_async(file, max_bytes, "Git info attributes").await
}

async fn read_regular_file_bounded_async(
    file: std::fs::File,
    max_bytes: usize,
    description: &str,
) -> io::Result<Vec<u8>> {
    let file = tokio::fs::File::from_std(file);
    let metadata = file.metadata().await?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{description} are not a regular file"),
        ));
    }
    if metadata.len() > max_bytes as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{description} exceed their byte limit"),
        ));
    }
    let mut contents = Vec::with_capacity(metadata.len() as usize);
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut contents)
        .await?;
    if contents.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{description} grew beyond their byte limit"),
        ));
    }
    Ok(contents)
}

fn copy_shared_indexes(source_dir: &Path, destination_dir: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(source_dir)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Some(suffix) = name.strip_prefix("sharedindex.") else {
            continue;
        };
        if !matches!(suffix.len(), 40 | 64)
            || !suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            continue;
        }
        copy_regular_file_without_follow(&entry.path(), &destination_dir.join(name))?;
    }
    Ok(())
}

fn copy_regular_file_without_follow(source: &Path, destination: &Path) -> io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;

        // Keep the opened index entry from being deleted/replaced while its
        // bytes are copied, and open a reparse point itself so it can be
        // rejected rather than followed.
        options
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut input = options.open(source).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("active Git index is unavailable for scratch classification: {error}"),
        )
    })?;
    let metadata = input.metadata()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "active Git index is not a regular file",
        ));
    }
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    std::io::copy(&mut input, &mut output)?;
    Ok(())
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

#[cfg(windows)]
fn open_git_child_directory_identity(path: &Path) -> io::Result<Handle> {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;

    // Keep the selected directory entry pinned until the Git child exits.
    // Deliberately omit FILE_SHARE_DELETE so it cannot be renamed or replaced
    // between final identity validation and process creation.
    let directory = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    Handle::from_file(directory)
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
        let canonical_identity = open_git_child_directory_identity(&canonical_path)?;
        let child_identity = open_git_child_directory_identity(&child_path)?;
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
        let canonical_identity = open_git_child_directory_identity(&self.canonical_path)?;
        let child_identity = open_git_child_directory_identity(&self.child_path)?;
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
