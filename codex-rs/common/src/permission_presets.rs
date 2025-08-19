use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;

/// A simple preset pairing an approval policy with a sandbox policy.
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
}

/// Built-in list of permission presets that pair approval and sandbox policy.
///
/// Keep this UI-agnostic so it can be reused by both TUI and MCP server.
pub fn builtin_permission_presets() -> Vec<PermissionPreset> {
    vec![
        PermissionPreset {
            id: "default-untrusted-read-only",
            label: "Read Only",
            description: "Default for untrusted directories: auto-run trusted read-only commands; ask for others. No writes or network",
            approval: AskForApproval::OnRequest,
            sandbox: SandboxPolicy::ReadOnly,
        },
        PermissionPreset {
            id: "trusted-directory",
            label: "Guarded Write",
            description: "Run everything in sandbox; model decides when to ask for permission. Network is blocked by default",
            approval: AskForApproval::OnRequest,
            sandbox: SandboxPolicy::new_workspace_write_policy(),
        },
        PermissionPreset {
            id: "full-auto",
            label: "Auto Write",
            description: "Doesn't ask for permission except for network access. Network is blocked by default",
            approval: AskForApproval::OnFailure,
            sandbox: SandboxPolicy::new_workspace_write_policy(),
        },
        PermissionPreset {
            id: "yolo-dangerous",
            label: "YOLO",
            description: "No approvals; full disk and network access. Extremely risky",
            approval: AskForApproval::Never,
            sandbox: SandboxPolicy::DangerFullAccess,
        },
    ]
}
