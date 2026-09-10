use std::collections::BTreeMap;
use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_config::ConfigPathContext;
use codex_config::permissions_toml::FilesystemPermissionToml;
use codex_config::permissions_toml::FilesystemPermissionsToml;
use codex_config::permissions_toml::NetworkDomainPermissionToml;
use codex_config::permissions_toml::NetworkDomainPermissionsToml;
use codex_config::permissions_toml::NetworkToml;
use codex_config::permissions_toml::NetworkUnixSocketPermissionToml;
use codex_config::permissions_toml::NetworkUnixSocketPermissionsToml;
use codex_config::permissions_toml::PermissionProfileToml;
use codex_config::permissions_toml::PermissionsToml;
use codex_config::permissions_toml::WorkspaceRootsToml;
use codex_features::NetworkProxyConfigToml;
use codex_features::NetworkProxyDomainPermissionToml;
use codex_features::NetworkProxyModeToml;
use codex_features::NetworkProxyUnixSocketPermissionToml;
use codex_network_proxy::NetworkMode;
use codex_network_proxy::NetworkProxyConfig;
#[cfg(test)]
use codex_network_proxy::NetworkUnixSocketPermission as ProxyNetworkUnixSocketPermission;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_READ_ONLY;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_WORKSPACE;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::permissions::project_roots_glob_pattern;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

use super::ProjectConfig;
use super::permission_path;
#[cfg(test)]
use super::permission_path::contains_glob_chars_for_platform;

pub(crate) const BUILT_IN_READ_ONLY_PROFILE: &str = BUILT_IN_PERMISSION_PROFILE_READ_ONLY;
pub(crate) const BUILT_IN_WORKSPACE_PROFILE: &str = BUILT_IN_PERMISSION_PROFILE_WORKSPACE;
pub(crate) const BUILT_IN_DANGER_FULL_ACCESS_PROFILE: &str =
    BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS;

pub(crate) fn default_builtin_permission_profile_name(
    active_project: &ProjectConfig,
    windows_sandbox_level: WindowsSandboxLevel,
) -> &'static str {
    if (active_project.is_trusted() || active_project.is_untrusted())
        && !(cfg!(target_os = "windows") && windows_sandbox_level == WindowsSandboxLevel::Disabled)
    {
        BUILT_IN_WORKSPACE_PROFILE
    } else {
        BUILT_IN_READ_ONLY_PROFILE
    }
}

pub(crate) fn is_builtin_permission_profile_name(profile_name: &str) -> bool {
    matches!(
        profile_name,
        BUILT_IN_READ_ONLY_PROFILE
            | BUILT_IN_WORKSPACE_PROFILE
            | BUILT_IN_DANGER_FULL_ACCESS_PROFILE
    )
}

fn builtin_permission_profile(
    profile_name: &str,
    workspace_write: Option<&WorkspaceWriteSettings>,
) -> Option<PermissionProfile> {
    match profile_name {
        BUILT_IN_READ_ONLY_PROFILE => Some(PermissionProfile::read_only()),
        BUILT_IN_WORKSPACE_PROFILE => Some(match workspace_write {
            Some(WorkspaceWriteSettings {
                writable_roots: _,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            }) => PermissionProfile::workspace_write_with(
                &[],
                if *network_access {
                    NetworkSandboxPolicy::Enabled
                } else {
                    NetworkSandboxPolicy::Restricted
                },
                *exclude_tmpdir_env_var,
                *exclude_slash_tmp,
            ),
            None => PermissionProfile::workspace_write(),
        }),
        BUILT_IN_DANGER_FULL_ACCESS_PROFILE => Some(PermissionProfile::Disabled),
        _ => None,
    }
}

pub(crate) fn validate_user_permission_profile_names(
    permissions: Option<&PermissionsToml>,
) -> io::Result<()> {
    let Some(permissions) = permissions else {
        return Ok(());
    };

    for profile_name in permissions.entries.keys() {
        if profile_name.starts_with(':') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "permissions profile `{profile_name}` uses a reserved built-in profile prefix"
                ),
            ));
        }
    }

    Ok(())
}

pub(crate) fn network_proxy_config_from_profile_network(
    network: Option<&NetworkToml>,
) -> NetworkProxyConfig {
    let mut config = network.map_or_else(
        NetworkProxyConfig::default,
        NetworkToml::to_network_proxy_config,
    );
    // Profile `network.enabled` controls sandbox network access. Profiles may
    // provide proxy settings for the feature gate to consume when that network
    // access is enabled, but they do not start the managed proxy on their own.
    config.enabled = false;
    config
}

pub(crate) fn apply_network_proxy_feature_config(
    config: &mut NetworkProxyConfig,
    feature_config: &NetworkProxyConfigToml,
) {
    NetworkToml {
        enabled: feature_config.enabled,
        proxy_url: feature_config.proxy_url.clone(),
        enable_socks5: feature_config.enable_socks5,
        socks_url: feature_config.socks_url.clone(),
        enable_socks5_udp: feature_config.enable_socks5_udp,
        allow_upstream_proxy: feature_config.allow_upstream_proxy,
        dangerously_allow_non_loopback_proxy: feature_config.dangerously_allow_non_loopback_proxy,
        dangerously_allow_all_unix_sockets: feature_config.dangerously_allow_all_unix_sockets,
        mode: feature_config.mode.map(|mode| match mode {
            NetworkProxyModeToml::Limited => NetworkMode::Limited,
            NetworkProxyModeToml::Full => NetworkMode::Full,
        }),
        domains: feature_config
            .domains
            .as_ref()
            .map(|domains| NetworkDomainPermissionsToml {
                entries: domains
                    .iter()
                    .map(|(pattern, permission)| {
                        let permission = match permission {
                            NetworkProxyDomainPermissionToml::Allow => {
                                NetworkDomainPermissionToml::Allow
                            }
                            NetworkProxyDomainPermissionToml::Deny => {
                                NetworkDomainPermissionToml::Deny
                            }
                        };
                        (pattern.clone(), permission)
                    })
                    .collect(),
            }),
        unix_sockets: feature_config.unix_sockets.as_ref().map(|unix_sockets| {
            NetworkUnixSocketPermissionsToml {
                entries: unix_sockets
                    .iter()
                    .map(|(path, permission)| {
                        let permission = match permission {
                            NetworkProxyUnixSocketPermissionToml::Allow => {
                                NetworkUnixSocketPermissionToml::Allow
                            }
                            NetworkProxyUnixSocketPermissionToml::Deny => {
                                NetworkUnixSocketPermissionToml::Deny
                            }
                        };
                        (path.clone(), permission)
                    })
                    .collect(),
            }
        }),
        allow_local_binding: feature_config.allow_local_binding,
        mitm: None,
    }
    .apply_to_network_proxy_config(config);
    if let Some(credential_broker) = feature_config.credential_broker {
        config.set_credential_broker_enabled(credential_broker);
    }
    if let Some(credentials) = feature_config.credentials.as_ref() {
        config.credential_providers.clone_from(credentials);
    }
}

/// Resolves a named permission profile and its inherited configuration.
pub fn resolve_permission_profile(
    permissions: &PermissionsToml,
    profile_name: &str,
) -> io::Result<PermissionProfileToml> {
    permissions
        .resolve_profile(profile_name, extensible_builtin_parent_profile)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err.to_string()))
}

fn extensible_builtin_parent_profile(profile_name: &str) -> Option<PermissionProfileToml> {
    let file_system = match profile_name {
        BUILT_IN_READ_ONLY_PROFILE => FileSystemSandboxPolicy::read_only(),
        BUILT_IN_WORKSPACE_PROFILE => FileSystemSandboxPolicy::workspace_write(
            &[],
            /*exclude_tmpdir_env_var*/ false,
            /*exclude_slash_tmp*/ false,
        ),
        _ => return None,
    };
    Some(permission_profile_toml_from_file_system_policy(file_system))
}

fn permission_profile_toml_from_file_system_policy(
    file_system: FileSystemSandboxPolicy,
) -> PermissionProfileToml {
    let mut filesystem = FilesystemPermissionsToml {
        glob_scan_max_depth: file_system.glob_scan_max_depth,
        entries: BTreeMap::new(),
    };
    for entry in file_system.entries {
        insert_filesystem_permission_toml(&mut filesystem.entries, entry);
    }
    PermissionProfileToml {
        description: None,
        extends: None,
        workspace_roots: None,
        filesystem: Some(filesystem),
        network: None,
    }
}

fn insert_filesystem_permission_toml(
    entries: &mut BTreeMap<String, FilesystemPermissionToml>,
    entry: FileSystemSandboxEntry,
) {
    if entry.skips_missing_path() {
        return;
    }

    match entry.path {
        FileSystemPath::Path { path } => {
            entries.insert(
                path.inferred_native_path_string(),
                FilesystemPermissionToml::Access(entry.access),
            );
        }
        FileSystemPath::GlobPattern { pattern } => {
            entries.insert(pattern, FilesystemPermissionToml::Access(entry.access));
        }
        FileSystemPath::Special { value } => {
            insert_special_filesystem_permission_toml(entries, value, entry.access);
        }
    }
}

fn insert_special_filesystem_permission_toml(
    entries: &mut BTreeMap<String, FilesystemPermissionToml>,
    value: FileSystemSpecialPath,
    access: FileSystemAccessMode,
) {
    match value {
        FileSystemSpecialPath::Root => {
            entries.insert(
                ":root".to_string(),
                FilesystemPermissionToml::Access(access),
            );
        }
        FileSystemSpecialPath::Minimal => {
            entries.insert(
                ":minimal".to_string(),
                FilesystemPermissionToml::Access(access),
            );
        }
        FileSystemSpecialPath::ProjectRoots { subpath } => {
            insert_scoped_filesystem_permission_toml(
                entries,
                ":workspace_roots".to_string(),
                subpath.unwrap_or_else(|| ".".to_string()),
                access,
            );
        }
        FileSystemSpecialPath::Tmpdir => {
            entries.insert(
                ":tmpdir".to_string(),
                FilesystemPermissionToml::Access(access),
            );
        }
        FileSystemSpecialPath::SlashTmp => {
            entries.insert(
                ":slash_tmp".to_string(),
                FilesystemPermissionToml::Access(access),
            );
        }
        FileSystemSpecialPath::Unknown { path, subpath } => {
            if let Some(subpath) = subpath {
                insert_scoped_filesystem_permission_toml(entries, path, subpath, access);
            } else {
                entries.insert(path, FilesystemPermissionToml::Access(access));
            }
        }
    };
}

fn insert_scoped_filesystem_permission_toml(
    entries: &mut BTreeMap<String, FilesystemPermissionToml>,
    path: String,
    subpath: String,
    access: FileSystemAccessMode,
) {
    let permission = entries
        .entry(path)
        .or_insert_with(|| FilesystemPermissionToml::Scoped(BTreeMap::new()));
    match permission {
        FilesystemPermissionToml::Scoped(scoped_entries) => {
            scoped_entries.insert(subpath, access);
        }
        FilesystemPermissionToml::Access(_) => {
            *permission = FilesystemPermissionToml::Scoped(BTreeMap::from([(subpath, access)]));
        }
    }
}

pub(crate) fn network_proxy_config_for_profile_selection(
    permissions: Option<&PermissionsToml>,
    profile_name: &str,
) -> io::Result<NetworkProxyConfig> {
    if is_builtin_permission_profile_name(profile_name) {
        return Ok(NetworkProxyConfig::default());
    }
    reject_unknown_builtin_permission_profile(profile_name)?;

    let permissions = permissions.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "default_permissions requires a `[permissions]` table",
        )
    })?;
    let profile = resolve_permission_profile(permissions, profile_name)?;
    Ok(network_proxy_config_from_profile_network(
        profile.network.as_ref(),
    ))
}

/// Supplied legacy workspace-write settings, using paths owned by the execution host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceWriteSettings {
    pub writable_roots: Vec<PathUri>,
    pub network_access: bool,
    pub exclude_tmpdir_env_var: bool,
    pub exclude_slash_tmp: bool,
}

/// A compiled profile with configured roots materialized and runtime root symbols retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledPermissionProfile {
    pub permission_profile: PermissionProfile,
    pub workspace_roots: Vec<PathUri>,
}

/// Resolves a selected profile and its roots using only supplied execution-host facts.
/// Configured roots are materialized before return; symbolic roots remain available
/// for runtime workspace selection. Local callers only convert the returned URI types.
pub fn compile_permission_profile(
    permissions: Option<&PermissionsToml>,
    profile_name: &str,
    context: &ConfigPathContext,
    workspace_write: Option<&WorkspaceWriteSettings>,
    startup_warnings: &mut Vec<String>,
) -> io::Result<CompiledPermissionProfile> {
    let builtin = builtin_permission_profile(profile_name, workspace_write);
    let (file_system, network, mut workspace_roots) = if let Some(builtin) = &builtin {
        let (file_system, network) = builtin.to_runtime_permissions();
        let workspace_roots = if profile_name == BUILT_IN_WORKSPACE_PROFILE {
            workspace_write
                .into_iter()
                .flat_map(|settings| &settings.writable_roots)
                .map(|root| {
                    context
                        .resolve_against(".", root)
                        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
                })
                .collect::<io::Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        (file_system, network, workspace_roots)
    } else {
        reject_unknown_builtin_permission_profile(profile_name)?;
        let permissions = permissions.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "default_permissions requires a `[permissions]` table",
            )
        })?;
        let profile = resolve_permission_profile(permissions, profile_name)?;
        let (file_system, network) =
            compile_resolved_permission_profile(&profile, profile_name, context, startup_warnings)?;
        let workspace_roots = profile
            .workspace_roots
            .iter()
            .flat_map(WorkspaceRootsToml::enabled_roots)
            .map(|path| {
                context
                    .resolve_path(path)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
            })
            .collect::<io::Result<Vec<_>>>()?;
        (file_system, network, workspace_roots)
    };
    let mut seen = HashSet::new();
    workspace_roots.retain(|root| seen.insert(root.clone()));
    let file_system = file_system.with_materialized_project_roots_for_path_uris(&workspace_roots);
    let permission_profile = match builtin {
        Some(PermissionProfile::Disabled) => PermissionProfile::Disabled,
        _ => PermissionProfile::from_runtime_permissions(&file_system, network),
    };
    Ok(CompiledPermissionProfile {
        permission_profile,
        workspace_roots,
    })
}

fn compile_resolved_permission_profile(
    profile: &PermissionProfileToml,
    profile_name: &str,
    context: &ConfigPathContext,
    startup_warnings: &mut Vec<String>,
) -> io::Result<(FileSystemSandboxPolicy, NetworkSandboxPolicy)> {
    let mut file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(Vec::new());
    let base_network_sandbox_policy = NetworkSandboxPolicy::Restricted;
    if let Some(filesystem) = profile.filesystem.as_ref() {
        if filesystem.is_empty() && file_system_sandbox_policy.entries.is_empty() {
            push_warning(
                startup_warnings,
                missing_filesystem_entries_warning(profile_name),
            );
        } else {
            if cfg!(not(target_os = "macos")) {
                for pattern in unsupported_read_write_glob_paths(filesystem, context)? {
                    push_warning(
                        startup_warnings,
                        format!(
                            "Filesystem glob `{pattern}` uses `read` or `write` access, which is not fully supported by this platform's sandboxing. Use an exact path or trailing `/**` subtree rule instead. `deny` globs are supported."
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
                file_system_sandbox_policy
                    .entries
                    .extend(compile_filesystem_permission(
                        path,
                        permission,
                        context,
                        startup_warnings,
                    )?);
            }
        }
    } else if file_system_sandbox_policy.entries.is_empty() {
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
    if let Some(glob_scan_max_depth) = glob_scan_max_depth {
        file_system_sandbox_policy.glob_scan_max_depth = Some(glob_scan_max_depth);
    }
    let network_sandbox_policy =
        compile_network_sandbox_policy(profile.network.as_ref(), base_network_sandbox_policy);
    Ok((file_system_sandbox_policy, network_sandbox_policy))
}

pub(crate) fn reject_unknown_builtin_permission_profile(profile_name: &str) -> io::Result<()> {
    if profile_name.starts_with(':') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("default_permissions refers to unknown built-in profile `{profile_name}`"),
        ));
    }

    Ok(())
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

fn compile_network_sandbox_policy(
    network: Option<&NetworkToml>,
    base_network_sandbox_policy: NetworkSandboxPolicy,
) -> NetworkSandboxPolicy {
    let Some(network) = network else {
        return base_network_sandbox_policy;
    };

    match network.enabled {
        Some(true) => NetworkSandboxPolicy::Enabled,
        Some(false) => NetworkSandboxPolicy::Restricted,
        None => base_network_sandbox_policy,
    }
}

fn compile_filesystem_permission(
    path: &str,
    permission: &FilesystemPermissionToml,
    context: &ConfigPathContext,
    startup_warnings: &mut Vec<String>,
) -> io::Result<Vec<FileSystemSandboxEntry>> {
    let mut entries = Vec::new();
    match permission {
        FilesystemPermissionToml::Access(access) => {
            entries.push(FileSystemSandboxEntry {
                path: compile_filesystem_access_path(path, *access, context, startup_warnings)?,
                access: *access,
                missing_path_behavior: None,
            });
        }
        FilesystemPermissionToml::Scoped(scoped_entries) => {
            for (subpath, access) in scoped_entries {
                if matches!(
                    parse_special_path(path),
                    Some(FileSystemSpecialPath::ProjectRoots { .. })
                ) && context.convention().home_relative_suffix(subpath).is_some()
                {
                    permission_path::relative_subpath(subpath, context)?;
                    entries.push(FileSystemSandboxEntry {
                        path: compile_filesystem_access_path(
                            subpath,
                            *access,
                            context,
                            startup_warnings,
                        )?,
                        access: *access,
                        missing_path_behavior: None,
                    });
                    continue;
                }
                let has_glob = permission_path::contains_glob(subpath, context)?;
                let can_compile_as_pattern = match parse_special_path(path) {
                    Some(FileSystemSpecialPath::ProjectRoots { .. }) | None => true,
                    Some(_) => false,
                };
                if has_glob && *access == FileSystemAccessMode::Deny && can_compile_as_pattern {
                    // Scoped glob syntax is a first-class filesystem policy
                    // pattern entry. Literal scoped paths continue through the
                    // exact-path parser so existing path semantics stay intact.
                    let entry = FileSystemSandboxEntry {
                        path: FileSystemPath::GlobPattern {
                            pattern: compile_scoped_filesystem_pattern(
                                path, subpath, *access, context,
                            )?,
                        },
                        access: *access,
                        missing_path_behavior: None,
                    };
                    entries.push(entry);
                } else {
                    let subpath = compile_read_write_glob_path(subpath, *access, context)?;
                    entries.push(FileSystemSandboxEntry {
                        path: compile_scoped_filesystem_path(
                            path,
                            subpath,
                            context,
                            startup_warnings,
                        )?,
                        access: *access,
                        missing_path_behavior: None,
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
    context: &ConfigPathContext,
    startup_warnings: &mut Vec<String>,
) -> io::Result<FileSystemPath> {
    if !permission_path::contains_glob(path, context)? {
        return compile_filesystem_path(path, context, startup_warnings);
    }

    if access == FileSystemAccessMode::Deny {
        // At this point `path` is an unscoped filesystem table key. Top-level
        // glob deny entries still go through the absolute-path parser before
        // becoming policy patterns; relative project-root glob syntax is
        // handled by `compile_scoped_filesystem_pattern`.
        return Ok(FileSystemPath::GlobPattern {
            pattern: permission_path::absolute_path(path, context)?
                .to_config_path_string(context.convention())
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?,
        });
    }

    let path = compile_read_write_glob_path(path, access, context)?;
    compile_filesystem_path(path, context, startup_warnings)
}

fn compile_filesystem_path(
    path: &str,
    context: &ConfigPathContext,
    startup_warnings: &mut Vec<String>,
) -> io::Result<FileSystemPath> {
    if let Some(special) = parse_special_path(path) {
        maybe_push_unknown_special_path_warning(&special, startup_warnings);
        return Ok(FileSystemPath::Special { value: special });
    }

    Ok(permission_path::absolute_path(path, context)?.into())
}

fn compile_scoped_filesystem_path(
    path: &str,
    subpath: &str,
    context: &ConfigPathContext,
    startup_warnings: &mut Vec<String>,
) -> io::Result<FileSystemPath> {
    if subpath == "." {
        return compile_filesystem_path(path, context, startup_warnings);
    }

    if let Some(special) = parse_special_path(path) {
        let subpath = permission_path::relative_subpath(subpath, context)?;
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

    let subpath = permission_path::relative_subpath(subpath, context)?;
    let base = permission_path::absolute_path(path, context)?;
    let path = context
        .resolve_against(&subpath, &base)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    Ok(path.into())
}

fn compile_scoped_filesystem_pattern(
    path: &str,
    subpath: &str,
    access: FileSystemAccessMode,
    context: &ConfigPathContext,
) -> io::Result<String> {
    // Pattern entries currently mean deny-read only. Supporting broader access
    // modes here would imply glob-based read/write allow semantics that the
    // sandbox policy does not express yet.
    if access != FileSystemAccessMode::Deny {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("filesystem glob subpath `{subpath}` only supports `deny` access"),
        ));
    }
    let subpath = permission_path::relative_subpath(subpath, context)?;

    match parse_special_path(path) {
        Some(FileSystemSpecialPath::ProjectRoots { .. }) => {
            // Keep `:workspace_roots` glob patterns symbolic until the active
            // workspace roots are known, then materialize them for cwd and any
            // runtime/profile-added workspace roots together.
            Ok(project_roots_glob_pattern(Path::new(&subpath)))
        }
        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("filesystem path `{path}` does not support nested entries"),
        )),
        None => {
            let base = permission_path::absolute_path(path, context)?;
            context
                .resolve_against(&subpath, &base)
                .and_then(|path| path.to_config_path_string(context.convention()))
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
        }
    }
}

fn compile_read_write_glob_path<'a>(
    path: &'a str,
    access: FileSystemAccessMode,
    context: &ConfigPathContext,
) -> io::Result<&'a str> {
    if !permission_path::contains_glob(path, context)? {
        return Ok(path);
    }

    let path_without_trailing_glob = remove_trailing_glob_suffix(path);
    if !permission_path::contains_glob(path_without_trailing_glob, context)? {
        return Ok(path_without_trailing_glob);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "filesystem glob path `{path}` only supports `deny` access; use an exact path or trailing `/**` for `{access}` subtree access"
        ),
    ))
}

fn unsupported_read_write_glob_paths(
    filesystem: &FilesystemPermissionsToml,
    context: &ConfigPathContext,
) -> io::Result<Vec<String>> {
    let mut patterns = Vec::new();
    for (path, permission) in &filesystem.entries {
        match permission {
            FilesystemPermissionToml::Access(access) => {
                if *access != FileSystemAccessMode::Deny
                    && permission_path::contains_glob(remove_trailing_glob_suffix(path), context)?
                {
                    patterns.push(path.clone());
                }
            }
            FilesystemPermissionToml::Scoped(scoped_entries) => {
                for (subpath, access) in scoped_entries {
                    if *access != FileSystemAccessMode::Deny
                        && permission_path::contains_glob(
                            remove_trailing_glob_suffix(subpath),
                            context,
                        )?
                    {
                        patterns.push(format!("{path}/{subpath}"));
                    }
                }
            }
        }
    }
    Ok(patterns)
}

fn unbounded_unreadable_globstar_paths(filesystem: &FilesystemPermissionsToml) -> Vec<String> {
    if filesystem.glob_scan_max_depth.is_some() {
        return Vec::new();
    }

    let mut patterns = Vec::new();
    for (path, permission) in &filesystem.entries {
        match permission {
            FilesystemPermissionToml::Access(FileSystemAccessMode::Deny) => {
                if path.contains("**") {
                    patterns.push(path.clone());
                }
            }
            FilesystemPermissionToml::Access(_) => {}
            FilesystemPermissionToml::Scoped(scoped_entries) => {
                for (subpath, access) in scoped_entries {
                    if *access == FileSystemAccessMode::Deny && subpath.contains("**") {
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
        // `:project_roots` shipped before the canonical rename; keep it as an alias.
        ":project_roots" | ":workspace_roots" => {
            Some(FileSystemSpecialPath::project_roots(/*subpath*/ None))
        }
        ":tmpdir" => Some(FileSystemSpecialPath::Tmpdir),
        ":slash_tmp" => Some(FileSystemSpecialPath::SlashTmp),
        _ if path.starts_with(':') => {
            Some(FileSystemSpecialPath::unknown(path, /*subpath*/ None))
        }
        _ => None,
    }
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
                "Configured filesystem path `{path}` with nested entry `{subpath}` is not recognized by this version of Codex and will be ignored. Upgrade Codex if this path is required."
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
