use std::borrow::Cow;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_config::permissions_toml::FilesystemPermissionToml;
use codex_config::permissions_toml::FilesystemPermissionsToml;
use codex_config::permissions_toml::NetworkToml;
use codex_config::permissions_toml::PermissionProfileToml;
use codex_config::permissions_toml::PermissionsToml;
use codex_network_proxy::NetworkProxyConfig;
#[cfg(test)]
use codex_network_proxy::NetworkUnixSocketPermission as ProxyNetworkUnixSocketPermission;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;

pub(crate) fn network_proxy_config_from_profile_network(
    network: Option<&NetworkToml>,
) -> NetworkProxyConfig {
    network.map_or_else(
        NetworkProxyConfig::default,
        NetworkToml::to_network_proxy_config,
    )
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
    policy_cwd: &Path,
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
            if cfg!(not(target_os = "macos")) {
                for pattern in unsupported_read_write_glob_paths(filesystem) {
                    push_warning(
                        startup_warnings,
                        format!(
                            "Filesystem glob `{pattern}` uses `read` or `write` access, which is not fully supported by this platform's sandboxing. Use an exact path or trailing `/**` subtree rule instead. `none` deny-read globs are supported."
                        ),
                    );
                }
                for pattern in unbounded_unreadable_globstar_paths(filesystem) {
                    push_warning(
                        startup_warnings,
                        format!(
                            "Filesystem deny-read glob `{pattern}` uses `**`. Non-macOS sandboxing does not support unbounded `**` natively; set `glob_scan_max_depth` in this filesystem profile to cap Linux glob expansion and silence this warning, or enumerate explicit depths such as `*.env`, `*/*.env`, and `*/*/*.env`."
                        ),
                    );
                }
            }
            for (path, permission) in &filesystem.entries {
                entries.extend(compile_filesystem_permission(
                    path,
                    permission,
                    policy_cwd,
                    startup_warnings,
                )?);
            }
        }
    } else {
        push_warning(
            startup_warnings,
            missing_filesystem_entries_warning(profile_name),
        );
    }
    let glob_scan_max_depth = validate_glob_scan_max_depth(
        profile
            .filesystem
            .as_ref()
            .and_then(|filesystem| filesystem.glob_scan_max_depth),
    )?;

    let network_sandbox_policy = compile_network_sandbox_policy(profile.network.as_ref());
    let mut file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(entries);
    file_system_sandbox_policy.glob_scan_max_depth = glob_scan_max_depth;
    Ok((file_system_sandbox_policy, network_sandbox_policy))
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

fn compile_filesystem_permission(
    path: &str,
    permission: &FilesystemPermissionToml,
    policy_cwd: &Path,
    startup_warnings: &mut Vec<String>,
) -> io::Result<Vec<FileSystemSandboxEntry>> {
    let mut entries = Vec::new();
    match permission {
        FilesystemPermissionToml::Access(access) => {
            entries.push(FileSystemSandboxEntry {
                path: compile_filesystem_access_path(path, *access, startup_warnings)?,
                access: *access,
            });
        }
        FilesystemPermissionToml::Scoped(scoped_entries) => {
            for (subpath, access) in scoped_entries {
                let has_glob = contains_glob_chars(subpath);
                let can_compile_as_pattern = match parse_special_path(path) {
                    Some(FileSystemSpecialPath::ProjectRoots { .. }) | None => true,
                    Some(_) => false,
                };
                if has_glob && *access == FileSystemAccessMode::None && can_compile_as_pattern {
                    // Scoped glob syntax is a first-class filesystem policy
                    // pattern entry. Literal scoped paths continue through the
                    // exact-path parser so existing path semantics stay intact.
                    let entry = FileSystemSandboxEntry {
                        path: FileSystemPath::GlobPattern {
                            pattern: compile_scoped_filesystem_pattern(
                                path, subpath, *access, policy_cwd,
                            )?,
                        },
                        access: *access,
                    };
                    entries.push(entry);
                } else {
                    let subpath = compile_read_write_glob_path(subpath, *access)?;
                    entries.push(FileSystemSandboxEntry {
                        path: compile_scoped_filesystem_path(path, subpath, startup_warnings)?,
                        access: *access,
                    });
                }
            }
        }
    }
    Ok(entries)
}

fn compile_filesystem_access_path(
    path: &str,
    access: FileSystemAccessMode,
    startup_warnings: &mut Vec<String>,
) -> io::Result<FileSystemPath> {
    if !contains_glob_chars(path) {
        return compile_filesystem_path(path, startup_warnings);
    }

    if access == FileSystemAccessMode::None {
        // At this point `path` is an unscoped filesystem table key. Top-level
        // glob deny entries still go through the absolute-path parser before
        // becoming policy patterns; relative project-root glob syntax is
        // handled by `compile_scoped_filesystem_pattern`.
        return Ok(FileSystemPath::GlobPattern {
            pattern: parse_absolute_path(path)?.to_string_lossy().into_owned(),
        });
    }

    let path = compile_read_write_glob_path(path, access)?;
    compile_filesystem_path(path, startup_warnings)
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

fn compile_scoped_filesystem_pattern(
    path: &str,
    subpath: &str,
    access: FileSystemAccessMode,
    policy_cwd: &Path,
) -> io::Result<String> {
    // Pattern entries currently mean deny-read only. Supporting broader access
    // modes here would imply glob-based read/write allow semantics that the
    // sandbox policy does not express yet.
    if access != FileSystemAccessMode::None {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("filesystem glob subpath `{subpath}` only supports `none` access"),
        ));
    }
    let subpath = parse_relative_subpath(subpath)?;

    match parse_special_path(path) {
        Some(FileSystemSpecialPath::ProjectRoots { .. }) => {
            // `:project_roots` is represented as a special path, but current
            // filesystem-policy resolution defines it relative to the session
            // cwd. Use the same policy cwd here so glob entries and exact
            // scoped entries resolve consistently.
            Ok(
                AbsolutePathBuf::resolve_path_against_base(&subpath, policy_cwd)
                    .to_string_lossy()
                    .to_string(),
            )
        }
        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("filesystem path `{path}` does not support nested entries"),
        )),
        None => {
            let base = parse_absolute_path(path)?;
            Ok(base.join(&subpath).to_string_lossy().to_string())
        }
    }
}

fn compile_read_write_glob_path(path: &str, access: FileSystemAccessMode) -> io::Result<&str> {
    if !contains_glob_chars(path) {
        return Ok(path);
    }

    let path_without_trailing_glob = remove_trailing_glob_suffix(path);
    if !contains_glob_chars(path_without_trailing_glob) {
        return Ok(path_without_trailing_glob);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "filesystem glob path `{path}` only supports `none` access; use an exact path or trailing `/**` for `{access}` subtree access"
        ),
    ))
}

fn unsupported_read_write_glob_paths(filesystem: &FilesystemPermissionsToml) -> Vec<String> {
    let mut patterns = Vec::new();
    for (path, permission) in &filesystem.entries {
        match permission {
            FilesystemPermissionToml::Access(access) => {
                if *access != FileSystemAccessMode::None
                    && contains_glob_chars(remove_trailing_glob_suffix(path))
                {
                    patterns.push(path.clone());
                }
            }
            FilesystemPermissionToml::Scoped(scoped_entries) => {
                for (subpath, access) in scoped_entries {
                    if *access != FileSystemAccessMode::None
                        && contains_glob_chars(remove_trailing_glob_suffix(subpath))
                    {
                        patterns.push(format!("{path}/{subpath}"));
                    }
                }
            }
        }
    }
    patterns
}

fn unbounded_unreadable_globstar_paths(filesystem: &FilesystemPermissionsToml) -> Vec<String> {
    if filesystem.glob_scan_max_depth.is_some() {
        return Vec::new();
    }

    let mut patterns = Vec::new();
    for (path, permission) in &filesystem.entries {
        match permission {
            FilesystemPermissionToml::Access(FileSystemAccessMode::None) => {
                if path.contains("**") {
                    patterns.push(path.clone());
                }
            }
            FilesystemPermissionToml::Access(_) => {}
            FilesystemPermissionToml::Scoped(scoped_entries) => {
                for (subpath, access) in scoped_entries {
                    if *access == FileSystemAccessMode::None && subpath.contains("**") {
                        patterns.push(format!("{path}/{subpath}"));
                    }
                }
            }
        }
    }
    patterns
}

fn validate_glob_scan_max_depth(max_depth: Option<usize>) -> io::Result<Option<usize>> {
    match max_depth {
        Some(0) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "glob_scan_max_depth must be at least 1",
        )),
        _ => Ok(max_depth),
    }
}

fn contains_glob_chars(path: &str) -> bool {
    path.chars().any(|ch| matches!(ch, '*' | '?' | '[' | ']'))
}

fn remove_trailing_glob_suffix(path: &str) -> &str {
    path.strip_suffix("/**").unwrap_or(path)
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
