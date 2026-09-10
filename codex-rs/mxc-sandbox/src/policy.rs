//! Translate canonical permissions into native MXC grants without expanding
//! access. Denies and read-only carveouts remain separate kernel policy lists.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::protocol::FileSystemPath;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use codex_protocol::protocol::FileSystemSpecialPath;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use codex_windows_sandbox::resolve_windows_deny_read_paths;
use thiserror::Error;
use wxc_common::cmdline::CommandLineContext;
use wxc_common::cmdline::CommandLineError;
use wxc_common::cmdline::cmdline_from_argv_for_context;
use wxc_common::filesystem_object::ExistingObjectComparison;
use wxc_common::filesystem_object::compare_existing_filesystem_objects;
use wxc_common::filesystem_object::normalize_object_conflicts;
use wxc_common::logger::Logger;
use wxc_common::logger::Mode;
use wxc_common::models::ContainerPolicy;
use wxc_common::models::ExecutionRequest;
use wxc_common::models::FallbackPolicy;
use wxc_common::models::NetworkAction;
use wxc_common::models::NetworkEgressPolicy;
use wxc_common::models::NetworkIngressPolicy;
use wxc_common::models::NetworkPolicy;

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
    #[error(transparent)]
    Path(#[from] std::io::Error),
    #[error(transparent)]
    CommandLine(#[from] CommandLineError),
}

pub fn build_request(
    command: &MxcCommand,
    command_cwd: &Path,
    env: Vec<String>,
    volume_roots: &[PathBuf],
    platform_read_roots: &[PathBuf],
) -> Result<ExecutionRequest, PolicyError> {
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
    let mut policy = permissions.file_system_sandbox_policy();
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
    let full_disk_write = policy.has_full_disk_write_access();
    let volumes = volume_roots
        .iter()
        .map(PathUri::from_host_native_path)
        .collect::<std::io::Result<Vec<_>>>()?;
    let fs = materialize_volume_roots(policy.clone(), &volumes)?;
    let roots = if full_disk_write {
        Vec::new()
    } else {
        fs.get_writable_roots_with_cwd_preserving_mutable_paths(cwd)
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
    let mut read = collect_paths(
        fs.get_readable_roots_with_cwd(cwd)
            .into_iter()
            .filter(|path| !fs.can_write_local_path_with_cwd(path.as_path(), cwd))
            .map(|path| path.to_path_buf()),
    )?;
    let absolute_cwd = AbsolutePathBuf::from_absolute_path(cwd)
        .map_err(|error| PolicyError::PolicyResolution(error.to_string()))?;
    // A symbolic root deny is the default, not a recursive native mask that
    // should erase narrower grants. Only materialize grants and carveouts.
    let deny = collect_paths(
        resolve_windows_deny_read_paths(&policy, &absolute_cwd)
            .map_err(PolicyError::PolicyResolution)?
            .into_iter()
            .map(AbsolutePathBuf::into_path_buf),
    )?;
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
    write.retain(|key, _| !carveouts.contains_key(key) && !deny.contains_key(key));
    read.retain(|key, _| !write.contains_key(key) && !deny.contains_key(key));
    let volume_roots = volume_roots
        .iter()
        .map(PathUri::from_host_native_path)
        .collect::<std::io::Result<HashSet<_>>>()?;
    prune_unavailable_volume_roots(&mut write, &volume_roots);
    prune_unavailable_volume_roots(&mut read, &volume_roots);
    // Normalize pre-expansion aliases so generated children inherit
    // any tightened access applied to their volume root.
    let normalized = normalize_policy(ContainerPolicy {
        readwrite_paths: unicode_paths(write)?,
        readonly_paths: unicode_paths(read)?,
        denied_paths: unicode_paths(deny)?,
        ..Default::default()
    })?;
    let write = collect_paths(normalized.readwrite_paths.into_iter().map(PathBuf::from))?;
    let read = collect_paths(normalized.readonly_paths.into_iter().map(PathBuf::from))?;
    let deny = collect_paths(normalized.denied_paths.into_iter().map(PathBuf::from))?;
    // If expansion skips an unavailable writable volume, its generated read-only
    // carveouts remain below. Pruning them requires retaining their root provenance,
    // so we accept that rare normalization failure for now.
    let mut write = expand_volume_roots(write, &volume_roots)?;
    write.retain(|key, _| !carveouts.contains_key(key) && !deny.contains_key(key));
    let mut read = expand_volume_roots(read, &volume_roots)?;
    // Resolve equal-path write overrides before reaching MXC so the native
    // API receives one effective access mode for each path identity.
    read.retain(|key, _| !write.contains_key(key) && !deny.contains_key(key));
    let network_enabled = permissions.network_sandbox_policy().is_enabled();
    let egress_default = if network_enabled {
        NetworkAction::Allow
    } else {
        NetworkAction::Deny
    };
    let ingress_default = if network_enabled {
        NetworkAction::Allow
    } else {
        NetworkAction::Deny
    };
    let egress = NetworkEgressPolicy {
        default: egress_default,
        ..Default::default()
    };
    let mut request = ExecutionRequest {
        script_code: cmdline_from_argv_for_context(
            &command.command,
            CommandLineContext::WindowsCreateProcess,
        )?,
        working_directory: command_cwd
            .to_str()
            .ok_or(PolicyError::NonUnicodeCommandCwd)?
            .to_owned(),
        env,
        policy: ContainerPolicy {
            capabilities: vec!["registryRead".to_owned()],
            readwrite_paths: unicode_paths(write)?,
            readonly_paths: unicode_paths(read)?,
            denied_paths: unicode_paths(deny)?,
            fallback: FallbackPolicy {
                allow_dacl_mutation: false,
            },
            default_network_policy: if egress_default == NetworkAction::Allow {
                NetworkPolicy::Allow
            } else {
                NetworkPolicy::Block
            },
            allow_local_network: ingress_default == NetworkAction::Allow,
            network_egress: Some(egress),
            network_ingress: Some(NetworkIngressPolicy {
                default: ingress_default,
                host_loopback: if network_enabled {
                    NetworkAction::Allow
                } else {
                    NetworkAction::Deny
                },
            }),
            network_specified: true,
            network_mode_specified: true,
            ..Default::default()
        },
        ..Default::default()
    };
    // Reconcile aliases introduced by expansion. MXC/PSEC separately enforces
    // restrictions on junction-resolved targets at access time.
    request.policy = normalize_policy(request.policy)?;
    Ok(request)
}

fn normalize_policy(mut policy: ContainerPolicy) -> Result<ContainerPolicy, PolicyError> {
    let mut logger = Logger::new(Mode::Buffer);
    if let Some(normalized) =
        normalize_object_conflicts(&policy, &mut logger).map_err(PolicyError::PolicyResolution)?
    {
        policy = normalized;
    }
    Ok(policy)
}

fn prune_unavailable_volume_roots(paths: &mut NativePaths, volume_roots: &HashSet<PathUri>) {
    paths.retain(|key, path| {
        if !volume_roots.contains(key) && path.parent().is_some() {
            return true;
        }
        match std::fs::read_dir(path) {
            Ok(_) => true,
            Err(error) => !inaccessible_volume(&error),
        }
    });
}

// Bind :root to every executor volume before canonical precedence and carveout
// resolution. This is purely lexical; native volume enumeration happens later.
pub(super) fn materialize_volume_roots(
    mut fs: FileSystemSandboxPolicy,
    volume_roots: &[PathUri],
) -> Result<FileSystemSandboxPolicy, PolicyError> {
    let mut entries = Vec::new();
    for entry in fs.entries {
        match &entry.path {
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            } => entries.extend(volume_roots.iter().map(|path| {
                let mut entry = entry.clone();
                entry.path = path.clone().into();
                entry
            })),
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

// MXC volume-root grants are nonrecursive, so snapshot each root's current
// immediate children as recursive grants. A child created directly under the
// root after policy construction is not granted. MXC/PSEC still enforces
// restrictions on junction-resolved targets at access time.
fn expand_volume_roots(
    paths: NativePaths,
    volume_roots: &HashSet<PathUri>,
) -> Result<NativePaths, PolicyError> {
    let mut expanded = NativePaths::new();
    for (key, path) in paths {
        if volume_roots.contains(&key) || path.parent().is_none() {
            match std::fs::read_dir(&path).and_then(Iterator::collect::<std::io::Result<Vec<_>>>) {
                Ok(entries) => {
                    for entry in entries {
                        let child = entry.path();
                        if child.to_str().is_none() {
                            continue;
                        }
                        // A self-comparison is MXC's public object-identity probe.
                        // Generated grants can be skipped; explicit paths fail closed below.
                        if compare_existing_filesystem_objects(&child, &child)
                            != ExistingObjectComparison::Same
                        {
                            continue;
                        }
                        expanded.insert(PathUri::from_host_native_path(&child)?, child);
                    }
                }
                // GetLogicalDrives includes disconnected and empty removable
                // drives. They must not prevent commands on available volumes.
                Err(error) if inaccessible_volume(&error) => continue,
                Err(error) => {
                    return Err(PolicyError::EnumerateVolume {
                        path,
                        source: error,
                    });
                }
            }
        }
        expanded.insert(key, path);
    }
    Ok(expanded)
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
