//! Translate canonical permissions into MXC path grants.
//! Denies and read-only carveouts remain separate kernel policy lists.
//! Missing deny parents are created and marked before deny globs and volume grants are expanded.
//! Writable volume roots expand only when no restrictive alias names the same directory.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::permissions::FileSystemPolicyProjection;
use codex_protocol::protocol::FileSystemAccessMode;
use codex_protocol::protocol::FileSystemPath;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use codex_protocol::protocol::FileSystemSpecialPath;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use codex_windows_sandbox::cmd_script_command_line;
use codex_windows_sandbox::expand_windows_deny_read_paths;
use codex_windows_sandbox::resolve_windows_deny_read_paths;
use mxc_sdk::mxc_common::cmdline::CommandLineContext;
use mxc_sdk::mxc_common::cmdline::CommandLineError;
use mxc_sdk::mxc_common::cmdline::cmdline_from_argv_for_context;
use mxc_sdk::mxc_common::filesystem_object::ExistingObjectComparison;
use mxc_sdk::mxc_common::filesystem_object::compare_existing_filesystem_objects;
use mxc_sdk::v1::ContainerRequest;
use mxc_sdk::v1::Containment;
use mxc_sdk::v1::FilesystemPolicy;
use mxc_sdk::v1::NetworkAction;
use mxc_sdk::v1::NetworkEgressPolicy;
use mxc_sdk::v1::NetworkIngressPolicy;
use mxc_sdk::v1::NetworkPeerPolicy;
use mxc_sdk::v1::NetworkPolicy;
use mxc_sdk::v1::NetworkRulePolicy;
use mxc_sdk::v1::UiPolicy;
use mxc_sdk::v1::configs::ProcessContainerConfig;
use mxc_sdk::v1::configs::ProcessContainerUi;
use mxc_sdk::v1::configs::ProcessContainerUiIsolation;
use thiserror::Error;

use crate::MxcCommand;

// Use the same Windows case/URI identity as the canonical permission model,
// while retaining native spellings for the OS API.
type NativePaths = HashMap<PathUri, PathBuf>;

/// Invalid inputs or unsupported permissions encountered during policy translation.
#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("MXC command must not be empty")]
    EmptyCommand,
    #[error("MXC requires an absolute policy working directory")]
    RelativePolicyCwd,
    #[error("MXC requires an absolute command working directory")]
    RelativeCommandCwd,
    #[error("MXC policy contains an unresolved symbolic filesystem path")]
    UnsupportedSymbolicPath,
    #[error("MXC requires a Unicode command working directory")]
    NonUnicodeCommandCwd,
    #[error("MXC requires Unicode filesystem policy paths")]
    NonUnicodePolicyPath,
    #[error("{0}")]
    PolicyResolution(String),
    #[error("enumerate MXC volume {path}: {source}")]
    EnumerateVolume {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "cannot determine whether writable MXC volume {volume} aliases restricted directory {restriction}"
    )]
    IndeterminateVolumeAlias {
        volume: PathBuf,
        restriction: PathBuf,
    },
    #[error("writable MXC volume {volume} is also denied through {denial}")]
    ConflictingVolumeDenial { volume: PathBuf, denial: PathBuf },
    #[error(transparent)]
    Path(#[from] std::io::Error),
    #[error(transparent)]
    CommandLine(#[from] CommandLineError),
}

/// Bind MXC's temporary-directory symbols using the filtered command environment.
pub(super) fn materialize_temporary_paths(
    mut policy: FileSystemSandboxPolicy,
    env: &[String],
) -> std::io::Result<FileSystemSandboxPolicy> {
    // Resolve Windows temporary directories from the filtered command
    // environment, including case-insensitive names.
    let mut temp_values = HashMap::new();
    for (key, value) in env.iter().filter_map(|entry| entry.split_once('=')) {
        if key.eq_ignore_ascii_case("TEMP") || key.eq_ignore_ascii_case("TMP") {
            temp_values.insert(key.to_ascii_uppercase(), value);
        }
    }
    let temp_paths = ["TEMP", "TMP"]
        .into_iter()
        .filter_map(|key| temp_values.get(key).copied())
        .filter(|path| Path::new(path).is_absolute())
        .map(AbsolutePathBuf::from_absolute_path)
        .collect::<std::io::Result<Vec<_>>>()?;
    policy.entries = policy
        .entries
        .into_iter()
        .flat_map(|entry| {
            match &entry.path {
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Tmpdir,
                } => temp_paths
                    .iter()
                    .map(|path| {
                        let mut entry = entry.clone();
                        entry.path = path.clone().into();
                        entry
                    })
                    .collect(),
                // /tmp has no special meaning on the Windows executor.
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::SlashTmp,
                } => Vec::new(),
                _ => vec![entry],
            }
        })
        .collect();
    Ok(policy)
}

pub(super) fn build_request(
    command: &MxcCommand,
    command_cwd: &Path,
    env: Vec<String>,
    volume_roots: &[PathBuf],
    platform_read_roots: &[PathBuf],
) -> Result<ContainerRequest, PolicyError> {
    if command.command.is_empty() {
        return Err(PolicyError::EmptyCommand);
    }
    let permissions = &command.permissions;
    let cwd = &command.sandbox_policy_cwd;
    if !cwd.is_absolute() {
        return Err(PolicyError::RelativePolicyCwd);
    }
    if !command_cwd.is_absolute() {
        return Err(PolicyError::RelativeCommandCwd);
    }
    let policy = materialize_temporary_paths(permissions.file_system_sandbox_policy(), &env)?;
    #[cfg(windows)]
    crate::windows::log_launch("phase=prepare_deny_parents");
    #[cfg(windows)]
    for denied in policy.get_unreadable_roots_with_cwd(cwd) {
        let Some(parent) = denied.as_path().parent() else {
            continue;
        };
        if parent.components().any(|component| {
            component
                .as_os_str()
                .to_string_lossy()
                .trim_end_matches(['.', ' '])
                .eq_ignore_ascii_case(".chatgptcreated")
        }) {
            return Err(PolicyError::PolicyResolution(format!(
                "MXC deny path {} uses the reserved directory name .chatgptcreated",
                denied.display()
            )));
        }
        let mut missing = parent
            .ancestors()
            .take_while(|path| matches!(path.try_exists(), Ok(false)))
            .collect::<Vec<_>>();
        missing.reverse();
        for directory in missing {
            // Only mark directories this launcher actually created.
            match std::fs::create_dir(directory) {
                Ok(()) => {}
                Err(error)
                    if error.kind() == std::io::ErrorKind::AlreadyExists && directory.is_dir() =>
                {
                    continue;
                }
                Err(error) => {
                    return Err(PolicyError::PolicyResolution(format!(
                        "MXC currently requires deny paths to have existing parent directories. Could not create {}: {error}. Create this directory manually or correct the path/permissions, then retry.",
                        directory.display()
                    )));
                }
            }
            let sentinel = directory.join(".chatgptcreated");
            std::fs::File::create_new(&sentinel).map_err(|error| {
                // Leave a failed preparation retryable when the directory is still empty.
                let _ = std::fs::remove_dir(directory);
                PolicyError::PolicyResolution(format!(
                    "cannot create MXC directory sentinel {}: {error}",
                    sentinel.display()
                ))
            })?;
        }
    }
    let full_disk_write = policy.has_full_disk_write_access();
    #[cfg(windows)]
    crate::windows::log_launch("phase=resolve_filesystem_policy");
    let volumes = volume_roots
        .iter()
        .map(PathUri::from_host_native_path)
        .collect::<std::io::Result<Vec<_>>>()?;
    #[cfg(windows)]
    let read_volumes = crate::windows::implicit_read_roots(volume_roots, cwd, command_cwd)?;
    #[cfg(not(windows))]
    let read_volumes = volumes.clone();
    let fs = materialize_volume_roots(policy.clone(), &volumes, &read_volumes)?;
    let absolute_cwd = AbsolutePathBuf::from_absolute_path(cwd)
        .map_err(|error| PolicyError::PolicyResolution(error.to_string()))?;
    let (roots, denied_paths) = if fs == policy {
        let resolved =
            fs.resolve_for_enforcement(cwd, FileSystemPolicyProjection::PreserveMutableRoots)?;
        #[cfg(windows)]
        crate::windows::log_launch("phase=resolve_deny_paths");
        let denies = expand_windows_deny_read_paths(&resolved, &absolute_cwd);
        (resolved.writable_roots, denies)
    } else {
        // A symbolic root deny sets the default on every volume. Expanding it into ACL
        // targets would mask narrower grants and probe disconnected drives.
        let roots = if full_disk_write {
            Vec::new()
        } else {
            fs.get_writable_roots_with_cwd_preserving_mutable_paths(cwd)
        };
        #[cfg(windows)]
        crate::windows::log_launch("phase=resolve_deny_paths");
        (
            roots,
            resolve_windows_deny_read_paths(&policy, &absolute_cwd),
        )
    };
    let mut write = collect_paths(roots.iter().map(|root| root.root.to_path_buf()))?;
    if full_disk_write {
        write.extend(collect_paths(volume_roots.iter().cloned())?);
        // Explicit grants may name unmapped shares outside the enumerated volumes.
        for entry in &policy.entries {
            if entry.access.can_write()
                && let FileSystemPath::Path { path } = &entry.path
            {
                write.insert(path.clone(), path.to_abs_path()?.into_path_buf());
            }
        }
    }
    // All symbolic read roots have been bound above. Keep the declared spelling:
    // canonicalizing a top-level read-only junction can collapse it into an
    // explicit writable drive before BaseContainer sees the restriction.
    let mut read = NativePaths::new();
    for entry in &fs.entries {
        if let FileSystemPath::Path { path } = &entry.path
            && entry.access == FileSystemAccessMode::Read
            && let Ok(local) = path.to_abs_path()
            && fs.can_read_local_path_with_cwd(&local, cwd)
            && !fs.can_write_local_path_with_cwd(&local, cwd)
        {
            read.insert(path.clone(), local.into_path_buf());
        }
    }
    let mut deny = collect_paths(
        denied_paths
            .map_err(PolicyError::PolicyResolution)?
            .into_iter()
            .map(AbsolutePathBuf::into_path_buf),
    )?;
    // The shared resolver omits the cwd drive root for a default root denial.
    // An absolute drive-root denial also constrains aliases of that drive.
    for entry in &policy.entries {
        if let FileSystemPath::Path { path } = &entry.path
            && entry.access == FileSystemAccessMode::Deny
            && let Ok(local) = path.to_abs_path()
            && local.parent().is_none()
            && !fs.can_read_local_path_with_cwd(&local, cwd)
        {
            deny.insert(path.clone(), local.into_path_buf());
        }
    }
    let carveouts = collect_paths(roots.into_iter().flat_map(|root| {
        let protected = root
            .protected_metadata_names
            .into_iter()
            .map(|name| root.root.join(name).to_path_buf());
        root.read_only_subpaths
            .into_iter()
            .map(|path| path.to_path_buf())
            .chain(protected)
            .collect::<Vec<_>>()
    }))?;
    // Write restrictions must not grant otherwise forbidden reads.
    read.extend(
        carveouts
            .iter()
            .filter(|(_, path)| fs.can_read_local_path_with_cwd(path.as_path(), cwd))
            .map(|(key, path)| (key.clone(), path.clone())),
    );
    if policy.include_platform_defaults() {
        read.extend(collect_paths(platform_read_roots.iter().cloned())?);
    }
    read.retain(|key, _| {
        (!write.contains_key(key) || carveouts.contains_key(key)) && !deny.contains_key(key)
    });
    let volume_roots = volume_roots
        .iter()
        .map(PathUri::from_host_native_path)
        .collect::<std::io::Result<HashSet<_>>>()?;
    #[cfg(windows)]
    crate::windows::log_launch("phase=expand_volume_grants");
    prune_unavailable_volume_roots(&mut write, &volume_roots);
    prune_unavailable_volume_roots(&mut read, &volume_roots);
    // BaseContainer owns alias precedence for the emitted paths. Do not call
    // MXC's object normalizer: its metadata preflight rejects usable SMB grants
    // and inaccessible deny paths before the native API can evaluate them.
    let mut write = expand_volume_roots(write, &volume_roots, &read, &deny)?;
    write.retain(|key, _| !carveouts.contains_key(key) && !deny.contains_key(key));
    // Resolve equal-path write overrides before reaching MXC so the native
    // API receives one effective access mode for each path identity.
    read.retain(|key, _| !write.contains_key(key) && !deny.contains_key(key));
    let network_enabled = permissions.network_sandbox_policy().is_enabled();
    let proxied = command.managed_network.is_some();
    if let Some(network) = &command.managed_network {
        crate::validate_managed_network(network)
            .map_err(|error| PolicyError::PolicyResolution(error.to_string()))?;
    }
    let egress_default = if network_enabled && !proxied {
        NetworkAction::Allow
    } else {
        NetworkAction::Deny
    };
    let ingress_default = if network_enabled && !proxied {
        NetworkAction::Allow
    } else {
        NetworkAction::Deny
    };
    let mut egress = NetworkEgressPolicy {
        default: Some(egress_default),
        ..Default::default()
    };
    if proxied {
        // PSEC host loopback can be bidirectional. This shape is supported only
        // when the caller already allows local clients and servers. Keep
        // private-network ingress denied and allow no direct DNS bypass.
        egress.allow = Some(vec![NetworkRulePolicy {
            to: Some(vec![
                NetworkPeerPolicy::new("127.0.0.0/8"),
                NetworkPeerPolicy::new("::1/128"),
            ]),
            ..Default::default()
        }]);
    }
    let mut process_container = ProcessContainerConfig::default();
    process_container.capabilities = vec!["registryRead".to_owned()];
    let mut ui = ProcessContainerUi::default();
    ui.isolation = ProcessContainerUiIsolation::Desktop;
    process_container.ui = Some(ui);
    let environment = env
        .iter()
        .map(|entry| {
            let (key, value) = entry.split_once('=').ok_or_else(|| {
                PolicyError::PolicyResolution(
                    "MXC environment entries must be KEY=VALUE".to_owned(),
                )
            })?;
            Ok((key.to_owned(), value.to_owned()))
        })
        .collect::<Result<Vec<_>, PolicyError>>()?;
    #[cfg(windows)]
    crate::windows::log_launch(format_args!(
        "filesystem grants read={} write={} deny={}",
        read.len(),
        write.len(),
        deny.len()
    ));
    Ok(ContainerRequest {
        working_directory: Some(
            command_cwd
                .to_str()
                .ok_or(PolicyError::NonUnicodeCommandCwd)?
                .to_owned(),
        ),
        environment: (!environment.is_empty()).then_some(environment),
        filesystem: Some(FilesystemPolicy {
            readwrite_paths: unicode_paths(write)?,
            readonly_paths: unicode_paths(read)?,
            denied_paths: unicode_paths(deny)?,
            ..Default::default()
        }),
        network: Some(NetworkPolicy {
            egress: Some(egress),
            ingress: Some(NetworkIngressPolicy {
                default: Some(ingress_default),
                host_loopback: Some(if network_enabled || proxied {
                    NetworkAction::Allow
                } else {
                    NetworkAction::Deny
                }),
            }),
            ..Default::default()
        }),
        // PowerShell needs Win32k and desktop handles during DLL startup.
        // Keep clipboard, input injection, and system-control restrictions.
        ui: Some(UiPolicy {
            disable: false,
            ..Default::default()
        }),
        containment: Containment::ProcessContainer(process_container),
        ..ContainerRequest::new(match cmd_script_command_line(&command.command) {
            Some(script) => script,
            None => cmdline_from_argv_for_context(
                &command.command,
                CommandLineContext::WindowsCreateProcess,
            )?,
        })
    })
}

fn prune_unavailable_volume_roots(paths: &mut NativePaths, volume_roots: &HashSet<PathUri>) {
    paths.retain(|key, path| {
        if !volume_roots.contains(key) && path.parent().is_some() {
            return true;
        }
        match std::fs::read_dir(path) {
            Ok(_) => true,
            // An unlistable root can still expose known children. Keep its
            // restrictions and compare its aliases before generating grants.
            Err(error) => {
                error.kind() == std::io::ErrorKind::PermissionDenied || !inaccessible_volume(&error)
            }
        }
    });
}

// Bind :root before canonical precedence and carveout resolution. Only implicit
// reads use the filtered volumes; writes and denies retain every executor volume.
pub(super) fn materialize_volume_roots(
    mut fs: FileSystemSandboxPolicy,
    volume_roots: &[PathUri],
    implicit_read_roots: &[PathUri],
) -> Result<FileSystemSandboxPolicy, PolicyError> {
    let mut entries = Vec::new();
    for entry in fs.entries {
        match &entry.path {
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            } => {
                let roots = if entry.access == FileSystemAccessMode::Read {
                    implicit_read_roots
                } else {
                    volume_roots
                };
                entries.extend(roots.iter().map(|path| {
                    let mut entry = entry.clone();
                    entry.path = path.clone().into();
                    entry
                }));
            }
            FileSystemPath::Special {
                value:
                    FileSystemSpecialPath::ProjectRoots { .. }
                    | FileSystemSpecialPath::Tmpdir
                    | FileSystemSpecialPath::SlashTmp,
            } => return Err(PolicyError::UnsupportedSymbolicPath),
            FileSystemPath::Path { .. }
            | FileSystemPath::GlobPattern { .. }
            | FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal | FileSystemSpecialPath::Unknown { .. },
            } => entries.push(entry),
        }
    }
    fs.entries = entries;
    Ok(fs)
}

// MXC writable volume-root grants are nonrecursive, so snapshot each root's current
// immediate children as recursive grants. A child created directly under the
// root after policy construction is not granted. MXC/PSEC still enforces
// restrictions on junction-resolved targets at access time.
fn expand_volume_roots(
    mut paths: NativePaths,
    volume_roots: &HashSet<PathUri>,
    readonly: &NativePaths,
    denied: &NativePaths,
) -> Result<NativePaths, PolicyError> {
    let is_volume =
        |key: &PathUri, path: &Path| volume_roots.contains(key) || path.parent().is_none();
    if !paths.iter().any(|(key, path)| is_volume(key, path)) {
        return Ok(paths);
    }
    let denied_dirs: Vec<_> = directory_candidates(denied).collect();
    let readonly_dirs: Vec<_> = directory_candidates(readonly).collect();
    let restricted_dirs: Vec<_> = readonly_dirs
        .iter()
        .copied()
        .chain(denied_dirs.iter().copied())
        .collect();
    let writable: Vec<_> = paths.values().map(PathBuf::as_path).collect();
    let mut children = Vec::new();
    let mut unavailable = Vec::new();
    for (key, path) in &paths {
        if is_volume(key, path) {
            let indeterminate = |restriction: &Path| PolicyError::IndeterminateVolumeAlias {
                volume: path.clone(),
                restriction: restriction.to_owned(),
            };
            // Even an inferred read-only metadata grant could reopen this volume.
            if let Some(denial) = matching_directory(path, &denied_dirs).map_err(indeterminate)? {
                return Err(PolicyError::ConflictingVolumeDenial {
                    volume: path.clone(),
                    denial: denial.to_owned(),
                });
            }
            if matching_directory(path, &readonly_dirs)
                .map_err(indeterminate)?
                .is_some()
            {
                continue;
            }
            match std::fs::read_dir(path).and_then(Iterator::collect::<std::io::Result<Vec<_>>>) {
                Ok(entries) => {
                    for entry in entries {
                        let child = entry.path();
                        if child.to_str().is_none() {
                            continue;
                        }
                        // Windows supplies the file type with directory entries.
                        let Ok(kind) = entry.file_type() else {
                            continue;
                        };
                        if kind.is_symlink()
                            && !can_generate_link_grant(&child, &restricted_dirs, &writable)
                        {
                            continue;
                        }
                        children.push((PathUri::from_host_native_path(&child)?, child));
                    }
                }
                // GetLogicalDrives includes disconnected and empty removable
                // drives. They must not prevent commands on available volumes.
                Err(error) if inaccessible_volume(&error) => unavailable.push(key.clone()),
                Err(error) => {
                    return Err(PolicyError::EnumerateVolume {
                        path: path.clone(),
                        source: error,
                    });
                }
            }
        }
    }
    for key in unavailable {
        paths.remove(&key);
    }
    paths.extend(children);
    Ok(paths)
}

fn directory_candidates(paths: &NativePaths) -> impl Iterator<Item = &Path> {
    // Ordinary files and missing entries cannot be aliases of a directory. Keep
    // their native restrictions without asking the filesystem for a file ID.
    paths
        .values()
        .filter(|path| match std::fs::symlink_metadata(path) {
            Ok(metadata) => !metadata.is_file(),
            Err(error) => error.kind() != std::io::ErrorKind::NotFound,
        })
        .map(PathBuf::as_path)
}

// BaseContainer applies the strictest grant to equivalent directories. A
// synthesized child would override that restriction by being more specific.
fn matching_directory<'a>(
    path: &Path,
    directories: &[&'a Path],
) -> Result<Option<&'a Path>, &'a Path> {
    let mut unknown = None;
    for &directory in directories {
        match compare_existing_filesystem_objects(path, directory) {
            ExistingObjectComparison::Same => return Ok(Some(directory)),
            ExistingObjectComparison::Different => {}
            ExistingObjectComparison::Unknown => {
                #[cfg(windows)]
                if let Ok(same) = crate::windows::same_final_directory(path, directory) {
                    if same {
                        return Ok(Some(directory));
                    }
                    continue;
                }
                unknown.get_or_insert(directory);
            }
        }
    }
    unknown.map_or(Ok(None), Err)
}

fn can_generate_link_grant(link: &Path, restricted: &[&Path], writable: &[&Path]) -> bool {
    let Ok(metadata) = std::fs::metadata(link) else {
        return false;
    };
    if restricted.is_empty() {
        return true;
    }
    let Ok(mut target) = std::fs::canonicalize(link) else {
        return false;
    };
    // Hard-linked file names keep independent permissions. Compare directories only.
    if metadata.is_file() {
        target.pop();
    }
    // A junction on a second writable drive can land beneath a restricted
    // root. Apply the nearest explicit directory grant to its resolved target.
    for ancestor in target.ancestors() {
        if matching_directory(ancestor, restricted) != Ok(None) {
            return false;
        }
        if matches!(matching_directory(ancestor, writable), Ok(Some(_))) {
            return true;
        }
    }
    true
}

fn collect_paths(paths: impl IntoIterator<Item = PathBuf>) -> Result<NativePaths, PolicyError> {
    paths
        .into_iter()
        .map(|path| Ok((PathUri::from_host_native_path(&path)?, path)))
        .collect()
}

fn inaccessible_volume(error: &std::io::Error) -> bool {
    if matches!(
        error.kind(),
        std::io::ErrorKind::PermissionDenied
            | std::io::ErrorKind::NotFound
            | std::io::ErrorKind::NetworkUnreachable
            | std::io::ErrorKind::HostUnreachable
    ) {
        return true;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::ERROR_BAD_NETPATH;
        use windows_sys::Win32::Foundation::ERROR_CONNECTION_UNAVAIL;
        use windows_sys::Win32::Foundation::ERROR_DEVICE_NOT_CONNECTED;
        use windows_sys::Win32::Foundation::ERROR_NOT_READY;
        matches!(
            error.raw_os_error().map(|code| code as u32),
            Some(ERROR_NOT_READY | ERROR_DEVICE_NOT_CONNECTED | ERROR_BAD_NETPATH)
        ) || error.raw_os_error() == Some(ERROR_CONNECTION_UNAVAIL as i32)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn unicode_paths(paths: NativePaths) -> Result<Vec<String>, PolicyError> {
    let mut paths = paths
        .into_values()
        .map(|path| {
            path.into_os_string()
                .into_string()
                .map_err(|_| PolicyError::NonUnicodePolicyPath)
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    Ok(paths)
}
