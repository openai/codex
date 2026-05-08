use super::shared::v2_enum_from_core;
use codex_protocol::approvals::ExecPolicyAmendment as CoreExecPolicyAmendment;
use codex_protocol::approvals::NetworkApprovalContext as CoreNetworkApprovalContext;
use codex_protocol::approvals::NetworkApprovalProtocol as CoreNetworkApprovalProtocol;
use codex_protocol::approvals::NetworkPolicyAmendment as CoreNetworkPolicyAmendment;
use codex_protocol::approvals::NetworkPolicyRuleAction as CoreNetworkPolicyRuleAction;
use codex_protocol::models::ActivePermissionProfile as CoreActivePermissionProfile;
use codex_protocol::models::ActivePermissionProfileModification as CoreActivePermissionProfileModification;
use codex_protocol::models::AdditionalPermissionProfile as CoreAdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions as CoreFileSystemPermissions;
use codex_protocol::models::ManagedFileSystemPermissions as CoreManagedFileSystemPermissions;
use codex_protocol::models::NetworkPermissions as CoreNetworkPermissions;
use codex_protocol::models::PermissionProfile as CorePermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode as CoreFileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath as CoreFileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry as CoreFileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSpecialPath as CoreFileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy as CoreNetworkSandboxPolicy;
use codex_protocol::protocol::NetworkAccess as CoreNetworkAccess;
use codex_protocol::request_permissions::PermissionGrantScope as CorePermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile as CoreRequestPermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use ts_rs::TS;

v2_enum_from_core! {
    pub enum NetworkApprovalProtocol from CoreNetworkApprovalProtocol {
        Http,
        Https,
        Socks5Tcp,
        Socks5Udp,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct NetworkApprovalContext {
    pub host: String,
    pub protocol: NetworkApprovalProtocol,
}

impl From<CoreNetworkApprovalContext> for NetworkApprovalContext {
    fn from(value: CoreNetworkApprovalContext) -> Self {
        Self {
            host: value.host,
            protocol: value.protocol.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AdditionalFileSystemPermissions {
    /// This will be removed in favor of `entries`.
    pub read: Option<Vec<AbsolutePathBuf>>,
    /// This will be removed in favor of `entries`.
    pub write: Option<Vec<AbsolutePathBuf>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub glob_scan_max_depth: Option<NonZeroUsize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub entries: Option<Vec<FileSystemSandboxEntry>>,
}

impl From<CoreFileSystemPermissions> for AdditionalFileSystemPermissions {
    fn from(value: CoreFileSystemPermissions) -> Self {
        if let Some((read, write)) = value.legacy_read_write_roots() {
            let mut entries = Vec::with_capacity(
                read.as_ref().map_or(0, Vec::len) + write.as_ref().map_or(0, Vec::len),
            );
            if let Some(paths) = read.as_ref() {
                entries.extend(paths.iter().map(|path| FileSystemSandboxEntry {
                    path: FileSystemPath::Path { path: path.clone() },
                    access: FileSystemAccessMode::Read,
                }));
            }
            if let Some(paths) = write.as_ref() {
                entries.extend(paths.iter().map(|path| FileSystemSandboxEntry {
                    path: FileSystemPath::Path { path: path.clone() },
                    access: FileSystemAccessMode::Write,
                }));
            }
            Self {
                read,
                write,
                glob_scan_max_depth: None,
                entries: Some(entries),
            }
        } else {
            Self {
                read: None,
                write: None,
                glob_scan_max_depth: value.glob_scan_max_depth,
                entries: Some(
                    value
                        .entries
                        .into_iter()
                        .map(FileSystemSandboxEntry::from)
                        .collect(),
                ),
            }
        }
    }
}

impl From<AdditionalFileSystemPermissions> for CoreFileSystemPermissions {
    fn from(value: AdditionalFileSystemPermissions) -> Self {
        let mut permissions = if let Some(entries) = value.entries {
            Self {
                entries: entries
                    .into_iter()
                    .map(CoreFileSystemSandboxEntry::from)
                    .collect(),
                glob_scan_max_depth: None,
            }
        } else {
            CoreFileSystemPermissions::from_read_write_roots(value.read, value.write)
        };
        permissions.glob_scan_max_depth = value.glob_scan_max_depth;
        permissions
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AdditionalNetworkPermissions {
    pub enabled: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PermissionProfileNetworkPermissions {
    pub enabled: bool,
}

impl From<CoreNetworkPermissions> for AdditionalNetworkPermissions {
    fn from(value: CoreNetworkPermissions) -> Self {
        Self {
            enabled: value.enabled,
        }
    }
}

impl From<AdditionalNetworkPermissions> for CoreNetworkPermissions {
    fn from(value: AdditionalNetworkPermissions) -> Self {
        Self {
            enabled: value.enabled,
        }
    }
}

impl From<CoreNetworkSandboxPolicy> for PermissionProfileNetworkPermissions {
    fn from(value: CoreNetworkSandboxPolicy) -> Self {
        Self {
            enabled: value.is_enabled(),
        }
    }
}

impl From<PermissionProfileNetworkPermissions> for CoreNetworkSandboxPolicy {
    fn from(value: PermissionProfileNetworkPermissions) -> Self {
        if value.enabled {
            Self::Enabled
        } else {
            Self::Restricted
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct RequestPermissionProfile {
    pub network: Option<AdditionalNetworkPermissions>,
    pub file_system: Option<AdditionalFileSystemPermissions>,
}

impl From<CoreRequestPermissionProfile> for RequestPermissionProfile {
    fn from(value: CoreRequestPermissionProfile) -> Self {
        Self {
            network: value.network.map(AdditionalNetworkPermissions::from),
            file_system: value.file_system.map(AdditionalFileSystemPermissions::from),
        }
    }
}

impl From<RequestPermissionProfile> for CoreRequestPermissionProfile {
    fn from(value: RequestPermissionProfile) -> Self {
        Self {
            network: value.network.map(CoreNetworkPermissions::from),
            file_system: value.file_system.map(CoreFileSystemPermissions::from),
        }
    }
}

v2_enum_from_core!(
    pub enum FileSystemAccessMode from CoreFileSystemAccessMode {
        Read,
        Write,
        None
    }
);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind")]
#[ts(export_to = "v2/")]
pub enum FileSystemSpecialPath {
    Root,
    Minimal,
    #[serde(alias = "current_working_directory")]
    ProjectRoots {
        subpath: Option<PathBuf>,
    },
    Tmpdir,
    SlashTmp,
    Unknown {
        path: String,
        subpath: Option<PathBuf>,
    },
}

impl From<CoreFileSystemSpecialPath> for FileSystemSpecialPath {
    fn from(value: CoreFileSystemSpecialPath) -> Self {
        match value {
            CoreFileSystemSpecialPath::Root => Self::Root,
            CoreFileSystemSpecialPath::Minimal => Self::Minimal,
            CoreFileSystemSpecialPath::ProjectRoots { subpath } => Self::ProjectRoots { subpath },
            CoreFileSystemSpecialPath::Tmpdir => Self::Tmpdir,
            CoreFileSystemSpecialPath::SlashTmp => Self::SlashTmp,
            CoreFileSystemSpecialPath::Unknown { path, subpath } => Self::Unknown { path, subpath },
        }
    }
}

impl From<FileSystemSpecialPath> for CoreFileSystemSpecialPath {
    fn from(value: FileSystemSpecialPath) -> Self {
        match value {
            FileSystemSpecialPath::Root => Self::Root,
            FileSystemSpecialPath::Minimal => Self::Minimal,
            FileSystemSpecialPath::ProjectRoots { subpath } => Self::ProjectRoots { subpath },
            FileSystemSpecialPath::Tmpdir => Self::Tmpdir,
            FileSystemSpecialPath::SlashTmp => Self::SlashTmp,
            FileSystemSpecialPath::Unknown { path, subpath } => Self::Unknown { path, subpath },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum FileSystemPath {
    Path { path: AbsolutePathBuf },
    GlobPattern { pattern: String },
    Special { value: FileSystemSpecialPath },
}

impl From<CoreFileSystemPath> for FileSystemPath {
    fn from(value: CoreFileSystemPath) -> Self {
        match value {
            CoreFileSystemPath::Path { path } => Self::Path { path },
            CoreFileSystemPath::GlobPattern { pattern } => Self::GlobPattern { pattern },
            CoreFileSystemPath::Special { value } => Self::Special {
                value: value.into(),
            },
        }
    }
}

impl From<FileSystemPath> for CoreFileSystemPath {
    fn from(value: FileSystemPath) -> Self {
        match value {
            FileSystemPath::Path { path } => Self::Path { path },
            FileSystemPath::GlobPattern { pattern } => Self::GlobPattern { pattern },
            FileSystemPath::Special { value } => Self::Special {
                value: value.into(),
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FileSystemSandboxEntry {
    pub path: FileSystemPath,
    pub access: FileSystemAccessMode,
}

impl From<CoreFileSystemSandboxEntry> for FileSystemSandboxEntry {
    fn from(value: CoreFileSystemSandboxEntry) -> Self {
        Self {
            path: value.path.into(),
            access: value.access.into(),
        }
    }
}

impl From<FileSystemSandboxEntry> for CoreFileSystemSandboxEntry {
    fn from(value: FileSystemSandboxEntry) -> Self {
        Self {
            path: value.path.into(),
            access: value.access.to_core(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum PermissionProfileFileSystemPermissions {
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Restricted {
        entries: Vec<FileSystemSandboxEntry>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        glob_scan_max_depth: Option<NonZeroUsize>,
    },
    Unrestricted,
}

impl From<CoreManagedFileSystemPermissions> for PermissionProfileFileSystemPermissions {
    fn from(value: CoreManagedFileSystemPermissions) -> Self {
        match value {
            CoreManagedFileSystemPermissions::Restricted {
                entries,
                glob_scan_max_depth,
            } => Self::Restricted {
                entries: entries
                    .into_iter()
                    .map(FileSystemSandboxEntry::from)
                    .collect(),
                glob_scan_max_depth,
            },
            CoreManagedFileSystemPermissions::Unrestricted => Self::Unrestricted,
        }
    }
}

impl From<PermissionProfileFileSystemPermissions> for CoreManagedFileSystemPermissions {
    fn from(value: PermissionProfileFileSystemPermissions) -> Self {
        match value {
            PermissionProfileFileSystemPermissions::Restricted {
                entries,
                glob_scan_max_depth,
            } => Self::Restricted {
                entries: entries
                    .into_iter()
                    .map(CoreFileSystemSandboxEntry::from)
                    .collect(),
                glob_scan_max_depth,
            },
            PermissionProfileFileSystemPermissions::Unrestricted => Self::Unrestricted,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum PermissionProfile {
    /// Codex owns sandbox construction for this profile.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Managed {
        network: PermissionProfileNetworkPermissions,
        file_system: PermissionProfileFileSystemPermissions,
    },
    /// Do not apply an outer sandbox.
    Disabled,
    /// Filesystem isolation is enforced by an external caller.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    External {
        network: PermissionProfileNetworkPermissions,
    },
}

impl From<CorePermissionProfile> for PermissionProfile {
    fn from(value: CorePermissionProfile) -> Self {
        match value {
            CorePermissionProfile::Managed {
                file_system,
                network,
            } => Self::Managed {
                network: network.into(),
                file_system: file_system.into(),
            },
            CorePermissionProfile::Disabled => Self::Disabled,
            CorePermissionProfile::External { network } => Self::External {
                network: network.into(),
            },
        }
    }
}

impl From<PermissionProfile> for CorePermissionProfile {
    fn from(value: PermissionProfile) -> Self {
        match value {
            PermissionProfile::Managed {
                file_system,
                network,
            } => Self::Managed {
                file_system: file_system.into(),
                network: network.into(),
            },
            PermissionProfile::Disabled => Self::Disabled,
            PermissionProfile::External { network } => Self::External {
                network: network.into(),
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ActivePermissionProfile {
    /// Identifier from `default_permissions` or the implicit built-in default,
    /// such as `:workspace` or a user-defined `[permissions.<id>]` profile.
    pub id: String,
    /// Parent profile identifier once permissions profiles support
    /// inheritance. This is currently always `null`.
    #[serde(default)]
    pub extends: Option<String>,
    /// Bounded user-requested modifications applied on top of the named
    /// profile, if any.
    #[serde(default)]
    pub modifications: Vec<ActivePermissionProfileModification>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum ActivePermissionProfileModification {
    /// Additional concrete directory that should be writable.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    AdditionalWritableRoot { path: AbsolutePathBuf },
}

impl From<CoreActivePermissionProfileModification> for ActivePermissionProfileModification {
    fn from(value: CoreActivePermissionProfileModification) -> Self {
        match value {
            CoreActivePermissionProfileModification::AdditionalWritableRoot { path } => {
                Self::AdditionalWritableRoot { path }
            }
        }
    }
}

impl From<ActivePermissionProfileModification> for CoreActivePermissionProfileModification {
    fn from(value: ActivePermissionProfileModification) -> Self {
        match value {
            ActivePermissionProfileModification::AdditionalWritableRoot { path } => {
                Self::AdditionalWritableRoot { path }
            }
        }
    }
}

impl From<CoreActivePermissionProfile> for ActivePermissionProfile {
    fn from(value: CoreActivePermissionProfile) -> Self {
        Self {
            id: value.id,
            extends: value.extends,
            modifications: value
                .modifications
                .into_iter()
                .map(ActivePermissionProfileModification::from)
                .collect(),
        }
    }
}

impl From<ActivePermissionProfile> for CoreActivePermissionProfile {
    fn from(value: ActivePermissionProfile) -> Self {
        Self {
            id: value.id,
            extends: value.extends,
            modifications: value
                .modifications
                .into_iter()
                .map(CoreActivePermissionProfileModification::from)
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum PermissionProfileSelectionParams {
    /// Select a named built-in or user-defined profile and optionally apply
    /// bounded modifications that Codex knows how to validate.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Profile {
        id: String,
        #[ts(optional = nullable)]
        modifications: Option<Vec<PermissionProfileModificationParams>>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum PermissionProfileModificationParams {
    /// Additional concrete directory that should be writable.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    AdditionalWritableRoot { path: AbsolutePathBuf },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AdditionalPermissionProfile {
    /// Partial overlay used for per-command permission requests.
    pub network: Option<AdditionalNetworkPermissions>,
    pub file_system: Option<AdditionalFileSystemPermissions>,
}

impl From<CoreAdditionalPermissionProfile> for AdditionalPermissionProfile {
    fn from(value: CoreAdditionalPermissionProfile) -> Self {
        Self {
            network: value.network.map(AdditionalNetworkPermissions::from),
            file_system: value.file_system.map(AdditionalFileSystemPermissions::from),
        }
    }
}

impl From<AdditionalPermissionProfile> for CoreAdditionalPermissionProfile {
    fn from(value: AdditionalPermissionProfile) -> Self {
        Self {
            network: value.network.map(CoreNetworkPermissions::from),
            file_system: value.file_system.map(CoreFileSystemPermissions::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GrantedPermissionProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub network: Option<AdditionalNetworkPermissions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub file_system: Option<AdditionalFileSystemPermissions>,
}

impl From<GrantedPermissionProfile> for CoreAdditionalPermissionProfile {
    fn from(value: GrantedPermissionProfile) -> Self {
        Self {
            network: value.network.map(CoreNetworkPermissions::from),
            file_system: value.file_system.map(CoreFileSystemPermissions::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum NetworkAccess {
    #[default]
    Restricted,
    Enabled,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum SandboxPolicy {
    DangerFullAccess,
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    ReadOnly {
        #[serde(default)]
        network_access: bool,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    ExternalSandbox {
        #[serde(default)]
        network_access: NetworkAccess,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    WorkspaceWrite {
        #[serde(default)]
        writable_roots: Vec<AbsolutePathBuf>,
        #[serde(default)]
        network_access: bool,
        #[serde(default)]
        exclude_tmpdir_env_var: bool,
        #[serde(default)]
        exclude_slash_tmp: bool,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum SandboxPolicyDeserialize {
    DangerFullAccess,
    #[serde(rename_all = "camelCase")]
    ReadOnly {
        #[serde(default)]
        network_access: bool,
        #[serde(default)]
        access: Option<LegacyReadOnlyAccess>,
    },
    #[serde(rename_all = "camelCase")]
    ExternalSandbox {
        #[serde(default)]
        network_access: NetworkAccess,
    },
    #[serde(rename_all = "camelCase")]
    WorkspaceWrite {
        #[serde(default)]
        writable_roots: Vec<AbsolutePathBuf>,
        #[serde(default)]
        read_only_access: Option<LegacyReadOnlyAccess>,
        #[serde(default)]
        network_access: bool,
        #[serde(default)]
        exclude_tmpdir_env_var: bool,
        #[serde(default)]
        exclude_slash_tmp: bool,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum LegacyReadOnlyAccess {
    FullAccess,
    Restricted,
}

impl<'de> Deserialize<'de> for SandboxPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match SandboxPolicyDeserialize::deserialize(deserializer)? {
            SandboxPolicyDeserialize::DangerFullAccess => Ok(SandboxPolicy::DangerFullAccess),
            SandboxPolicyDeserialize::ReadOnly {
                network_access,
                access,
            } => {
                if matches!(access, Some(LegacyReadOnlyAccess::Restricted)) {
                    return Err(serde::de::Error::custom(
                        "readOnly.access is no longer supported; use permissionProfile for restricted reads",
                    ));
                }
                Ok(SandboxPolicy::ReadOnly { network_access })
            }
            SandboxPolicyDeserialize::ExternalSandbox { network_access } => {
                Ok(SandboxPolicy::ExternalSandbox { network_access })
            }
            SandboxPolicyDeserialize::WorkspaceWrite {
                writable_roots,
                read_only_access,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            } => {
                if matches!(read_only_access, Some(LegacyReadOnlyAccess::Restricted)) {
                    return Err(serde::de::Error::custom(
                        "workspaceWrite.readOnlyAccess is no longer supported; use permissionProfile for restricted reads",
                    ));
                }
                Ok(SandboxPolicy::WorkspaceWrite {
                    writable_roots,
                    network_access,
                    exclude_tmpdir_env_var,
                    exclude_slash_tmp,
                })
            }
        }
    }
}

impl SandboxPolicy {
    pub fn to_core(&self) -> codex_protocol::protocol::SandboxPolicy {
        match self {
            SandboxPolicy::DangerFullAccess => {
                codex_protocol::protocol::SandboxPolicy::DangerFullAccess
            }
            SandboxPolicy::ReadOnly { network_access } => {
                codex_protocol::protocol::SandboxPolicy::ReadOnly {
                    network_access: *network_access,
                }
            }
            SandboxPolicy::ExternalSandbox { network_access } => {
                codex_protocol::protocol::SandboxPolicy::ExternalSandbox {
                    network_access: match network_access {
                        NetworkAccess::Restricted => CoreNetworkAccess::Restricted,
                        NetworkAccess::Enabled => CoreNetworkAccess::Enabled,
                    },
                }
            }
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            } => codex_protocol::protocol::SandboxPolicy::WorkspaceWrite {
                writable_roots: writable_roots.clone(),
                network_access: *network_access,
                exclude_tmpdir_env_var: *exclude_tmpdir_env_var,
                exclude_slash_tmp: *exclude_slash_tmp,
            },
        }
    }
}

impl From<codex_protocol::protocol::SandboxPolicy> for SandboxPolicy {
    fn from(value: codex_protocol::protocol::SandboxPolicy) -> Self {
        match value {
            codex_protocol::protocol::SandboxPolicy::DangerFullAccess => {
                SandboxPolicy::DangerFullAccess
            }
            codex_protocol::protocol::SandboxPolicy::ReadOnly { network_access } => {
                SandboxPolicy::ReadOnly { network_access }
            }
            codex_protocol::protocol::SandboxPolicy::ExternalSandbox { network_access } => {
                SandboxPolicy::ExternalSandbox {
                    network_access: match network_access {
                        CoreNetworkAccess::Restricted => NetworkAccess::Restricted,
                        CoreNetworkAccess::Enabled => NetworkAccess::Enabled,
                    },
                }
            }
            codex_protocol::protocol::SandboxPolicy::WorkspaceWrite {
                writable_roots,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            } => SandboxPolicy::WorkspaceWrite {
                writable_roots,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(transparent)]
#[ts(type = "Array<string>", export_to = "v2/")]
pub struct ExecPolicyAmendment {
    pub command: Vec<String>,
}

impl ExecPolicyAmendment {
    pub fn into_core(self) -> CoreExecPolicyAmendment {
        CoreExecPolicyAmendment::new(self.command)
    }
}

impl From<CoreExecPolicyAmendment> for ExecPolicyAmendment {
    fn from(value: CoreExecPolicyAmendment) -> Self {
        Self {
            command: value.command().to_vec(),
        }
    }
}

v2_enum_from_core!(
    pub enum NetworkPolicyRuleAction from CoreNetworkPolicyRuleAction {
        Allow, Deny
    }
);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct NetworkPolicyAmendment {
    pub host: String,
    pub action: NetworkPolicyRuleAction,
}

impl NetworkPolicyAmendment {
    pub fn into_core(self) -> CoreNetworkPolicyAmendment {
        CoreNetworkPolicyAmendment {
            host: self.host,
            action: self.action.to_core(),
        }
    }
}

impl From<CoreNetworkPolicyAmendment> for NetworkPolicyAmendment {
    fn from(value: CoreNetworkPolicyAmendment) -> Self {
        Self {
            host: value.host,
            action: NetworkPolicyRuleAction::from(value.action),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PermissionsRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    /// Unix timestamp (in milliseconds) when this approval request started.
    #[ts(type = "number")]
    pub started_at_ms: i64,
    pub cwd: AbsolutePathBuf,
    pub reason: Option<String>,
    pub permissions: RequestPermissionProfile,
}

v2_enum_from_core!(
    #[derive(Default)]
    pub enum PermissionGrantScope from CorePermissionGrantScope {
        #[default]
        Turn,
        Session
    }
);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PermissionsRequestApprovalResponse {
    pub permissions: GrantedPermissionProfile,
    #[serde(default)]
    pub scope: PermissionGrantScope,
    /// Review every subsequent command in this turn before normal sandboxed execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub strict_auto_review: Option<bool>,
}
