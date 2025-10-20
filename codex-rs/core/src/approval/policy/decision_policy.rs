use crate::approval::CommandDecision;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;

pub fn evaluate_decision_policy(
    approval_policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    with_escalated_permissions: bool,
    user_explicitly_approved: bool,
) -> CommandDecision {
    use crate::approval::platform_sandbox;
    use crate::exec::SandboxType;
    use codex_protocol::protocol::AskForApproval::*;
    use codex_protocol::protocol::SandboxPolicy::*;

    // 1) Interactive user approval always wins.
    if user_explicitly_approved {
        return CommandDecision::permit(SandboxType::None, true);
    }

    // 2) User wants to approve all commands not on the trusted list.
    if matches!(approval_policy, UnlessTrusted) {
        return CommandDecision::require_approval();
    }

    // 3) If the sandbox policy is "danger, full access", we don't sandbox.
    if matches!(sandbox_policy, DangerFullAccess) {
        return CommandDecision::permit(SandboxType::None, false);
    }

    // From here, sandbox policy is ReadOnly or WorkspaceWrite.
    // Helper to try a platform sandbox or fall back to a policy-specific outcome.
    let try_platform_sandbox = |fallback: CommandDecision| -> CommandDecision {
        match platform_sandbox() {
            Some(sandbox_type) => CommandDecision::permit(sandbox_type, false),
            None => fallback,
        }
    };

    match approval_policy {
        // Already handled above; unreachable here by construction.
        UnlessTrusted => unreachable!(),

        // Ask on request: allow sandboxed auto-approval unless escalation was requested.
        OnRequest => {
            if with_escalated_permissions {
                CommandDecision::require_approval()
            } else {
                // Prefer sandbox; if none available, ask.
                try_platform_sandbox(CommandDecision::require_approval())
            }
        }

        // Never auto-ask: prefer sandbox; if none, reject outright.
        Never => try_platform_sandbox(CommandDecision::deny(
            "auto-rejected because command is not on trusted list",
        )),

        // Ask only when sandboxing fails: prefer sandbox; if none, ask the user.
        OnFailure => try_platform_sandbox(CommandDecision::require_approval()),
    }
}
