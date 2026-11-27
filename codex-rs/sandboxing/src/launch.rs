//! Bind filesystem and network inputs for sandbox preparation on the execution host.

use crate::SandboxExecRequest;
use crate::SandboxIntegrityRequest;
use crate::SandboxType;
use crate::SpawnOptions;
use crate::SpawnRequest;
use crate::WindowsSandboxSpawnRequest;
use crate::spawn::spawn_process;
use crate::windows::WindowsSessionPolicy;
use anyhow::Context;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::sandbox::SandboxOverride;
use codex_utils_pty::SpawnedProcess;
use std::collections::HashMap;
use std::io;
use std::process::Command;

use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
#[cfg(any(not(target_os = "linux"), test))]
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
use codex_utils_absolute_path::canonicalize_preserving_symlinks;
#[cfg(any(windows, test))]
use codex_utils_path_uri::LegacyAppPathString;
#[cfg(any(windows, test))]
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;

/// A command prepared for its selected sandbox. Native Windows token sessions
/// must use the Windows session launcher.
#[must_use]
pub struct WrappedSandboxCommand {
    pub(super) request: SandboxExecRequest,
    pub(super) inner_arg_count: usize,
    pub(super) windows_session: Option<Box<WindowsSessionPolicy>>,
}

impl WrappedSandboxCommand {
    /// Prepare a native command with the executor's validated working directory and child environment.
    pub fn native(
        argv: Vec<String>,
        cwd: PathUri,
        env: HashMap<String, String>,
        arg0: Option<String>,
    ) -> io::Result<Self> {
        if argv.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "argv must not be empty",
            ));
        }
        Ok(Self {
            inner_arg_count: argv.len(),
            request: SandboxExecRequest {
                sandbox_override: SandboxOverride::NoOverride,
                command: argv,
                native_program: None,
                sandbox_policy_cwd: cwd.clone(),
                cwd,
                env,
                network: None,
                network_environment_id: None,
                sandbox: SandboxType::None,
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                permission_profile: PermissionProfile::Disabled,
                arg0,
            },
            windows_session: None,
        })
    }

    /// The backend that will enforce this launch.
    pub fn backend(&self) -> SandboxType {
        self.request.sandbox
    }

    /// Inspect launch dependencies with the controller's approval observation.
    pub fn integrity_request(
        &self,
        sandbox_override: SandboxOverride,
    ) -> SandboxIntegrityRequest<'_> {
        let mut facts = SandboxIntegrityRequest::from(&self.request);
        facts.sandbox_override = sandbox_override;
        facts
    }

    /// Check whether a snapshot can capture this shell invocation inside the prepared launcher.
    pub fn supports_shell_snapshot(&self, expected: &[String]) -> bool {
        self.shell_script_index(expected).is_some()
    }

    fn shell_script_index(&self, expected: &[String]) -> Option<usize> {
        match self.request.sandbox {
            SandboxType::None | SandboxType::MacosSeatbelt | SandboxType::LinuxSeccomp => {}
            // Windows either needs a native session or encodes its workload in the environment.
            SandboxType::WindowsRestrictedToken | SandboxType::WindowsMxc => return None,
        }
        // An argv[0] helper may precede the shell inside the sandbox. Only the submitted
        // invocation can match, never a suffix reaching into the enclosing sandbox launcher.
        if expected.len() < 3 || expected.len() > self.inner_arg_count {
            return None;
        }
        let prefix_len = self.request.command.strip_suffix(expected)?.len();
        self.request.cwd.to_abs_path().ok()?;
        Some(prefix_len + 2)
    }

    /// Build a capture command by replacing only the matched shell script.
    pub fn shell_snapshot_capture_command(
        &self,
        expected: &[String],
        script: &str,
    ) -> Option<Command> {
        let index = self.shell_script_index(expected)?;
        let request = &self.request;
        let (program, _) = request.program_and_args()?;
        let mut command = Command::new(program);
        command
            .args(&request.command[1..index])
            .arg(script)
            .args(&request.command[index + 1..])
            .current_dir(request.cwd.to_abs_path().ok()?)
            .env_clear()
            .envs(&request.env);
        #[cfg(unix)]
        if let Some(arg0) = &request.arg0 {
            use std::os::unix::process::CommandExt;
            command.arg0(arg0);
        }
        Some(command)
    }

    /// Borrow the matched shell's flag, script, and environment for snapshot replay.
    pub fn shell_snapshot_replay_mut(
        &mut self,
        expected: &[String],
    ) -> Option<(&mut String, &mut String, &mut HashMap<String, String>)> {
        let index = self.shell_script_index(expected)?;
        let [flag, script] = self.request.command[index - 1..].first_chunk_mut()?;
        Some((flag, script, &mut self.request.env))
    }

    /// Spawn the command in its prepared sandbox.
    pub async fn spawn(self, options: SpawnOptions<'_>) -> anyhow::Result<SpawnedProcess> {
        let request = self.request;
        let windows_sandbox =
            self.windows_session
                .as_deref()
                .map(|session| WindowsSandboxSpawnRequest {
                    permission_profile: &request.permission_profile,
                    workspace_roots: &session.workspace_roots,
                    windows_sandbox_level: request.windows_sandbox_level,
                    proxy_enforced: session.proxy_enforced,
                    #[cfg(windows)]
                    network_proxy_restricting_sid: session.restricting_sid.as_deref(),
                    #[cfg(not(windows))]
                    network_proxy_restricting_sid: None,
                    proxy_settings_mode: session.proxy_settings_mode,
                    filesystem_overrides: session.filesystem_overrides.as_ref(),
                });
        let cwd = request.cwd.to_abs_path().context("invalid process cwd")?;
        let (program, args) = request
            .program_and_args()
            .context("missing program for process spawn")?;
        spawn_process(SpawnRequest {
            program,
            args,
            cwd: cwd.as_path(),
            env: &request.env,
            arg0: &request.arg0,
            sandbox: request.sandbox,
            windows_sandbox,
            tty: options.tty,
            stdin_open: options.stdin_open,
            inherited_fds: options.inherited_fds,
        })
        .await
    }

    /// Return the request for a caller that supplies its own process launcher.
    pub fn into_command_request(self) -> io::Result<SandboxExecRequest> {
        if self.windows_session.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "a native Windows sandbox requires its session launcher",
            ));
        }
        Ok(self.request)
    }

    /// Return a configured process command when argv carries all required sandbox enforcement.
    pub fn into_direct_command(self) -> io::Result<Command> {
        let request = self.request;
        if request.sandbox == SandboxType::WindowsRestrictedToken {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "a native Windows sandbox requires its session launcher",
            ));
        }
        let (program, args) = request.program_and_args().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "sandbox command is empty")
        })?;
        let mut command = Command::new(program);
        #[cfg(unix)]
        if let Some(arg0) = &request.arg0 {
            use std::os::unix::process::CommandExt;
            command.arg0(arg0);
        }
        command
            .args(args)
            .current_dir(request.cwd.to_abs_path()?)
            .env_clear()
            .envs(request.env);
        Ok(command)
    }
}

#[derive(Clone, Default)]
pub(crate) enum SandboxOperation {
    #[default]
    Command,
    #[cfg(target_os = "macos")]
    InheritedTerminal {
        allow_unix_sockets: Vec<AbsolutePathBuf>,
    },
    FileSystemHelper,
}

#[cfg(unix)]
pub(crate) use crate::linux_policy::LinuxBinding;
#[cfg(not(unix))]
pub(crate) type LinuxBinding = ();

/// The authorized profile, resolved on the host that will enforce it.
#[derive(Debug)]
pub(super) struct PreparedLaunchPolicy {
    pub(super) linux_binding: Option<LinuxBinding>,
    pub(super) permissions: PermissionProfile,
    #[cfg(windows)]
    pub(super) workspace_roots: Vec<AbsolutePathBuf>,
}

impl PreparedLaunchPolicy {
    /// An empty workspace list stays empty, even when the policy contains `:workspace_roots`.
    pub(super) fn resolve(
        mut permissions: PermissionProfile,
        request: &crate::SandboxTransformRequest<'_>,
        operation: &SandboxOperation,
        runtime_readable_paths: &[AbsolutePathBuf],
    ) -> io::Result<Self> {
        let is_file_system_helper = matches!(operation, SandboxOperation::FileSystemHelper);
        let policy_cwd = request.sandbox_policy_cwd;
        let roots = request.workspace_roots;
        #[cfg(unix)]
        let linux = match request.selection {
            crate::SandboxSelection::Explicit(backend) => backend == SandboxType::LinuxSeccomp,
            crate::SandboxSelection::Platform { .. } => cfg!(target_os = "linux"),
        };
        let bind =
            |_entries: &mut Vec<FileSystemSandboxEntry>| -> io::Result<Option<LinuxBinding>> {
                #[cfg(unix)]
                if linux {
                    return crate::linux_policy::bind_linux_policy_environment(
                        _entries,
                        &request.command.cwd,
                        &request.command.env,
                        policy_cwd,
                        crate::linux::uses_legacy_landlock(
                            request.use_legacy_landlock,
                            request.network.enforce_managed_network,
                        ),
                    );
                }
                Ok(None)
            };
        let Some(roots) = roots else {
            if is_file_system_helper || !runtime_readable_paths.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "runtime read access requires explicit workspace roots",
                ));
            }
            let linux_binding = match &mut permissions {
                PermissionProfile::Managed {
                    file_system:
                        codex_protocol::models::ManagedFileSystemPermissions::Restricted {
                            entries, ..
                        },
                    ..
                } => bind(entries)?,
                _ => None,
            };
            return Ok(Self {
                linux_binding,
                permissions,
                #[cfg(windows)]
                workspace_roots: Vec::new(),
            });
        };
        let cwd = native_path(policy_cwd, "sandbox policy cwd")?;
        let workspace_roots = roots
            .iter()
            .map(|root| native_path(root, "sandbox workspace root"))
            .collect::<io::Result<Vec<_>>>()?;
        let (file_system, network) = permissions.to_runtime_permissions();
        for entry in &file_system.entries {
            if let FileSystemPath::Path { path } = &entry.path {
                native_path(path, "sandbox permission path")?;
            }
        }
        let mut file_system =
            file_system.materialize_project_roots_with_workspace_roots(&workspace_roots);
        // Freeze the materialized rules before testing whether trusted helper paths need a read grant.
        let linux_binding = bind(&mut file_system.entries)?;
        let permissions = if is_file_system_helper {
            tracing::Span::current().record("permission_entries", file_system.entries.len());
            if !file_system.has_full_disk_read_access() {
                let minimal = FileSystemSandboxEntry::new(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::Minimal,
                    },
                    FileSystemAccessMode::Read,
                );
                if !file_system.entries.contains(&minimal) {
                    file_system.entries.push(minimal);
                }
            }
            // On Windows, :root can cover the policy cwd's drive but miss the helper's drive.
            for path in runtime_readable_paths {
                if !file_system.can_read_local_path_with_cwd(path, &cwd) {
                    file_system.entries.push(FileSystemSandboxEntry::new(
                        path.clone().into(),
                        FileSystemAccessMode::Read,
                    ));
                }
            }
            // Linux resolves aliases in the sandbox helper. Probing unrelated roots here would
            // also block the executor runtime thread on their filesystems.
            #[cfg(not(target_os = "linux"))]
            normalize_helper_policy_root_aliases(&mut file_system)?;
            #[cfg(windows)]
            bind_windows_cwd_relative_deny_read_globs(&mut file_system, policy_cwd)?;
            PermissionProfile::from_runtime_permissions(
                &file_system,
                NetworkSandboxPolicy::Restricted,
            )
        } else {
            let file_system =
                file_system.with_additional_readable_roots(&cwd, runtime_readable_paths);
            PermissionProfile::from_runtime_permissions_with_enforcement(
                permissions.enforcement(),
                &file_system,
                network,
            )
        };
        Ok(Self {
            linux_binding,
            permissions,
            #[cfg(windows)]
            workspace_roots,
        })
    }
}

fn native_path(path: &PathUri, field: &str) -> io::Result<AbsolutePathBuf> {
    path.to_abs_path()
        .map_err(|err| io::Error::new(err.kind(), format!("invalid {field}: {err}")))
}

/// The Windows wrapper interprets the policy from the helper's cwd, which can be a drive root.
#[cfg(any(windows, test))]
fn bind_windows_cwd_relative_deny_read_globs(
    file_system: &mut FileSystemSandboxPolicy,
    policy_cwd: &PathUri,
) -> io::Result<()> {
    let windows = PathConvention::Windows;
    for entry in &mut file_system.entries {
        if let FileSystemPath::GlobPattern { pattern } = &mut entry.path
            && entry.access == FileSystemAccessMode::Deny
            && windows.home_relative_suffix(pattern).is_none()
            && LegacyAppPathString::from_string(pattern.as_str())
                .to_path_uri(windows)
                .is_err()
        {
            let path = policy_cwd
                .validate_glob_directory(windows)
                .and_then(|()| policy_cwd.join(pattern.as_str()).map_err(Into::into))
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
            *pattern = path.inferred_native_path_string();
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn normalize_helper_policy_root_aliases(
    file_system: &mut FileSystemSandboxPolicy,
) -> io::Result<()> {
    for entry in &mut file_system.entries {
        if let FileSystemPath::Path { path } = &mut entry.path
            && let Ok(native) = path.to_abs_path()
        {
            #[cfg(target_os = "macos")]
            let normalized = native.normalize_system_aliases().map_err(|err| {
                io::Error::new(err.kind(), format!("failed to normalize {path}: {err}"))
            })?;
            #[cfg(not(target_os = "macos"))]
            let normalized = normalize_top_level_alias(native);
            *path = normalized.into();
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn normalize_top_level_alias(path: AbsolutePathBuf) -> AbsolutePathBuf {
    let raw = path.as_path();
    for ancestor in raw.ancestors() {
        if std::fs::symlink_metadata(ancestor).is_ok()
            && let Ok(normalized) = canonicalize_preserving_symlinks(ancestor)
            && normalized != ancestor
            && let Ok(suffix) = raw.strip_prefix(ancestor)
            && let Ok(normalized) = AbsolutePathBuf::from_absolute_path(normalized.join(suffix))
        {
            return normalized;
        }
    }
    path
}

/// Tilde patterns stay home-relative when Windows sandbox commands start at another cwd.
#[cfg(test)]
#[test]
fn windows_cwd_relative_denial_binding_keeps_tilde_patterns_home_relative() {
    let cwd = PathUri::parse("file:///C:/policy/checkout").expect("Windows policy cwd");
    let deny_glob = |pattern: &str| {
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
            FileSystemPath::GlobPattern {
                pattern: pattern.to_owned(),
            },
            FileSystemAccessMode::Deny,
        )])
    };
    for (pattern, expected) in [
        ("~", "~"),
        ("~/", "~/"),
        (r"~\", r"~\"),
        ("~/private/*.env", "~/private/*.env"),
        (r"~\private\*.env", r"~\private\*.env"),
        ("hidden/*.env", r"C:\policy\checkout\hidden\*.env"),
        ("~someone/private", r"C:\policy\checkout\~someone\private"),
        ("D:/absolute/*.env", "D:/absolute/*.env"),
    ] {
        let mut policy = deny_glob(pattern);
        bind_windows_cwd_relative_deny_read_globs(&mut policy, &cwd).expect("bind Windows policy");
        pretty_assertions::assert_eq!(policy, deny_glob(expected));
    }
}
