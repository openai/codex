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
    Noop {
        workspace_roots: Option<Vec<AbsolutePathBuf>>,
    },
    Selection(LegacySandboxProfileSelection),
}

pub(super) fn resolve_legacy_sandbox_profile_selection(
    sandbox_policy: &codex_app_server_protocol::SandboxPolicy,
    current: Option<CurrentPermissionProfile<'_>>,
    cwd: &AbsolutePathBuf,
    explicit_workspace_roots: Option<&[AbsolutePathBuf]>,
    field_name: &str,
) -> Result<LegacySandboxResolution, JSONRPCErrorError> {
    let legacy_workspace_roots =
        workspace_roots_from_implicit_legacy_sandbox(cwd, sandbox_policy, explicit_workspace_roots);
    if let Some(current_match) = legacy_sandbox_current_match(
        sandbox_policy,
        current,
        cwd,
        explicit_workspace_roots,
        legacy_workspace_roots.as_deref(),
    ) {
        return Ok(LegacySandboxResolution::Noop {
            workspace_roots: current_match.workspace_roots,
        });
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
                workspace_roots: legacy_workspace_roots,
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

struct LegacySandboxCurrentMatch {
    workspace_roots: Option<Vec<AbsolutePathBuf>>,
}

fn legacy_sandbox_current_match(
    sandbox_policy: &codex_app_server_protocol::SandboxPolicy,
    current: Option<CurrentPermissionProfile<'_>>,
    cwd: &AbsolutePathBuf,
    explicit_workspace_roots: Option<&[AbsolutePathBuf]>,
    legacy_workspace_roots: Option<&[AbsolutePathBuf]>,
) -> Option<LegacySandboxCurrentMatch> {
    let current = current?;

    let projection_workspace_roots = explicit_workspace_roots
        .or(legacy_workspace_roots)
        .unwrap_or(current.workspace_roots);
    let materialized_profile = current
        .permission_profile
        .materialize_project_roots_with_workspace_roots(projection_workspace_roots);
    let file_system_policy = materialized_profile.file_system_sandbox_policy();
    let active_sandbox = codex_sandboxing::compatibility_sandbox_policy_for_permission_profile(
        &materialized_profile,
        &file_system_policy,
        materialized_profile.network_sandbox_policy(),
        cwd.as_path(),
    );
    if active_sandbox != sandbox_policy.to_core() {
        return None;
    }

    let workspace_roots = legacy_workspace_roots
        .filter(|roots| *roots != current.workspace_roots)
        .map(<[AbsolutePathBuf]>::to_vec);

    Some(LegacySandboxCurrentMatch { workspace_roots })
}

fn workspace_roots_from_implicit_legacy_sandbox(
    cwd: &AbsolutePathBuf,
    sandbox_policy: &codex_app_server_protocol::SandboxPolicy,
    explicit_workspace_roots: Option<&[AbsolutePathBuf]>,
) -> Option<Vec<AbsolutePathBuf>> {
    if explicit_workspace_roots.is_some()
        || !matches!(
            sandbox_policy,
            codex_app_server_protocol::SandboxPolicy::WorkspaceWrite { .. }
        )
    {
        None
    } else {
        Some(workspace_roots_from_legacy_sandbox(cwd, sandbox_policy))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn abs_test_path(name: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::from_absolute_path(std::env::temp_dir().join(name))
            .expect("temp dir path should be absolute")
    }

    fn workspace_write_policy() -> codex_app_server_protocol::SandboxPolicy {
        codex_app_server_protocol::SandboxPolicy::WorkspaceWrite {
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
            legacy_writable_roots: Vec::new(),
        }
    }

    #[test]
    fn legacy_workspace_sandbox_updates_roots_when_current_profile_matches_new_cwd() {
        let old_root = abs_test_path("codex-old-workspace-root");
        let cwd = abs_test_path("codex-new-workspace-root");
        let policy = workspace_write_policy();

        let resolution = resolve_legacy_sandbox_profile_selection(
            &policy,
            Some(CurrentPermissionProfile {
                permission_profile: &PermissionProfile::workspace_write(),
                workspace_roots: std::slice::from_ref(&old_root),
            }),
            &cwd,
            /*explicit_workspace_roots*/ None,
            "sandboxPolicy",
        )
        .expect("legacy sandbox should resolve");

        match resolution {
            LegacySandboxResolution::Noop {
                workspace_roots: Some(workspace_roots),
            } => assert_eq!(workspace_roots, vec![cwd]),
            _ => panic!("expected workspace-roots-only resolution, got unexpected selection"),
        }
    }

    #[test]
    fn legacy_workspace_sandbox_is_noop_when_current_workspace_roots_match() {
        let cwd = abs_test_path("codex-current-workspace-root");
        let policy = workspace_write_policy();

        let resolution = resolve_legacy_sandbox_profile_selection(
            &policy,
            Some(CurrentPermissionProfile {
                permission_profile: &PermissionProfile::workspace_write(),
                workspace_roots: std::slice::from_ref(&cwd),
            }),
            &cwd,
            /*explicit_workspace_roots*/ None,
            "sandboxPolicy",
        )
        .expect("legacy sandbox should resolve");

        match resolution {
            LegacySandboxResolution::Noop {
                workspace_roots: None,
            } => {}
            _ => panic!("expected no-op resolution, got unexpected workspace roots or selection"),
        }
    }
}
