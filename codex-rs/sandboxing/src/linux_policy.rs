//! Freeze environment-dependent filesystem rules before encoding the Linux helper command.

use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use std::io;
use std::os::unix::fs::MetadataExt;

/// The directory against which a relative TMPDIR was resolved.
#[derive(Debug)]
pub(super) struct BoundCwd {
    pub(super) path: AbsolutePathBuf,
    pub(super) identity: String,
}

/// Linux-only bounds on a launch after binding environment-dependent paths.
#[derive(Debug)]
pub(super) struct LinuxBinding {
    pub(super) command_cwd: Option<BoundCwd>,
    // Keep the original ceiling only when the bound policy would exceed it.
    ceiling: Option<FileSystemAccessMode>,
    original_legacy: Option<FileSystemSandboxPolicy>,
}

impl LinuxBinding {
    /// Decide once, after runtime grants have been added. Both sandbox selection
    /// and the helper use the same decision.
    pub(super) fn for_policy(self, permissions: &PermissionProfile) -> Self {
        let ceiling = self.ceiling.filter(|ceiling| {
            let policy = permissions.file_system_sandbox_policy();
            policy.has_full_disk_write_access()
                || (!ceiling.can_read() && policy.has_full_disk_read_access())
        });
        Self { ceiling, ..self }
    }

    pub(super) fn needs_encoding(&self) -> bool {
        self.ceiling.is_some()
    }
}

/// Encode a real root grant as a Linux path if binding would enable the helper's
/// full-disk shortcut. Keep this representation at the helper boundary: legacy
/// policies distinguish a writable `/` from the symbolic `:root` grant.
pub(super) fn linux_helper_permissions(
    exec: &crate::SandboxExecRequest,
    binding: Option<LinuxBinding>,
) -> io::Result<Option<PermissionProfile>> {
    let Some(binding) = binding else {
        return Ok(None);
    };
    if !binding.needs_encoding() && binding.original_legacy.is_none() {
        return Ok(None);
    }
    let granted = &exec.permission_profile;
    let mut file_system = granted.file_system_sandbox_policy();
    if let Some(ceiling) = binding.ceiling {
        let full_read = file_system.has_full_disk_read_access();
        let root = PathUri::from_host_native_path("/")?;
        for entry in &mut file_system.entries {
            if matches!(
                entry.path,
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root
                }
            ) && entry.access.can_read()
            {
                entry.path = root.clone().into();
            }
        }
        if full_read && ceiling.can_read() {
            file_system.entries.push(FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Read,
            ));
        }
    }
    let network = granted.network_sandbox_policy();
    if let Some(original) = &binding.original_legacy {
        // Older Linux helpers in Landlock mode consume a legacy projection. Require its grants
        // and metadata carveouts to survive the root encoding at the approved
        // cwd and TMPDIR; an unrepresentable original must stay unrepresentable.
        let cwd = exec.sandbox_policy_cwd.to_abs_path()?;
        let legacy_grants = |source: &FileSystemSandboxPolicy| -> io::Result<_> {
            let legacy = source.to_legacy_sandbox_policy(network, cwd.as_path())?;
            let mut projected =
                FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(&legacy, cwd.as_path());
            let _ = bind_linux_paths(
                &mut projected.entries,
                &exec.cwd,
                &exec.env,
                &exec.sandbox_policy_cwd,
            )?;
            Ok(projected)
        };
        let original_grants = legacy_grants(original)?;
        let encoded_grants = legacy_grants(&file_system)?;
        if !original_grants.is_semantically_equivalent_to(&encoded_grants, cwd.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "binding this Linux policy would change legacy Landlock grants",
            ));
        }
    }

    Ok(binding.ceiling.map(|_| {
        PermissionProfile::from_runtime_permissions_with_enforcement(
            granted.enforcement(),
            &file_system,
            network,
        )
    }))
}

/// Give the Linux helper the paths selected by the original launch environment. In particular, a
/// relative TMPDIR belongs to the command cwd, which can differ from the sandbox policy cwd.
pub(super) fn bind_linux_policy_environment(
    entries: &mut Vec<FileSystemSandboxEntry>,
    command_cwd: &PathUri,
    command_env: &std::collections::HashMap<String, String>,
    policy_cwd: &PathUri,
    legacy_landlock: bool,
) -> io::Result<Option<LinuxBinding>> {
    let original = FileSystemSandboxPolicy::restricted(std::mem::take(entries));
    // Freezing :tmpdir can make it alias an existing rule in either direction.
    // Keep the approved policy's process confinement even if its concrete grants now look broader.
    let binds_tmpdir = original.entries.iter().any(|entry| {
        matches!(
            &entry.path,
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Tmpdir
            }
        )
    });
    let full_disk_access_ceiling = if !binds_tmpdir || original.has_full_disk_write_access() {
        None
    } else if original.has_full_disk_read_access() {
        Some(FileSystemAccessMode::Read)
    } else {
        Some(FileSystemAccessMode::Deny)
    };
    let check_legacy = legacy_landlock && (binds_tmpdir || original.entries.iter().any(|entry|
        matches!(&entry.path, FileSystemPath::GlobPattern { pattern } if entry.access == FileSystemAccessMode::Deny && PathConvention::Posix.home_relative_suffix(pattern).is_some())
    ));
    // Rebinding can turn an unrepresentable legacy grant into WorkspaceWrite or
    // ReadOnly even if the full-disk classification is unchanged.
    let original_legacy = check_legacy.then(|| original.clone());
    *entries = original.entries;
    let command_cwd = bind_linux_paths(entries, command_cwd, command_env, policy_cwd)?;
    let binding = (full_disk_access_ceiling.is_some() || check_legacy || command_cwd.is_some())
        .then_some(LinuxBinding {
            ceiling: full_disk_access_ceiling,
            original_legacy,
            command_cwd,
        });
    Ok(binding)
}

fn bind_linux_paths(
    entries: &mut Vec<FileSystemSandboxEntry>,
    command_cwd: &PathUri,
    command_env: &std::collections::HashMap<String, String>,
    policy_cwd: &PathUri,
) -> io::Result<Option<BoundCwd>> {
    use codex_shell_command::shell_detect::get_user_home_path;
    use std::cell::LazyCell;
    use std::path::PathBuf;

    let posix = PathConvention::Posix;
    let invalid_path = |err| io::Error::new(io::ErrorKind::InvalidInput, err);
    let env_home = command_env.get("HOME").filter(|s| !s.is_empty());
    let home = LazyCell::new(|| env_home.map(PathBuf::from).or_else(get_user_home_path));
    let resolve = |path: PathBuf| -> io::Result<PathUri> {
        let text = path.to_str().ok_or(io::ErrorKind::InvalidInput)?;
        PathUri::validate_config_path_text(text, posix).map_err(invalid_path)?;
        PathUri::from_host_native_path(path)
    };
    let tmpdir = command_env.get("TMPDIR").filter(|s| !s.is_empty());
    if tmpdir.is_none() {
        entries.retain(|entry| {
            !matches!(
                entry.path,
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Tmpdir
                }
            )
        });
    }
    let mut resolved_cwd = None;
    for entry in entries {
        match &mut entry.path {
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Tmpdir,
            } => {
                let value = tmpdir.ok_or(io::ErrorKind::InvalidInput)?;
                let mut path = if let Some(suffix) = posix.home_relative_suffix(value)
                    && let Some(home) = home.as_ref()
                {
                    home.join(suffix.trim_start_matches('/'))
                } else {
                    PathBuf::from(value)
                };
                // getcwd in the helper follows symlinks.
                if path.is_relative() {
                    let cwd = match &resolved_cwd {
                        Some(cwd) => cwd,
                        None => {
                            let requested = command_cwd.to_abs_path()?;
                            let metadata = std::fs::metadata(&requested)?;
                            resolved_cwd.insert(BoundCwd {
                                path: requested.canonicalize()?,
                                identity: format!("{}:{}", metadata.dev(), metadata.ino()),
                            })
                        }
                    };
                    path = cwd.path.as_path().join(path);
                }
                entry.path = resolve(path)?.into();
            }
            FileSystemPath::GlobPattern { pattern }
                if entry.access == FileSystemAccessMode::Deny =>
            {
                let Some(suffix) = posix.home_relative_suffix(pattern) else {
                    continue;
                };
                let home = home.as_ref().ok_or_else(|| io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unable to resolve deny-read glob pattern `{pattern}` without executor home"),
                ))?;
                let directory = policy_cwd.to_abs_path()?.as_path().join(home);
                PathUri::from_host_native_path(&directory)?
                    .validate_glob_directory(posix)
                    .map_err(invalid_path)?;
                *pattern = resolve(directory.join(suffix.trim_start_matches('/')))?
                    .inferred_native_path_string();
            }
            _ => {}
        }
    }
    Ok(resolved_cwd)
}
