use std::borrow::Cow;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_config::permissions_toml::FilesystemPermissionToml;
use codex_config::permissions_toml::NetworkToml;
use codex_config::permissions_toml::PermissionProfileToml;
use codex_config::permissions_toml::PermissionsToml;
use codex_network_proxy::NetworkProxyConfig;
#[cfg(test)]
use codex_network_proxy::NetworkUnixSocketPermission as ProxyNetworkUnixSocketPermission;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::PersistPermissionProfileAction;
use codex_protocol::request_permissions::PermissionProfilePersistence;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::config::Config;
use crate::config::deserialize_config_toml_with_base;
use crate::config::edit::ConfigEdit;
use crate::config::edit::apply_edits_to_string;

pub(crate) fn network_proxy_config_from_profile_network(
    network: Option<&NetworkToml>,
) -> NetworkProxyConfig {
    network.map_or_else(
        NetworkProxyConfig::default,
        NetworkToml::to_network_proxy_config,
    )
}

pub(crate) fn persistence_target_for_permissions(
    config: &Config,
    requested_permissions: &PermissionProfile,
) -> Option<PermissionProfilePersistence> {
    if !is_supported_filesystem_only_request(requested_permissions) {
        return None;
    }

    // TODO: honor inherited default permission profiles instead of only the raw user layer.
    let user_layer = config.config_layer_stack.get_user_layer()?;
    let user_config =
        deserialize_config_toml_with_base(user_layer.config.clone(), &config.codex_home).ok()?;
    let profile_name = user_config.default_permissions?;
    let permissions = user_config.permissions?;
    if !permissions.entries.contains_key(profile_name.as_str()) {
        return None;
    }

    let action = PersistPermissionProfileAction {
        profile_name,
        permissions: requested_permissions.clone(),
    };
    validate_persist_permission_profile_action(config, &action)
        .ok()
        .map(|()| PermissionProfilePersistence {
            profile_name: action.profile_name,
        })
}

pub(crate) fn validate_persist_permission_profile_action(
    config: &Config,
    action: &PersistPermissionProfileAction,
) -> io::Result<()> {
    let user_layer = config.config_layer_stack.get_user_layer().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "permissions profiles can only be persisted to user config",
        )
    })?;
    let serialized_user_config = toml::to_string(&user_layer.config).map_err(invalid_data)?;
    let user_config =
        deserialize_config_toml_with_base(user_layer.config.clone(), &config.codex_home)?;

    if user_config.default_permissions.as_deref() != Some(action.profile_name.as_str()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "permissions profile `{}` is not the active default profile",
                action.profile_name
            ),
        ));
    }

    let Some(permissions) = user_config.permissions.as_ref() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "default_permissions requires a `[permissions]` table",
        ));
    };
    if !permissions
        .entries
        .contains_key(action.profile_name.as_str())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "default_permissions refers to undefined profile `{}`",
                action.profile_name
            ),
        ));
    }

    let edits = [ConfigEdit::MergePermissionProfile(action.clone())];
    let merged_config =
        apply_edits_to_string(&serialized_user_config, /*profile*/ None, &edits)
            .map_err(invalid_data)?
            .unwrap_or(serialized_user_config);
    let merged_value = toml::from_str(&merged_config).map_err(invalid_data)?;
    let merged_user_config = deserialize_config_toml_with_base(merged_value, &config.codex_home)?;
    let permissions = merged_user_config.permissions.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "default_permissions requires a `[permissions]` table",
        )
    })?;
    let (file_system_sandbox_policy, network_sandbox_policy) =
        compile_permission_profile(&permissions, action.profile_name.as_str(), &mut Vec::new())?;
    file_system_sandbox_policy
        .to_legacy_sandbox_policy(network_sandbox_policy, config.cwd.as_path())?;
    Ok(())
}

pub(crate) fn resolve_permission_profile<'a>(
    permissions: &'a PermissionsToml,
    profile_name: &str,
) -> io::Result<&'a PermissionProfileToml> {
    permissions.entries.get(profile_name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("default_permissions refers to undefined profile `{profile_name}`"),
        )
    })
}

pub(crate) fn compile_permission_profile(
    permissions: &PermissionsToml,
    profile_name: &str,
    startup_warnings: &mut Vec<String>,
) -> io::Result<(FileSystemSandboxPolicy, NetworkSandboxPolicy)> {
    let profile = resolve_permission_profile(permissions, profile_name)?;

    let mut entries = Vec::new();
    if let Some(filesystem) = profile.filesystem.as_ref() {
        if filesystem.is_empty() {
            push_warning(
                startup_warnings,
                missing_filesystem_entries_warning(profile_name),
            );
        } else {
            for (path, permission) in &filesystem.entries {
                compile_filesystem_permission(path, permission, &mut entries, startup_warnings)?;
            }
        }
    } else {
        push_warning(
            startup_warnings,
            missing_filesystem_entries_warning(profile_name),
        );
    }

    let network_sandbox_policy = compile_network_sandbox_policy(profile.network.as_ref());

    Ok((
        FileSystemSandboxPolicy::restricted(entries),
        network_sandbox_policy,
    ))
}

/// Returns a list of paths that must be readable by shell tools in order
/// for Codex to function. These should always be added to the
/// `FileSystemSandboxPolicy` for a thread.
pub(crate) fn get_readable_roots_required_for_codex_runtime(
    codex_home: &Path,
    zsh_path: Option<&PathBuf>,
    main_execve_wrapper_exe: Option<&PathBuf>,
) -> Vec<AbsolutePathBuf> {
    let arg0_root = AbsolutePathBuf::from_absolute_path(codex_home.join("tmp").join("arg0")).ok();
    let zsh_path = zsh_path.and_then(|path| AbsolutePathBuf::from_absolute_path(path).ok());
    let execve_wrapper_root = main_execve_wrapper_exe.and_then(|path| {
        let path = AbsolutePathBuf::from_absolute_path(path).ok()?;
        if let Some(arg0_root) = arg0_root.as_ref()
            && path.as_path().starts_with(arg0_root.as_path())
        {
            path.parent()
        } else {
            Some(path)
        }
    });

    let mut readable_roots = Vec::new();
    if let Some(zsh_path) = zsh_path {
        readable_roots.push(zsh_path);
    }
    if let Some(execve_wrapper_root) = execve_wrapper_root {
        readable_roots.push(execve_wrapper_root);
    }
    readable_roots
}

fn compile_network_sandbox_policy(network: Option<&NetworkToml>) -> NetworkSandboxPolicy {
    let Some(network) = network else {
        return NetworkSandboxPolicy::Restricted;
    };

    match network.enabled {
        Some(true) => NetworkSandboxPolicy::Enabled,
        _ => NetworkSandboxPolicy::Restricted,
    }
}

fn is_supported_filesystem_only_request(permissions: &PermissionProfile) -> bool {
    let Some(file_system) = permissions.file_system.as_ref() else {
        return false;
    };

    if file_system.is_empty() {
        return false;
    }

    if permissions
        .network
        .as_ref()
        .and_then(|network| network.enabled)
        .unwrap_or(false)
    {
        return false;
    }

    true
}

fn invalid_data(err: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err.to_string())
}

fn compile_filesystem_permission(
    path: &str,
    permission: &FilesystemPermissionToml,
    entries: &mut Vec<FileSystemSandboxEntry>,
    startup_warnings: &mut Vec<String>,
) -> io::Result<()> {
    match permission {
        FilesystemPermissionToml::Access(access) => entries.push(FileSystemSandboxEntry {
            path: compile_filesystem_path(path, startup_warnings)?,
            access: *access,
        }),
        FilesystemPermissionToml::Scoped(scoped_entries) => {
            for (subpath, access) in scoped_entries {
                entries.push(FileSystemSandboxEntry {
                    path: compile_scoped_filesystem_path(path, subpath, startup_warnings)?,
                    access: *access,
                });
            }
        }
    }
    Ok(())
}

fn compile_filesystem_path(
    path: &str,
    startup_warnings: &mut Vec<String>,
) -> io::Result<FileSystemPath> {
    if let Some(special) = parse_special_path(path) {
        maybe_push_unknown_special_path_warning(&special, startup_warnings);
        return Ok(FileSystemPath::Special { value: special });
    }

    let path = parse_absolute_path(path)?;
    Ok(FileSystemPath::Path { path })
}

fn compile_scoped_filesystem_path(
    path: &str,
    subpath: &str,
    startup_warnings: &mut Vec<String>,
) -> io::Result<FileSystemPath> {
    if subpath == "." {
        return compile_filesystem_path(path, startup_warnings);
    }

    if let Some(special) = parse_special_path(path) {
        let subpath = parse_relative_subpath(subpath)?;
        let special = match special {
            FileSystemSpecialPath::ProjectRoots { .. } => Ok(FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(Some(subpath)),
            }),
            FileSystemSpecialPath::Unknown { path, .. } => Ok(FileSystemPath::Special {
                value: FileSystemSpecialPath::unknown(path, Some(subpath)),
            }),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("filesystem path `{path}` does not support nested entries"),
            )),
        }?;
        if let FileSystemPath::Special { value } = &special {
            maybe_push_unknown_special_path_warning(value, startup_warnings);
        }
        return Ok(special);
    }

    let subpath = parse_relative_subpath(subpath)?;
    let base = parse_absolute_path(path)?;
    let path = AbsolutePathBuf::resolve_path_against_base(&subpath, base.as_path());
    Ok(FileSystemPath::Path { path })
}

// WARNING: keep this parser forward-compatible.
// Adding a new `:special_path` must not make older Codex versions reject the
// config. Unknown values intentionally round-trip through
// `FileSystemSpecialPath::Unknown` so they can be surfaced as warnings and
// ignored, rather than aborting config load.
fn parse_special_path(path: &str) -> Option<FileSystemSpecialPath> {
    match path {
        ":root" => Some(FileSystemSpecialPath::Root),
        ":minimal" => Some(FileSystemSpecialPath::Minimal),
        ":project_roots" => Some(FileSystemSpecialPath::project_roots(/*subpath*/ None)),
        ":tmpdir" => Some(FileSystemSpecialPath::Tmpdir),
        _ if path.starts_with(':') => {
            Some(FileSystemSpecialPath::unknown(path, /*subpath*/ None))
        }
        _ => None,
    }
}

fn parse_absolute_path(path: &str) -> io::Result<AbsolutePathBuf> {
    parse_absolute_path_for_platform(path, cfg!(windows))
}

fn parse_absolute_path_for_platform(path: &str, is_windows: bool) -> io::Result<AbsolutePathBuf> {
    let path_ref = normalize_absolute_path_for_platform(path, is_windows);
    if !is_absolute_path_for_platform(path, path_ref.as_ref(), is_windows)
        && path != "~"
        && !path.starts_with("~/")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("filesystem path `{path}` must be absolute, use `~/...`, or start with `:`"),
        ));
    }
    AbsolutePathBuf::from_absolute_path(path_ref.as_ref())
}

fn is_absolute_path_for_platform(path: &str, normalized_path: &Path, is_windows: bool) -> bool {
    if is_windows {
        is_windows_absolute_path(path)
            || is_windows_absolute_path(&normalized_path.to_string_lossy())
    } else {
        normalized_path.is_absolute()
    }
}

fn normalize_absolute_path_for_platform(path: &str, is_windows: bool) -> Cow<'_, Path> {
    if !is_windows {
        return Cow::Borrowed(Path::new(path));
    }

    match normalize_windows_device_path(path) {
        Some(normalized) => Cow::Owned(PathBuf::from(normalized)),
        None => Cow::Borrowed(Path::new(path)),
    }
}

fn normalize_windows_device_path(path: &str) -> Option<String> {
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        return Some(format!(r"\\{unc}"));
    }
    if let Some(unc) = path.strip_prefix(r"\\.\UNC\") {
        return Some(format!(r"\\{unc}"));
    }
    if let Some(path) = path.strip_prefix(r"\\?\")
        && is_windows_drive_absolute_path(path)
    {
        return Some(path.to_string());
    }
    if let Some(path) = path.strip_prefix(r"\\.\")
        && is_windows_drive_absolute_path(path)
    {
        return Some(path.to_string());
    }
    None
}

fn is_windows_absolute_path(path: &str) -> bool {
    is_windows_drive_absolute_path(path) || path.starts_with(r"\\")
}

fn is_windows_drive_absolute_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
}

fn parse_relative_subpath(subpath: &str) -> io::Result<PathBuf> {
    let path = Path::new(subpath);
    if !subpath.is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Ok(path.to_path_buf());
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "filesystem subpath `{}` must be a descendant path without `.` or `..` components",
            path.display()
        ),
    ))
}

fn push_warning(startup_warnings: &mut Vec<String>, message: String) {
    tracing::warn!("{message}");
    startup_warnings.push(message);
}

fn missing_filesystem_entries_warning(profile_name: &str) -> String {
    format!(
        "Permissions profile `{profile_name}` does not define any recognized filesystem entries for this version of Codex. Filesystem access will remain restricted. Upgrade Codex if this profile expects filesystem permissions."
    )
}

fn maybe_push_unknown_special_path_warning(
    special: &FileSystemSpecialPath,
    startup_warnings: &mut Vec<String>,
) {
    let FileSystemSpecialPath::Unknown { path, subpath } = special else {
        return;
    };
    push_warning(
        startup_warnings,
        match subpath.as_deref() {
            Some(subpath) => format!(
                "Configured filesystem path `{path}` with nested entry `{}` is not recognized by this version of Codex and will be ignored. Upgrade Codex if this path is required.",
                subpath.display()
            ),
            None => format!(
                "Configured filesystem path `{path}` is not recognized by this version of Codex and will be ignored. Upgrade Codex if this path is required."
            ),
        },
    );
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
