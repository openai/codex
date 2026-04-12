//! Built-in permission presets shared by clients that render permission pickers.
//!
//! This crate keeps preset definitions independent from any single UI. Callers
//! decide which presets are available in their environment, then apply the
//! returned policies only after the relevant confirmation surface resolves.

use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;

/// A simple preset pairing an approval policy with a sandbox policy.
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
    /// Sandbox policy to apply.
    pub sandbox: SandboxPolicy,
}

/// A built-in permission mode the user can select.
#[derive(Debug, Clone)]
pub struct PermissionPreset {
    /// Stable identifier for the preset.
    pub id: &'static str,
    /// Display label shown in UIs.
    pub label: &'static str,
    /// Short human description shown next to the label in UIs.
    pub description: &'static str,
    /// Approval policy to apply.
    pub approval: AskForApproval,
    /// Sandbox policy to apply.
    pub sandbox: SandboxPolicy,
    /// Approval reviewer to apply.
    pub approvals_reviewer: ApprovalsReviewer,
}

/// Built-in list of approval presets that pair approval and sandbox policy.
///
/// Keep this UI-agnostic so it can be reused by both TUI and MCP server.
pub fn builtin_approval_presets() -> Vec<ApprovalPreset> {
    vec![
        ApprovalPreset {
            id: "read-only",
            label: "Read Only",
            description: "Codex can read files in the current workspace. Approval is required to edit files or access the internet.",
            approval: AskForApproval::OnRequest,
            sandbox: SandboxPolicy::new_read_only_policy(),
        },
        ApprovalPreset {
            id: "auto",
            label: "Default",
            description: "Codex can read and edit files in the current workspace, and run commands. Approval is required to access the internet or edit other files. (Identical to Agent mode)",
            approval: AskForApproval::OnRequest,
            sandbox: SandboxPolicy::new_workspace_write_policy(),
        },
        ApprovalPreset {
            id: "full-access",
            label: "Full Access",
            description: "Codex can edit files outside this workspace and access the internet without asking for approval. Exercise caution when using.",
            approval: AskForApproval::Never,
            sandbox: SandboxPolicy::DangerFullAccess,
        },
    ]
}

/// Built-in permission presets exposed by permission-mode selection surfaces.
///
/// `include_read_only` and `include_guardian` let each client match the set of
/// presets it already exposes without duplicating the preset definitions.
pub fn builtin_permission_presets(
    include_read_only: bool,
    include_guardian: bool,
) -> Vec<PermissionPreset> {
    let mut presets = Vec::new();
    for preset in builtin_approval_presets() {
        if !include_read_only && preset.id == "read-only" {
            continue;
        }

        presets.push(PermissionPreset {
            id: preset.id,
            label: preset.label,
            description: preset.description,
            approval: preset.approval,
            sandbox: preset.sandbox.clone(),
            approvals_reviewer: ApprovalsReviewer::User,
        });

        if include_guardian && preset.id == "auto" {
            presets.push(PermissionPreset {
                id: "guardian-approvals",
                label: "Guardian Approvals",
                description: "Same workspace-write permissions as Default, but eligible `on-request` approvals are routed through the guardian reviewer subagent.",
                approval: preset.approval,
                sandbox: preset.sandbox,
                approvals_reviewer: ApprovalsReviewer::GuardianSubagent,
            });
        }
    }
    presets
}

/// Finds a built-in permission preset by stable identifier.
///
/// The availability flags must match the client surface that will display the
/// preset. Passing broader flags than the UI uses can let a model request a
/// preset that the user never had a chance to select.
pub fn find_builtin_permission_preset(
    id: &str,
    include_read_only: bool,
    include_guardian: bool,
) -> Option<PermissionPreset> {
    builtin_permission_presets(include_read_only, include_guardian)
        .into_iter()
        .find(|preset| preset.id == id)
}
