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
            approval: AskForApproval::UnlessTrusted,
            sandbox: SandboxPolicy::ReadOnly,
        },
        PermissionPreset {
            id: "trusted-directory",
            label: "Workspace Write (ask when needed)",
            description: "Workspace is writable; model decides when to ask. Network is blocked",
            approval: AskForApproval::OnRequest,
            sandbox: SandboxPolicy::new_workspace_write_policy(),
        },
        PermissionPreset {
            id: "full-auto",
            label: "Workspace Write (runs everything in sandbox; ask only on failure)",
            description: "Run everything in sandbox; ask only on failure. Network is blocked",
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
