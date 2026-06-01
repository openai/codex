use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_READ_ONLY;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_WORKSPACE;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;

/// A simple preset pairing an approval policy with a permission profile.
#[derive(Debug, Clone)]
pub struct ApprovalPreset {
    /// Stable identifier for the preset.
    pub id: &'static str,
    /// Display label shown in UIs.
    pub label: &'static str,
    /// Short human description shown next to the label in UIs.
    pub description: &'static str,
    /// Approval policy to apply.
    pub approval: AskForApproval,
    /// Built-in permission profile selected by this preset.
    pub active_permission_profile: ActivePermissionProfile,
    /// Permission profile to apply.
    pub permission_profile: PermissionProfile,
}

/// Built-in list of approval presets that pair approval and permissions.
///
/// Keep this UI-agnostic so it can be reused by both TUI and MCP server.
pub fn builtin_approval_presets() -> Vec<ApprovalPreset> {
    vec![
        ApprovalPreset {
            id: "read-only",
            label: "Read Only",
            description: "Codex can read files in the current workspace. Approval is required to edit files or access the internet.",
            approval: AskForApproval::OnRequest,
            active_permission_profile: ActivePermissionProfile::new(
                BUILT_IN_PERMISSION_PROFILE_READ_ONLY,
            ),
            permission_profile: PermissionProfile::read_only(),
        },
        ApprovalPreset {
            id: "auto",
            label: "Default",
            description: "Codex can read and edit files in the current workspace, and run commands. Approval is required to access the internet or edit other files. (Identical to Agent mode)",
            approval: AskForApproval::OnRequest,
            active_permission_profile: ActivePermissionProfile::new(
                BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
            ),
            permission_profile: PermissionProfile::workspace_write(),
        },
        ApprovalPreset {
            id: "full-access",
            label: "Full Access",
            description: "Codex can edit files outside this workspace and access the internet without asking for approval. Exercise caution when using.",
            approval: AskForApproval::Never,
            active_permission_profile: ActivePermissionProfile::new(
                BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS,
            ),
            permission_profile: PermissionProfile::Disabled,
        },
    ]
}

/// Return the concrete profile for one of the built-in active profile ids.
pub fn builtin_permission_profile_for_active_permission_profile(
    active_permission_profile: &ActivePermissionProfile,
) -> Option<PermissionProfile> {
    if active_permission_profile.extends.is_some() {
        return None;
    }

    match active_permission_profile.id.as_str() {
        BUILT_IN_PERMISSION_PROFILE_READ_ONLY => Some(PermissionProfile::read_only()),
        BUILT_IN_PERMISSION_PROFILE_WORKSPACE => Some(PermissionProfile::workspace_write()),
        BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS => Some(PermissionProfile::Disabled),
        _ => None,
    }
}
