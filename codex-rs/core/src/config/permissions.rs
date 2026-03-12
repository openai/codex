use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_network_proxy::NetworkMode;
use codex_network_proxy::NetworkProxyConfig;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::MacOsAutomationPermission;
use codex_protocol::models::MacOsPreferencesPermission;
use codex_protocol::models::MacOsSeatbeltProfileExtensions;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
pub struct PermissionsToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, PermissionProfileToml>,
}

impl PermissionsToml {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct PermissionProfileToml {
    pub filesystem: Option<FilesystemPermissionsToml>,
    pub network: Option<NetworkToml>,
    pub macos: Option<MacOsPermissionsToml>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
pub struct FilesystemPermissionsToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, FilesystemPermissionToml>,
}

impl FilesystemPermissionsToml {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[serde(untagged)]
pub enum FilesystemPermissionToml {
    Access(FileSystemAccessMode),
    Scoped(BTreeMap<String, FileSystemAccessMode>),
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct NetworkToml {
    pub enabled: Option<bool>,
    pub proxy_url: Option<String>,
    pub enable_socks5: Option<bool>,
    pub socks_url: Option<String>,
    pub enable_socks5_udp: Option<bool>,
    pub allow_upstream_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_proxy: Option<bool>,
    pub dangerously_allow_all_unix_sockets: Option<bool>,
    #[schemars(with = "Option<NetworkModeSchema>")]
    pub mode: Option<NetworkMode>,
    pub allowed_domains: Option<Vec<String>>,
    pub denied_domains: Option<Vec<String>>,
    pub allow_unix_sockets: Option<Vec<String>>,
    pub allow_local_binding: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct MacOsPermissionsToml {
    pub preferences: Option<MacOsPreferencesPermission>,
    pub automations: Option<MacOsAutomationPermission>,
    pub accessibility: Option<bool>,
    pub calendar: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum NetworkModeSchema {
    Limited,
    Full,
}

impl NetworkToml {
    pub(crate) fn apply_to_network_proxy_config(&self, config: &mut NetworkProxyConfig) {
        if let Some(enabled) = self.enabled {
            config.network.enabled = enabled;
        }
        if let Some(proxy_url) = self.proxy_url.as_ref() {
            config.network.proxy_url = proxy_url.clone();
        }
        if let Some(enable_socks5) = self.enable_socks5 {
            config.network.enable_socks5 = enable_socks5;
        }
        if let Some(socks_url) = self.socks_url.as_ref() {
            config.network.socks_url = socks_url.clone();
        }
        if let Some(enable_socks5_udp) = self.enable_socks5_udp {
            config.network.enable_socks5_udp = enable_socks5_udp;
        }
        if let Some(allow_upstream_proxy) = self.allow_upstream_proxy {
            config.network.allow_upstream_proxy = allow_upstream_proxy;
        }
        if let Some(dangerously_allow_non_loopback_proxy) =
            self.dangerously_allow_non_loopback_proxy
        {
            config.network.dangerously_allow_non_loopback_proxy =
                dangerously_allow_non_loopback_proxy;
        }
        if let Some(dangerously_allow_all_unix_sockets) = self.dangerously_allow_all_unix_sockets {
            config.network.dangerously_allow_all_unix_sockets = dangerously_allow_all_unix_sockets;
        }
        if let Some(mode) = self.mode {
            config.network.mode = mode;
        }
        if let Some(allowed_domains) = self.allowed_domains.as_ref() {
            config.network.allowed_domains = allowed_domains.clone();
        }
        if let Some(denied_domains) = self.denied_domains.as_ref() {
            config.network.denied_domains = denied_domains.clone();
        }
        if let Some(allow_unix_sockets) = self.allow_unix_sockets.as_ref() {
            config.network.allow_unix_sockets = allow_unix_sockets.clone();
        }
        if let Some(allow_local_binding) = self.allow_local_binding {
            config.network.allow_local_binding = allow_local_binding;
        }
    }

    pub(crate) fn to_network_proxy_config(&self) -> NetworkProxyConfig {
        let mut config = NetworkProxyConfig::default();
        self.apply_to_network_proxy_config(&mut config);
        config
    }
}

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
    startup_warnings: &mut Vec<String>,
) -> io::Result<(
    FileSystemSandboxPolicy,
    NetworkSandboxPolicy,
    Option<MacOsSeatbeltProfileExtensions>,
)> {
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
    let macos_seatbelt_profile_extensions = profile.macos.as_ref().map(macos_permissions_from_toml);

    Ok((
        FileSystemSandboxPolicy::restricted(entries),
        network_sandbox_policy,
        macos_seatbelt_profile_extensions,
    ))
}

pub(crate) fn merge_permission_profile_toml(
    base: Option<&PermissionProfileToml>,
    permissions: &PermissionProfileToml,
) -> PermissionProfileToml {
    let filesystem = match (
        base.and_then(|base| base.filesystem.as_ref()),
        permissions.filesystem.as_ref(),
    ) {
        (Some(base), Some(permissions)) => {
            Some(merge_filesystem_permissions_toml(base, permissions))
        }
        (Some(base), None) => Some(base.clone()),
        (None, Some(permissions)) => Some(permissions.clone()),
        (None, None) => None,
    };
    let network = match (
        base.and_then(|base| base.network.as_ref()),
        permissions.network.as_ref(),
    ) {
        (Some(base), Some(permissions)) => Some(merge_network_toml(base, permissions)),
        (Some(base), None) => Some(base.clone()),
        (None, Some(permissions)) => Some(permissions.clone()),
        (None, None) => None,
    };
    let macos = match (
        base.and_then(|base| base.macos.as_ref()),
        permissions.macos.as_ref(),
    ) {
        (Some(base), Some(permissions)) => Some(merge_macos_permissions_toml(base, permissions)),
        (Some(base), None) => Some(base.clone()),
        (None, Some(permissions)) => Some(permissions.clone()),
        (None, None) => None,
    };

    PermissionProfileToml {
        filesystem,
        network,
        macos,
    }
}

pub(crate) fn permission_profile_toml_from_runtime_permissions(
    permissions: &PermissionProfile,
) -> PermissionProfileToml {
    let filesystem = permissions
        .file_system
        .as_ref()
        .map(filesystem_permissions_toml_from_runtime)
        .filter(|filesystem| !filesystem.is_empty());
    let network = permissions
        .network
        .as_ref()
        .map(network_toml_from_runtime)
        .filter(network_toml_has_values);
    let macos = permissions
        .macos
        .as_ref()
        .map(macos_permissions_toml_from_runtime)
        .filter(macos_permissions_toml_has_values);

    PermissionProfileToml {
        filesystem,
        network,
        macos,
    }
}

pub(crate) fn permission_profile_toml_from_effective_permissions(
    file_system: &FileSystemSandboxPolicy,
    network: &NetworkProxyConfig,
    macos: Option<&MacOsSeatbeltProfileExtensions>,
) -> io::Result<PermissionProfileToml> {
    let filesystem = filesystem_permissions_toml_from_policy(file_system)?;
    let network = network_toml_from_proxy_config(network);
    let macos = macos
        .map(macos_permissions_toml_from_runtime)
        .filter(macos_permissions_toml_has_values);

    Ok(PermissionProfileToml {
        filesystem: Some(filesystem).filter(|filesystem| !filesystem.is_empty()),
        network: Some(network).filter(network_toml_has_values),
        macos,
    })
}

fn filesystem_permissions_toml_from_runtime(
    permissions: &FileSystemPermissions,
) -> FilesystemPermissionsToml {
    let mut entries = BTreeMap::new();

    if let Some(read) = permissions.read.as_ref() {
        for path in read {
            merge_filesystem_permission_entry(
                &mut entries,
                path.to_string_lossy().as_ref(),
                FilesystemPermissionToml::Access(FileSystemAccessMode::Read),
            );
        }
    }
    if let Some(write) = permissions.write.as_ref() {
        for path in write {
            merge_filesystem_permission_entry(
                &mut entries,
                path.to_string_lossy().as_ref(),
                FilesystemPermissionToml::Access(FileSystemAccessMode::Write),
            );
        }
    }

    FilesystemPermissionsToml { entries }
}

fn filesystem_permissions_toml_from_policy(
    policy: &FileSystemSandboxPolicy,
) -> io::Result<FilesystemPermissionsToml> {
    let mut entries = BTreeMap::new();

    for entry in &policy.entries {
        let (path, permission) = filesystem_permission_toml_from_entry(entry)?;
        merge_filesystem_permission_entry(&mut entries, &path, permission);
    }

    Ok(FilesystemPermissionsToml { entries })
}

fn filesystem_permission_toml_from_entry(
    entry: &FileSystemSandboxEntry,
) -> io::Result<(String, FilesystemPermissionToml)> {
    let access = entry.access;
    match &entry.path {
        FileSystemPath::Path { path } => Ok((
            path.to_string_lossy().to_string(),
            FilesystemPermissionToml::Access(access),
        )),
        FileSystemPath::Special { value } => match value {
            FileSystemSpecialPath::Root => Ok((
                ":root".to_string(),
                FilesystemPermissionToml::Access(access),
            )),
            FileSystemSpecialPath::Minimal => Ok((
                ":minimal".to_string(),
                FilesystemPermissionToml::Access(access),
            )),
            FileSystemSpecialPath::CurrentWorkingDirectory => {
                Ok((":cwd".to_string(), FilesystemPermissionToml::Access(access)))
            }
            FileSystemSpecialPath::ProjectRoots { subpath: None } => Ok((
                ":project_roots".to_string(),
                FilesystemPermissionToml::Access(access),
            )),
            FileSystemSpecialPath::ProjectRoots {
                subpath: Some(subpath),
            } => Ok((
                ":project_roots".to_string(),
                FilesystemPermissionToml::Scoped(BTreeMap::from([(
                    subpath.to_string_lossy().to_string(),
                    access,
                )])),
            )),
            FileSystemSpecialPath::Tmpdir => Ok((
                ":tmpdir".to_string(),
                FilesystemPermissionToml::Access(access),
            )),
            FileSystemSpecialPath::SlashTmp => Ok((
                ":slash_tmp".to_string(),
                FilesystemPermissionToml::Access(access),
            )),
            FileSystemSpecialPath::Unknown {
                path,
                subpath: None,
            } => Ok((path.clone(), FilesystemPermissionToml::Access(access))),
            FileSystemSpecialPath::Unknown {
                path,
                subpath: Some(subpath),
            } => Ok((
                path.clone(),
                FilesystemPermissionToml::Scoped(BTreeMap::from([(
                    subpath.to_string_lossy().to_string(),
                    access,
                )])),
            )),
        },
    }
}

fn network_toml_from_runtime(permissions: &NetworkPermissions) -> NetworkToml {
    NetworkToml {
        enabled: permissions.enabled,
        ..Default::default()
    }
}

fn network_toml_from_proxy_config(config: &NetworkProxyConfig) -> NetworkToml {
    let defaults = NetworkProxyConfig::default();
    NetworkToml {
        enabled: config.network.enabled.then_some(true),
        proxy_url: (config.network.proxy_url != defaults.network.proxy_url)
            .then(|| config.network.proxy_url.clone()),
        enable_socks5: (config.network.enable_socks5 != defaults.network.enable_socks5)
            .then_some(config.network.enable_socks5),
        socks_url: (config.network.socks_url != defaults.network.socks_url)
            .then(|| config.network.socks_url.clone()),
        enable_socks5_udp: (config.network.enable_socks5_udp != defaults.network.enable_socks5_udp)
            .then_some(config.network.enable_socks5_udp),
        allow_upstream_proxy: (config.network.allow_upstream_proxy
            != defaults.network.allow_upstream_proxy)
            .then_some(config.network.allow_upstream_proxy),
        dangerously_allow_non_loopback_proxy: (config.network.dangerously_allow_non_loopback_proxy
            != defaults.network.dangerously_allow_non_loopback_proxy)
            .then_some(config.network.dangerously_allow_non_loopback_proxy),
        dangerously_allow_all_unix_sockets: (config.network.dangerously_allow_all_unix_sockets
            != defaults.network.dangerously_allow_all_unix_sockets)
            .then_some(config.network.dangerously_allow_all_unix_sockets),
        mode: (config.network.mode != defaults.network.mode).then_some(config.network.mode),
        allowed_domains: Some(config.network.allowed_domains.clone())
            .filter(|allowed_domains| !allowed_domains.is_empty()),
        denied_domains: Some(config.network.denied_domains.clone())
            .filter(|denied_domains| !denied_domains.is_empty()),
        allow_unix_sockets: Some(config.network.allow_unix_sockets.clone())
            .filter(|allow_unix_sockets| !allow_unix_sockets.is_empty()),
        allow_local_binding: (config.network.allow_local_binding
            != defaults.network.allow_local_binding)
            .then_some(config.network.allow_local_binding),
    }
}

fn macos_permissions_toml_from_runtime(
    permissions: &MacOsSeatbeltProfileExtensions,
) -> MacOsPermissionsToml {
    MacOsPermissionsToml {
        preferences: Some(permissions.macos_preferences.clone()),
        automations: Some(permissions.macos_automation.clone()),
        accessibility: Some(permissions.macos_accessibility),
        calendar: Some(permissions.macos_calendar),
    }
}

fn macos_permissions_from_toml(
    permissions: &MacOsPermissionsToml,
) -> MacOsSeatbeltProfileExtensions {
    MacOsSeatbeltProfileExtensions {
        macos_preferences: permissions.preferences.clone().unwrap_or_default(),
        macos_automation: permissions.automations.clone().unwrap_or_default(),
        macos_accessibility: permissions.accessibility.unwrap_or(false),
        macos_calendar: permissions.calendar.unwrap_or(false),
    }
}

fn merge_filesystem_permissions_toml(
    base: &FilesystemPermissionsToml,
    permissions: &FilesystemPermissionsToml,
) -> FilesystemPermissionsToml {
    let mut entries = base.entries.clone();
    for (path, permission) in &permissions.entries {
        merge_filesystem_permission_entry(&mut entries, path, permission.clone());
    }
    FilesystemPermissionsToml { entries }
}

fn merge_filesystem_permission_entry(
    entries: &mut BTreeMap<String, FilesystemPermissionToml>,
    path: &str,
    permission: FilesystemPermissionToml,
) {
    entries
        .entry(path.to_string())
        .and_modify(|existing| {
            *existing = merge_filesystem_permission_toml(existing.clone(), permission.clone());
        })
        .or_insert(permission);
}

fn merge_filesystem_permission_toml(
    base: FilesystemPermissionToml,
    permissions: FilesystemPermissionToml,
) -> FilesystemPermissionToml {
    match (base, permissions) {
        (FilesystemPermissionToml::Access(base), FilesystemPermissionToml::Access(permissions)) => {
            FilesystemPermissionToml::Access(more_permissive_access(base, permissions))
        }
        (FilesystemPermissionToml::Scoped(base), FilesystemPermissionToml::Scoped(permissions)) => {
            FilesystemPermissionToml::Scoped(merge_scoped_permissions(base, permissions))
        }
        (FilesystemPermissionToml::Access(base), FilesystemPermissionToml::Scoped(permissions)) => {
            FilesystemPermissionToml::Scoped(merge_scoped_permissions(
                BTreeMap::from([(".".to_string(), base)]),
                permissions,
            ))
        }
        (FilesystemPermissionToml::Scoped(base), FilesystemPermissionToml::Access(permissions)) => {
            FilesystemPermissionToml::Scoped(merge_scoped_permissions(
                base,
                BTreeMap::from([(".".to_string(), permissions)]),
            ))
        }
    }
}

fn merge_scoped_permissions(
    mut base: BTreeMap<String, FileSystemAccessMode>,
    permissions: BTreeMap<String, FileSystemAccessMode>,
) -> BTreeMap<String, FileSystemAccessMode> {
    for (subpath, access) in permissions {
        base.entry(subpath)
            .and_modify(|existing| *existing = more_permissive_access(*existing, access))
            .or_insert(access);
    }
    base
}

fn more_permissive_access(
    base: FileSystemAccessMode,
    permissions: FileSystemAccessMode,
) -> FileSystemAccessMode {
    match (base, permissions) {
        (FileSystemAccessMode::Write, _) | (_, FileSystemAccessMode::Write) => {
            FileSystemAccessMode::Write
        }
        (FileSystemAccessMode::Read, _) | (_, FileSystemAccessMode::Read) => {
            FileSystemAccessMode::Read
        }
        _ => FileSystemAccessMode::None,
    }
}

fn merge_network_toml(base: &NetworkToml, permissions: &NetworkToml) -> NetworkToml {
    NetworkToml {
        enabled: permissions.enabled.or(base.enabled),
        proxy_url: permissions
            .proxy_url
            .clone()
            .or_else(|| base.proxy_url.clone()),
        enable_socks5: permissions.enable_socks5.or(base.enable_socks5),
        socks_url: permissions
            .socks_url
            .clone()
            .or_else(|| base.socks_url.clone()),
        enable_socks5_udp: permissions.enable_socks5_udp.or(base.enable_socks5_udp),
        allow_upstream_proxy: permissions
            .allow_upstream_proxy
            .or(base.allow_upstream_proxy),
        dangerously_allow_non_loopback_proxy: permissions
            .dangerously_allow_non_loopback_proxy
            .or(base.dangerously_allow_non_loopback_proxy),
        dangerously_allow_all_unix_sockets: permissions
            .dangerously_allow_all_unix_sockets
            .or(base.dangerously_allow_all_unix_sockets),
        mode: permissions.mode.or(base.mode),
        allowed_domains: permissions
            .allowed_domains
            .clone()
            .or_else(|| base.allowed_domains.clone()),
        denied_domains: permissions
            .denied_domains
            .clone()
            .or_else(|| base.denied_domains.clone()),
        allow_unix_sockets: permissions
            .allow_unix_sockets
            .clone()
            .or_else(|| base.allow_unix_sockets.clone()),
        allow_local_binding: permissions.allow_local_binding.or(base.allow_local_binding),
    }
}

fn merge_macos_permissions_toml(
    base: &MacOsPermissionsToml,
    permissions: &MacOsPermissionsToml,
) -> MacOsPermissionsToml {
    let base_runtime = macos_permissions_from_toml(base);
    let permissions_runtime = macos_permissions_from_toml(permissions);
    macos_permissions_toml_from_runtime(
        &crate::sandboxing::macos_permissions::merge_macos_seatbelt_profile_extensions(
            Some(&base_runtime),
            Some(&permissions_runtime),
        )
        .unwrap_or_default(),
    )
}

fn network_toml_has_values(network: &NetworkToml) -> bool {
    network.enabled.is_some()
        || network.proxy_url.is_some()
        || network.enable_socks5.is_some()
        || network.socks_url.is_some()
        || network.enable_socks5_udp.is_some()
        || network.allow_upstream_proxy.is_some()
        || network.dangerously_allow_non_loopback_proxy.is_some()
        || network.dangerously_allow_all_unix_sockets.is_some()
        || network.mode.is_some()
        || network.allowed_domains.is_some()
        || network.denied_domains.is_some()
        || network.allow_unix_sockets.is_some()
        || network.allow_local_binding.is_some()
}

fn macos_permissions_toml_has_values(permissions: &MacOsPermissionsToml) -> bool {
    permissions.preferences.is_some()
        || permissions.automations.is_some()
        || permissions.accessibility.is_some()
        || permissions.calendar.is_some()
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
    let path = AbsolutePathBuf::resolve_path_against_base(&subpath, base.as_path())?;
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
        ":cwd" => Some(FileSystemSpecialPath::CurrentWorkingDirectory),
        ":project_roots" => Some(FileSystemSpecialPath::project_roots(None)),
        ":tmpdir" => Some(FileSystemSpecialPath::Tmpdir),
        ":slash_tmp" => Some(FileSystemSpecialPath::SlashTmp),
        _ if path.starts_with(':') => Some(FileSystemSpecialPath::unknown(path, None)),
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
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn normalize_absolute_path_for_platform_simplifies_windows_verbatim_paths() {
        let parsed =
            normalize_absolute_path_for_platform(r"\\?\D:\c\x\worktrees\2508\swift-base", true);
        assert_eq!(parsed, PathBuf::from(r"D:\c\x\worktrees\2508\swift-base"));
    }
}
