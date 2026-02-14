use std::collections::HashSet;
use std::path::Components;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;
use dirs::home_dir;
use dunce::canonicalize as canonicalize_path;
use serde::Deserialize;
use tracing::warn;

use crate::config::Constrained;
use crate::config::Permissions;
use crate::config::types::ShellEnvironmentPolicy;
use crate::protocol::NetworkAccess;
use crate::protocol::AskForApproval;
use crate::protocol::ReadOnlyAccess;
use crate::protocol::SandboxPolicy;
use crate::skills::model::SkillMetadata;
use crate::bash::parse_shell_lc_plain_commands;
use crate::bash::parse_shell_lc_single_command_prefix;
#[cfg(target_os = "macos")]
use crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions;
#[cfg(not(target_os = "macos"))]
type MacOsSeatbeltProfileExtensions = ();

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub(crate) struct SkillManifestPermissions {
    #[serde(default)]
    pub(crate) network: bool,
    #[serde(default)]
    pub(crate) file_system: SkillManifestFileSystemPermissions,
    #[serde(default)]
    pub(crate) macos: SkillManifestMacOsPermissions,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub(crate) struct SkillManifestFileSystemPermissions {
    #[serde(default)]
    pub(crate) read: Vec<String>,
    #[serde(default)]
    pub(crate) write: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub(crate) struct SkillManifestMacOsPermissions {
    #[serde(default)]
    pub(crate) preferences: Option<MacOsPreferencesValue>,
    #[serde(default)]
    pub(crate) automations: Option<MacOsAutomationValue>,
    #[serde(default)]
    pub(crate) accessibility: bool,
    #[serde(default)]
    pub(crate) calendar: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(crate) enum MacOsPreferencesValue {
    Bool(bool),
    Mode(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(crate) enum MacOsAutomationValue {
    Bool(bool),
    BundleIds(Vec<String>),
}

pub(crate) fn compile_permission_profile(
    skill_dir: &Path,
    permissions: Option<SkillManifestPermissions>,
) -> Option<Permissions> {
    let permissions = permissions?;
    let fs_read = normalize_permission_paths(
        skill_dir,
        &permissions.file_system.read,
        "permissions.file_system.read",
    );
    let fs_write = normalize_permission_paths(
        skill_dir,
        &permissions.file_system.write,
        "permissions.file_system.write",
    );
    let sandbox_policy = if !fs_write.is_empty() {
        SandboxPolicy::WorkspaceWrite {
            writable_roots: fs_write,
            read_only_access: if fs_read.is_empty() {
                ReadOnlyAccess::FullAccess
            } else {
                ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: fs_read,
                }
            },
            network_access: permissions.network,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }
    } else if !fs_read.is_empty() {
        SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: fs_read,
            },
        }
    } else {
        // Default sandbox policy
        SandboxPolicy::new_read_only_policy()
    };
    let macos_seatbelt_profile_extensions =
        build_macos_seatbelt_profile_extensions(&permissions.macos);

    Some(Permissions {
        approval_policy: Constrained::allow_any(AskForApproval::Never),
        sandbox_policy: Constrained::allow_any(sandbox_policy),
        network: None,
        shell_environment_policy: ShellEnvironmentPolicy::default(),
        windows_sandbox_mode: None,
        macos_seatbelt_profile_extensions,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EffectiveCommandPermissions {
    pub(crate) approval_policy: AskForApproval,
    pub(crate) sandbox_policy: SandboxPolicy,
    pub(crate) sandbox_cwd: PathBuf,
    pub(crate) macos_seatbelt_profile_extensions: Option<MacOsSeatbeltProfileExtensions>,
}

pub(crate) fn resolve_effective_command_permissions(
    command: &[String],
    command_cwd: &Path,
    turn_approval_policy: AskForApproval,
    turn_sandbox_policy: &SandboxPolicy,
    turn_sandbox_cwd: &Path,
    turn_macos_seatbelt_profile_extensions: Option<&MacOsSeatbeltProfileExtensions>,
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<PathBuf>,
) -> EffectiveCommandPermissions {
    let mut effective = EffectiveCommandPermissions {
        approval_policy: turn_approval_policy,
        sandbox_policy: turn_sandbox_policy.clone(),
        sandbox_cwd: turn_sandbox_cwd.to_path_buf(),
        macos_seatbelt_profile_extensions: turn_macos_seatbelt_profile_extensions.cloned(),
    };

    let Some(matched_skill) =
        find_matching_skill_permission_profile(command, command_cwd, skills, disabled_paths)
    else {
        return effective;
    };

    effective.approval_policy = stricter_approval_policy(
        effective.approval_policy,
        matched_skill.profile.approval_policy.value(),
    );
    effective.sandbox_policy = merge_sandbox_policies(
        &effective.sandbox_policy,
        matched_skill.profile.sandbox_policy.get(),
    );
    effective.sandbox_cwd = matched_skill.skill_dir;
    if let Some(extensions) = matched_skill
        .profile
        .macos_seatbelt_profile_extensions
        .as_ref()
    {
        effective.macos_seatbelt_profile_extensions = Some(extensions.clone());
    }

    effective
}

fn stricter_approval_policy(lhs: AskForApproval, rhs: AskForApproval) -> AskForApproval {
    if approval_policy_rank(lhs) >= approval_policy_rank(rhs) {
        lhs
    } else {
        rhs
    }
}

fn approval_policy_rank(policy: AskForApproval) -> u8 {
    match policy {
        AskForApproval::OnFailure => 0,
        AskForApproval::OnRequest => 1,
        AskForApproval::UnlessTrusted => 2,
        AskForApproval::Never => 3,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum WriteAccessKind {
    FullAccess = 0,
    WorkspaceWrite = 1,
    ReadOnly = 2,
}

fn write_access_kind(policy: &SandboxPolicy) -> WriteAccessKind {
    match policy {
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
            WriteAccessKind::FullAccess
        }
        SandboxPolicy::WorkspaceWrite { .. } => WriteAccessKind::WorkspaceWrite,
        SandboxPolicy::ReadOnly { .. } => WriteAccessKind::ReadOnly,
    }
}

fn merge_sandbox_policies(lhs: &SandboxPolicy, rhs: &SandboxPolicy) -> SandboxPolicy {
    let network_access = lhs.has_full_network_access() && rhs.has_full_network_access();
    let strictest_write = std::cmp::max(write_access_kind(lhs), write_access_kind(rhs));
    let read_access = merge_read_only_access(read_access_for_policy(lhs), read_access_for_policy(rhs));

    match strictest_write {
        WriteAccessKind::ReadOnly => SandboxPolicy::ReadOnly { access: read_access },
        WriteAccessKind::WorkspaceWrite => {
            let lhs_workspace = workspace_policy_parts(lhs);
            let rhs_workspace = workspace_policy_parts(rhs);
            let writable_roots = merge_workspace_roots(
                lhs_workspace
                    .as_ref()
                    .map(|parts| parts.writable_roots.as_slice()),
                rhs_workspace
                    .as_ref()
                    .map(|parts| parts.writable_roots.as_slice()),
            );
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                read_only_access: read_access,
                network_access,
                exclude_tmpdir_env_var: lhs_workspace
                    .as_ref()
                    .is_some_and(|parts| parts.exclude_tmpdir_env_var)
                    || rhs_workspace
                        .as_ref()
                        .is_some_and(|parts| parts.exclude_tmpdir_env_var),
                exclude_slash_tmp: lhs_workspace
                    .as_ref()
                    .is_some_and(|parts| parts.exclude_slash_tmp)
                    || rhs_workspace
                        .as_ref()
                        .is_some_and(|parts| parts.exclude_slash_tmp),
            }
        }
        WriteAccessKind::FullAccess => {
            if network_access {
                if matches!(lhs, SandboxPolicy::ExternalSandbox { .. })
                    || matches!(rhs, SandboxPolicy::ExternalSandbox { .. })
                {
                    SandboxPolicy::ExternalSandbox {
                        network_access: NetworkAccess::Enabled,
                    }
                } else {
                    SandboxPolicy::DangerFullAccess
                }
            } else {
                SandboxPolicy::ExternalSandbox {
                    network_access: NetworkAccess::Restricted,
                }
            }
        }
    }
}

#[derive(Clone)]
struct WorkspacePolicyParts {
    writable_roots: Vec<AbsolutePathBuf>,
    exclude_tmpdir_env_var: bool,
    exclude_slash_tmp: bool,
}

fn workspace_policy_parts(policy: &SandboxPolicy) -> Option<WorkspacePolicyParts> {
    match policy {
        SandboxPolicy::WorkspaceWrite {
            writable_roots,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
            ..
        } => Some(WorkspacePolicyParts {
            writable_roots: writable_roots.clone(),
            exclude_tmpdir_env_var: *exclude_tmpdir_env_var,
            exclude_slash_tmp: *exclude_slash_tmp,
        }),
        _ => None,
    }
}

fn merge_workspace_roots(
    lhs: Option<&[AbsolutePathBuf]>,
    rhs: Option<&[AbsolutePathBuf]>,
) -> Vec<AbsolutePathBuf> {
    match (lhs, rhs) {
        (Some(lhs), Some(rhs)) => intersect_absolute_roots(lhs, rhs),
        (Some(lhs), None) => lhs.to_vec(),
        (None, Some(rhs)) => rhs.to_vec(),
        (None, None) => Vec::new(),
    }
}

fn read_access_for_policy(policy: &SandboxPolicy) -> ReadOnlyAccess {
    match policy {
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
            ReadOnlyAccess::FullAccess
        }
        SandboxPolicy::ReadOnly { access } => access.clone(),
        SandboxPolicy::WorkspaceWrite {
            read_only_access, ..
        } => read_only_access.clone(),
    }
}

fn merge_read_only_access(lhs: ReadOnlyAccess, rhs: ReadOnlyAccess) -> ReadOnlyAccess {
    match (lhs, rhs) {
        (ReadOnlyAccess::FullAccess, access) => access,
        (access, ReadOnlyAccess::FullAccess) => access,
        (
            ReadOnlyAccess::Restricted {
                include_platform_defaults: lhs_defaults,
                readable_roots: lhs_roots,
            },
            ReadOnlyAccess::Restricted {
                include_platform_defaults: rhs_defaults,
                readable_roots: rhs_roots,
            },
        ) => ReadOnlyAccess::Restricted {
            include_platform_defaults: lhs_defaults && rhs_defaults,
            readable_roots: intersect_absolute_roots(&lhs_roots, &rhs_roots),
        },
    }
}

fn intersect_absolute_roots(
    lhs: &[AbsolutePathBuf],
    rhs: &[AbsolutePathBuf],
) -> Vec<AbsolutePathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    for left in lhs {
        for right in rhs {
            let candidate = if left.as_path().starts_with(right.as_path()) {
                Some(left.clone())
            } else if right.as_path().starts_with(left.as_path()) {
                Some(right.clone())
            } else {
                None
            };
            if let Some(candidate) = candidate
                && seen.insert(candidate.to_path_buf())
            {
                roots.push(candidate);
            }
        }
    }

    roots
}

struct MatchedSkillPermissionProfile<'a> {
    profile: &'a Permissions,
    skill_dir: PathBuf,
}

fn find_matching_skill_permission_profile<'a>(
    command: &[String],
    command_cwd: &Path,
    skills: &'a [SkillMetadata],
    disabled_paths: &HashSet<PathBuf>,
) -> Option<MatchedSkillPermissionProfile<'a>> {
    let normalized_command_cwd = normalize_runtime_absolute_path(command_cwd)?;
    let command_paths = collect_command_paths(command, &normalized_command_cwd);

    skills
        .iter()
        .filter_map(|skill| {
            if disabled_paths.contains(&skill.path) {
                return None;
            }
            let profile = skill.permission_profile.as_ref()?;
            let skill_dir = skill.path.parent()?;
            let normalized_skill_dir = normalize_runtime_absolute_path(skill_dir)?;

            let matches_cwd = normalized_command_cwd.starts_with(&normalized_skill_dir);
            let matches_path = command_paths
                .iter()
                .any(|path| path.starts_with(&normalized_skill_dir));
            if !matches_cwd && !matches_path {
                return None;
            }

            Some(MatchedSkillPermissionProfile {
                profile,
                skill_dir: normalized_skill_dir,
            })
        })
        .max_by(|lhs, rhs| {
            compare_path_specificity(lhs.skill_dir.as_path(), rhs.skill_dir.as_path())
        })
}

fn compare_path_specificity(lhs: &Path, rhs: &Path) -> std::cmp::Ordering {
    let lhs_components = count_path_components(lhs.components());
    let rhs_components = count_path_components(rhs.components());
    lhs_components
        .cmp(&rhs_components)
        .then_with(|| lhs.cmp(rhs))
}

fn count_path_components(components: Components<'_>) -> usize {
    components.count()
}

fn collect_command_paths(command: &[String], command_cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    collect_paths_from_tokens(
        command.iter().map(String::as_str),
        command_cwd,
        &mut seen,
        &mut paths,
    );

    if let Some(commands) = parse_shell_lc_plain_commands(command) {
        for parsed_command in commands {
            collect_paths_from_tokens(
                parsed_command.iter().map(String::as_str),
                command_cwd,
                &mut seen,
                &mut paths,
            );
        }
    }

    if let Some(parsed_command) = parse_shell_lc_single_command_prefix(command) {
        collect_paths_from_tokens(
            parsed_command.iter().map(String::as_str),
            command_cwd,
            &mut seen,
            &mut paths,
        );
    }

    paths
}

fn collect_paths_from_tokens<'a>(
    tokens: impl Iterator<Item = &'a str>,
    command_cwd: &Path,
    seen: &mut HashSet<PathBuf>,
    paths: &mut Vec<PathBuf>,
) {
    for token in tokens {
        let mut candidates = Vec::new();
        add_token_path_candidates(token, &mut candidates);
        for candidate in candidates {
            if let Some(path) = normalize_runtime_token_path(candidate, command_cwd)
                && seen.insert(path.clone())
            {
                paths.push(path);
            }
        }
    }
}

fn add_token_path_candidates<'a>(token: &'a str, output: &mut Vec<&'a str>) {
    let trimmed = token.trim();
    if trimmed.is_empty() || trimmed.contains("://") {
        return;
    }

    if trimmed.starts_with('-') {
        if let Some((_, value)) = trimmed.split_once('=')
            && !value.trim().is_empty()
            && !value.contains("://")
        {
            output.push(value);
        }
        return;
    }

    if is_path_like_token(trimmed) {
        output.push(trimmed);
    }
}

fn is_path_like_token(token: &str) -> bool {
    token.starts_with('.')
        || token.starts_with('~')
        || token.contains('/')
        || token.contains('\\')
}

fn normalize_runtime_token_path(token: &str, command_cwd: &Path) -> Option<PathBuf> {
    let expanded = expand_home(token);
    let token_path = PathBuf::from(expanded);
    let absolute = if token_path.is_absolute() {
        token_path
    } else {
        command_cwd.join(token_path)
    };
    normalize_runtime_absolute_path(&absolute)
}

fn normalize_runtime_absolute_path(path: &Path) -> Option<PathBuf> {
    let normalized = normalize_lexically(path);
    let canonicalized = canonicalize_path(&normalized).unwrap_or(normalized);
    AbsolutePathBuf::from_absolute_path(&canonicalized)
        .ok()
        .map(AbsolutePathBuf::into_path_buf)
}

fn normalize_permission_paths(
    skill_dir: &Path,
    values: &[String],
    field: &str,
) -> Vec<AbsolutePathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    for value in values {
        let Some(path) = normalize_permission_path(skill_dir, value, field) else {
            continue;
        };
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    }

    paths
}

fn normalize_permission_path(
    skill_dir: &Path,
    value: &str,
    field: &str,
) -> Option<AbsolutePathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        warn!("ignoring {field}: value is empty");
        return None;
    }

    let expanded = expand_home(trimmed);
    let path = PathBuf::from(expanded);
    let absolute = if path.is_absolute() {
        path
    } else {
        skill_dir.join(path)
    };
    let normalized = normalize_lexically(&absolute);
    let canonicalized = canonicalize_path(&normalized).unwrap_or(normalized);
    match AbsolutePathBuf::from_absolute_path(&canonicalized) {
        Ok(path) => Some(path),
        Err(error) => {
            warn!("ignoring {field}: expected absolute path, got {canonicalized:?}: {error}");
            None
        }
    }
}

fn expand_home(path: &str) -> String {
    if path == "~" {
        if let Some(home) = home_dir() {
            return home.to_string_lossy().to_string();
        }
        return path.to_string();
    }
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest).to_string_lossy().to_string();
    }
    path.to_string()
}

#[cfg(target_os = "macos")]
fn build_macos_seatbelt_profile_extensions(
    permissions: &SkillManifestMacOsPermissions,
) -> Option<MacOsSeatbeltProfileExtensions> {
    let defaults = MacOsSeatbeltProfileExtensions::default();

    let extensions = MacOsSeatbeltProfileExtensions {
        macos_preferences: resolve_macos_preferences_permission(
            permissions.preferences.as_ref(),
            defaults.macos_preferences,
        ),
        macos_automation: resolve_macos_automation_permission(
            permissions.automations.as_ref(),
            defaults.macos_automation,
        ),
        macos_accessibility: permissions.accessibility,
        macos_calendar: permissions.calendar,
    };
    Some(extensions)
}

#[cfg(target_os = "macos")]
fn resolve_macos_preferences_permission(
    value: Option<&MacOsPreferencesValue>,
    default: crate::seatbelt_permissions::MacOsPreferencesPermission,
) -> crate::seatbelt_permissions::MacOsPreferencesPermission {
    use crate::seatbelt_permissions::MacOsPreferencesPermission;

    match value {
        Some(MacOsPreferencesValue::Bool(true)) => MacOsPreferencesPermission::ReadOnly,
        Some(MacOsPreferencesValue::Bool(false)) => MacOsPreferencesPermission::None,
        Some(MacOsPreferencesValue::Mode(mode)) => {
            let mode = mode.trim();
            if mode.eq_ignore_ascii_case("readonly") || mode.eq_ignore_ascii_case("read-only") {
                MacOsPreferencesPermission::ReadOnly
            } else if mode.eq_ignore_ascii_case("readwrite")
                || mode.eq_ignore_ascii_case("read-write")
            {
                MacOsPreferencesPermission::ReadWrite
            } else {
                warn!(
                    "ignoring permissions.macos.preferences: expected true/false, readonly, or readwrite"
                );
                default
            }
        }
        None => default,
    }
}

#[cfg(target_os = "macos")]
fn resolve_macos_automation_permission(
    value: Option<&MacOsAutomationValue>,
    default: crate::seatbelt_permissions::MacOsAutomationPermission,
) -> crate::seatbelt_permissions::MacOsAutomationPermission {
    use crate::seatbelt_permissions::MacOsAutomationPermission;

    match value {
        Some(MacOsAutomationValue::Bool(true)) => MacOsAutomationPermission::All,
        Some(MacOsAutomationValue::Bool(false)) => MacOsAutomationPermission::None,
        Some(MacOsAutomationValue::BundleIds(bundle_ids)) => {
            let bundle_ids = bundle_ids
                .iter()
                .map(|bundle_id| bundle_id.trim())
                .filter(|bundle_id| !bundle_id.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<String>>();
            if bundle_ids.is_empty() {
                MacOsAutomationPermission::None
            } else {
                MacOsAutomationPermission::BundleIds(bundle_ids)
            }
        }
        None => default,
    }
}

#[cfg(not(target_os = "macos"))]
fn build_macos_seatbelt_profile_extensions(
    _: &SkillManifestMacOsPermissions,
) -> Option<MacOsSeatbeltProfileExtensions> {
    None
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::EffectiveCommandPermissions;
    use super::SkillManifestFileSystemPermissions;
    use super::SkillManifestMacOsPermissions;
    use super::SkillManifestPermissions;
    use super::compile_permission_profile;
    use super::resolve_effective_command_permissions;
    use crate::config::Constrained;
    use crate::config::Permissions;
    use crate::config::types::ShellEnvironmentPolicy;
    use crate::protocol::AskForApproval;
    use crate::protocol::ReadOnlyAccess;
    use crate::protocol::SandboxPolicy;
    use crate::skills::model::SkillMetadata;
    use codex_protocol::protocol::SkillScope;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    fn build_skill_with_permissions(
        skill_dir: &Path,
        profile: Option<Permissions>,
        name: &str,
    ) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: "desc".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: profile,
            path: skill_dir.join("SKILL.md"),
            scope: SkillScope::User,
        }
    }

    fn assert_effective(
        effective: EffectiveCommandPermissions,
        expected_approval: AskForApproval,
        expected_sandbox_cwd: &Path,
    ) {
        assert_eq!(effective.approval_policy, expected_approval);
        assert_eq!(effective.sandbox_cwd, expected_sandbox_cwd.to_path_buf());
    }

    #[test]
    fn compile_permission_profile_normalizes_paths() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skill");
        fs::create_dir_all(skill_dir.join("scripts")).expect("skill dir");
        let read_dir = skill_dir.join("data");
        fs::create_dir_all(&read_dir).expect("read dir");

        let profile = compile_permission_profile(
            &skill_dir,
            Some(SkillManifestPermissions {
                network: true,
                file_system: SkillManifestFileSystemPermissions {
                    read: vec![
                        "./data".to_string(),
                        "./data".to_string(),
                        "scripts/../data".to_string(),
                    ],
                    write: vec!["./output".to_string()],
                },
                ..Default::default()
            }),
        )
        .expect("profile");

        assert_eq!(
            profile,
            Permissions {
                approval_policy: Constrained::allow_any(AskForApproval::Never),
                sandbox_policy: Constrained::allow_any(SandboxPolicy::WorkspaceWrite {
                    writable_roots: vec![
                        AbsolutePathBuf::try_from(skill_dir.join("output"))
                            .expect("absolute output path")
                    ],
                    read_only_access: ReadOnlyAccess::Restricted {
                        include_platform_defaults: true,
                        readable_roots: vec![
                            AbsolutePathBuf::try_from(
                                dunce::canonicalize(&read_dir).unwrap_or(read_dir)
                            )
                            .expect("absolute read path")
                        ],
                    },
                    network_access: true,
                    exclude_tmpdir_env_var: false,
                    exclude_slash_tmp: false,
                }),
                network: None,
                shell_environment_policy: ShellEnvironmentPolicy::default(),
                windows_sandbox_mode: None,
                #[cfg(target_os = "macos")]
                macos_seatbelt_profile_extensions: Some(
                    crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions::default(),
                ),
                #[cfg(not(target_os = "macos"))]
                macos_seatbelt_profile_extensions: None,
            }
        );
    }

    #[test]
    fn compile_permission_profile_without_permissions_has_empty_profile() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skill");
        fs::create_dir_all(&skill_dir).expect("skill dir");

        let profile = compile_permission_profile(&skill_dir, None);

        assert_eq!(profile, None);
    }

    #[test]
    fn resolve_effective_permissions_matches_skill_by_cwd() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let turn_cwd = tempdir.path().join("repo");
        fs::create_dir_all(&turn_cwd).expect("turn cwd");

        let skill_dir = turn_cwd.join("skills").join("demo");
        fs::create_dir_all(skill_dir.join("data")).expect("skill data");
        fs::create_dir_all(skill_dir.join("output")).expect("skill output");
        fs::write(skill_dir.join("SKILL.md"), "demo").expect("skill file");

        let profile = compile_permission_profile(
            &skill_dir,
            Some(SkillManifestPermissions {
                network: false,
                file_system: SkillManifestFileSystemPermissions {
                    read: vec!["./data".to_string()],
                    write: vec!["./output".to_string()],
                },
                ..Default::default()
            }),
        );
        let skill = build_skill_with_permissions(&skill_dir, profile, "demo");

        let effective = resolve_effective_command_permissions(
            &["/bin/echo".to_string(), "hello".to_string()],
            &skill_dir,
            AskForApproval::OnRequest,
            &SandboxPolicy::DangerFullAccess,
            &turn_cwd,
            None,
            &[skill],
            &HashSet::new(),
        );

        assert_effective(effective.clone(), AskForApproval::Never, &skill_dir);
        assert!(
            !matches!(effective.sandbox_policy, SandboxPolicy::DangerFullAccess),
            "skill profile should restrict sandbox policy"
        );
    }

    #[test]
    fn resolve_effective_permissions_matches_skill_by_command_path() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let turn_cwd = tempdir.path().join("repo");
        fs::create_dir_all(&turn_cwd).expect("turn cwd");

        let skill_dir = turn_cwd.join("skills").join("demo");
        fs::create_dir_all(skill_dir.join("bin")).expect("skill bin");
        fs::write(skill_dir.join("SKILL.md"), "demo").expect("skill file");
        let command_path = skill_dir.join("bin").join("run.sh");
        fs::write(&command_path, "#!/bin/sh\necho hi\n").expect("command file");

        let profile = compile_permission_profile(
            &skill_dir,
            Some(SkillManifestPermissions {
                network: false,
                ..Default::default()
            }),
        );
        let skill = build_skill_with_permissions(&skill_dir, profile, "demo");
        let command = vec![command_path.to_string_lossy().to_string()];

        let effective = resolve_effective_command_permissions(
            &command,
            &turn_cwd,
            AskForApproval::OnRequest,
            &SandboxPolicy::DangerFullAccess,
            &turn_cwd,
            None,
            &[skill],
            &HashSet::new(),
        );

        assert_effective(effective, AskForApproval::Never, &skill_dir);
    }

    #[test]
    fn resolve_effective_permissions_ignores_disabled_skill() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let turn_cwd = tempdir.path().join("repo");
        fs::create_dir_all(&turn_cwd).expect("turn cwd");

        let skill_dir = turn_cwd.join("skills").join("demo");
        fs::create_dir_all(&skill_dir).expect("skill dir");
        let skill_path = skill_dir.join("SKILL.md");
        fs::write(&skill_path, "demo").expect("skill file");

        let profile = compile_permission_profile(
            &skill_dir,
            Some(SkillManifestPermissions::default()),
        );
        let skill = build_skill_with_permissions(&skill_dir, profile, "demo");

        let mut disabled_paths = HashSet::new();
        disabled_paths.insert(skill_path.clone());

        let effective = resolve_effective_command_permissions(
            &["/bin/echo".to_string(), "hello".to_string()],
            &skill_dir,
            AskForApproval::OnRequest,
            &SandboxPolicy::DangerFullAccess,
            &turn_cwd,
            None,
            &[skill],
            &disabled_paths,
        );

        assert_effective(effective.clone(), AskForApproval::OnRequest, &turn_cwd);
        assert_eq!(effective.sandbox_policy, SandboxPolicy::DangerFullAccess);
    }

    #[test]
    fn resolve_effective_permissions_does_not_loosen_turn_read_only_policy() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let turn_cwd = tempdir.path().join("repo");
        fs::create_dir_all(&turn_cwd).expect("turn cwd");

        let skill_dir = turn_cwd.join("skills").join("demo");
        fs::create_dir_all(skill_dir.join("output")).expect("skill output");
        fs::write(skill_dir.join("SKILL.md"), "demo").expect("skill file");

        let profile = compile_permission_profile(
            &skill_dir,
            Some(SkillManifestPermissions {
                network: true,
                file_system: SkillManifestFileSystemPermissions {
                    read: Vec::new(),
                    write: vec!["./output".to_string()],
                },
                ..Default::default()
            }),
        );
        let skill = build_skill_with_permissions(&skill_dir, profile, "demo");

        let effective = resolve_effective_command_permissions(
            &["/bin/echo".to_string(), "hello".to_string()],
            &skill_dir,
            AskForApproval::OnRequest,
            &SandboxPolicy::new_read_only_policy(),
            &turn_cwd,
            None,
            &[skill],
            &HashSet::new(),
        );

        assert!(matches!(
            effective.sandbox_policy,
            SandboxPolicy::ReadOnly { .. }
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn compile_permission_profile_builds_macos_permission_file() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skill");
        fs::create_dir_all(&skill_dir).expect("skill dir");

        let profile = compile_permission_profile(
            &skill_dir,
            Some(SkillManifestPermissions {
                macos: SkillManifestMacOsPermissions {
                    preferences: Some(super::MacOsPreferencesValue::Mode("readwrite".to_string())),
                    automations: Some(super::MacOsAutomationValue::BundleIds(vec![
                        "com.apple.Notes".to_string(),
                    ])),
                    accessibility: true,
                    calendar: true,
                },
                ..Default::default()
            }),
        )
        .expect("profile");

        assert_eq!(
            profile.macos_seatbelt_profile_extensions,
            Some(
                crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions {
                    macos_preferences:
                        crate::seatbelt_permissions::MacOsPreferencesPermission::ReadWrite,
                    macos_automation:
                        crate::seatbelt_permissions::MacOsAutomationPermission::BundleIds(vec![
                            "com.apple.Notes".to_string()
                        ],),
                    macos_accessibility: true,
                    macos_calendar: true,
                }
            )
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn compile_permission_profile_uses_macos_defaults_when_values_missing() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skill");
        fs::create_dir_all(&skill_dir).expect("skill dir");

        let profile =
            compile_permission_profile(&skill_dir, Some(SkillManifestPermissions::default()))
                .expect("profile");

        assert_eq!(
            profile.macos_seatbelt_profile_extensions,
            Some(crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions::default())
        );
    }
}
