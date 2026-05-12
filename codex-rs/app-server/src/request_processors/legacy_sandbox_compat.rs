use super::*;

const WORKSPACE_PERMISSION_PROFILE_ID: &str = ":workspace";
const READ_ONLY_PERMISSION_PROFILE_ID: &str = ":read-only";
const DANGER_NO_SANDBOX_PERMISSION_PROFILE_ID: &str = ":danger-no-sandbox";

pub(super) struct CurrentPermissionProfile<'a> {
    pub(super) permission_profile: &'a PermissionProfile,
    pub(super) workspace_roots: &'a [AbsolutePathBuf],
}

pub(super) struct LegacySandboxProfileSelection {
    pub(super) permissions: String,
    pub(super) workspace_roots: Option<Vec<AbsolutePathBuf>>,
    expected_enforcement: codex_protocol::models::SandboxEnforcement,
    expected_network: codex_protocol::permissions::NetworkSandboxPolicy,
}

pub(super) enum LegacySandboxResolution {
    Noop,
    Selection(LegacySandboxProfileSelection),
}

pub(super) fn resolve_legacy_sandbox_profile_selection(
    sandbox_policy: &codex_app_server_protocol::SandboxPolicy,
    current: Option<CurrentPermissionProfile<'_>>,
    cwd: &AbsolutePathBuf,
    explicit_workspace_roots: Option<&[AbsolutePathBuf]>,
    field_name: &str,
) -> Result<LegacySandboxResolution, JSONRPCErrorError> {
    if legacy_sandbox_matches_current(sandbox_policy, current, cwd, explicit_workspace_roots) {
        return Ok(LegacySandboxResolution::Noop);
    }

    let expected_network =
        codex_protocol::permissions::NetworkSandboxPolicy::from(&sandbox_policy.to_core());
    match sandbox_policy {
        codex_app_server_protocol::SandboxPolicy::DangerFullAccess => Ok(
            LegacySandboxResolution::Selection(LegacySandboxProfileSelection {
                permissions: DANGER_NO_SANDBOX_PERMISSION_PROFILE_ID.to_string(),
                workspace_roots: None,
                expected_enforcement: codex_protocol::models::SandboxEnforcement::Disabled,
                expected_network,
            }),
        ),
        codex_app_server_protocol::SandboxPolicy::ReadOnly { network_access: _ } => Ok(
            LegacySandboxResolution::Selection(LegacySandboxProfileSelection {
                permissions: READ_ONLY_PERMISSION_PROFILE_ID.to_string(),
                workspace_roots: None,
                expected_enforcement: codex_protocol::models::SandboxEnforcement::Managed,
                expected_network,
            }),
        ),
        codex_app_server_protocol::SandboxPolicy::WorkspaceWrite { .. } => Ok(
            LegacySandboxResolution::Selection(LegacySandboxProfileSelection {
                permissions: WORKSPACE_PERMISSION_PROFILE_ID.to_string(),
                workspace_roots: explicit_workspace_roots
                    .is_none()
                    .then(|| workspace_roots_from_legacy_sandbox(cwd, sandbox_policy)),
                expected_enforcement: codex_protocol::models::SandboxEnforcement::Managed,
                expected_network,
            }),
        ),
        codex_app_server_protocol::SandboxPolicy::ExternalSandbox { .. } => {
            Err(invalid_request(format!(
                "`{field_name}` externalSandbox cannot be mapped to a named permissions profile"
            )))
        }
    }
}

pub(super) fn sandbox_policy_from_legacy_mode(
    sandbox_mode: SandboxMode,
) -> codex_app_server_protocol::SandboxPolicy {
    match sandbox_mode {
        SandboxMode::ReadOnly => codex_protocol::protocol::SandboxPolicy::new_read_only_policy(),
        SandboxMode::WorkspaceWrite => {
            codex_protocol::protocol::SandboxPolicy::new_workspace_write_policy()
        }
        SandboxMode::DangerFullAccess => codex_protocol::protocol::SandboxPolicy::DangerFullAccess,
    }
    .into()
}

pub(super) fn validate_legacy_sandbox_profile_selection(
    legacy_selection: &LegacySandboxProfileSelection,
    resolved_selection: &ResolvedPermissionProfileSelection,
    field_name: &str,
) -> Result<(), JSONRPCErrorError> {
    let permission_profile = &resolved_selection.permission_profile;
    if permission_profile.enforcement() != legacy_selection.expected_enforcement {
        return Err(invalid_request(format!(
            "`{field_name}` does not match permissions profile `{}`",
            legacy_selection.permissions
        )));
    }
    if permission_profile.network_sandbox_policy() != legacy_selection.expected_network {
        return Err(invalid_request(format!(
            "`{field_name}` network access does not match permissions profile `{}`",
            legacy_selection.permissions
        )));
    }
    Ok(())
}

pub(super) fn resolve_cwd_against_fallback(
    cwd: Option<&Path>,
    fallback_cwd: &AbsolutePathBuf,
) -> AbsolutePathBuf {
    match cwd {
        Some(cwd) => {
            if let Ok(path) = AbsolutePathBuf::try_from(cwd) {
                path
            } else {
                AbsolutePathBuf::resolve_path_against_base(cwd, fallback_cwd.as_path())
            }
        }
        None => fallback_cwd.clone(),
    }
}

fn legacy_sandbox_matches_current(
    sandbox_policy: &codex_app_server_protocol::SandboxPolicy,
    current: Option<CurrentPermissionProfile<'_>>,
    cwd: &AbsolutePathBuf,
    explicit_workspace_roots: Option<&[AbsolutePathBuf]>,
) -> bool {
    let Some(current) = current else {
        return false;
    };

    let materialized_profile = current
        .permission_profile
        .materialize_project_roots_with_workspace_roots(current.workspace_roots);
    let file_system_policy = materialized_profile.file_system_sandbox_policy();
    let active_sandbox = codex_sandboxing::compatibility_sandbox_policy_for_permission_profile(
        &materialized_profile,
        &file_system_policy,
        materialized_profile.network_sandbox_policy(),
        cwd.as_path(),
    );
    if active_sandbox != sandbox_policy.to_core() {
        return false;
    }

    if explicit_workspace_roots.is_some() || sandbox_policy.legacy_writable_roots().is_empty() {
        return true;
    }

    workspace_roots_from_legacy_sandbox(cwd, sandbox_policy) == current.workspace_roots
}

fn workspace_roots_from_legacy_sandbox(
    cwd: &AbsolutePathBuf,
    sandbox_policy: &codex_app_server_protocol::SandboxPolicy,
) -> Vec<AbsolutePathBuf> {
    let mut roots = Vec::with_capacity(1 + sandbox_policy.legacy_writable_roots().len());
    push_unique_root(&mut roots, cwd.clone());
    for root in sandbox_policy.legacy_writable_roots() {
        push_unique_root(&mut roots, root.clone());
    }
    roots
}

fn push_unique_root(roots: &mut Vec<AbsolutePathBuf>, root: AbsolutePathBuf) {
    if !roots.iter().any(|existing| existing == &root) {
        roots.push(root);
    }
}
