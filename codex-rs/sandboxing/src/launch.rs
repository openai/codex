//! Bind filesystem and network inputs for sandbox preparation on the execution host.

use crate::SandboxExecRequest;
use crate::SandboxType;
use std::ffi::OsStr;
use std::ffi::OsString;
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
    // UTF-8 argv cannot represent every native executable path.
    pub(super) native_program: Option<OsString>,
}

impl WrappedSandboxCommand {
    /// Return the request with UTF-8 argv for callers that spawn the command themselves.
    pub fn into_command_request(self) -> SandboxExecRequest {
        self.request
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
        let (program, args) = request.command.split_first().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "sandbox command is empty")
        })?;
        let mut command = Command::new(
            self.native_program
                .as_deref()
                .unwrap_or_else(|| OsStr::new(program)),
        );
        #[cfg(unix)]
        if let Some(arg0) = request.arg0 {
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
pub(super) enum SandboxOperation {
    #[default]
    Command,
    #[cfg(target_os = "macos")]
    InheritedTerminal {
        allow_unix_sockets: Vec<AbsolutePathBuf>,
    },
    FileSystemHelper,
}

/// The authorized profile, resolved on the host that will enforce it.
#[derive(Debug)]
pub(super) struct PreparedLaunchPolicy {
    pub(super) permissions: PermissionProfile,
    #[cfg(windows)]
    pub(super) workspace_roots: Vec<AbsolutePathBuf>,
}

impl PreparedLaunchPolicy {
    /// An empty workspace list stays empty, even when the policy contains `:workspace_roots`.
    pub(super) fn resolve(
        permissions: PermissionProfile,
        policy_cwd: &PathUri,
        roots: Option<&[PathUri]>,
        runtime_readable_paths: &[AbsolutePathBuf],
        operation: &SandboxOperation,
    ) -> io::Result<Self> {
        let is_file_system_helper = matches!(operation, SandboxOperation::FileSystemHelper);
        let Some(roots) = roots else {
            if is_file_system_helper || !runtime_readable_paths.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "runtime read access requires explicit workspace roots",
                ));
            }
            return Ok(Self {
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
