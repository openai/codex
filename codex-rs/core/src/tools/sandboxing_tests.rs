use super::*;
use crate::sandboxing::SandboxPermissions;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::NetworkAccess;
use pretty_assertions::assert_eq;

#[test]
fn external_sandbox_skips_exec_approval_on_request() {
    let sandbox_policy = SandboxPolicy::ExternalSandbox {
        network_access: NetworkAccess::Restricted,
    };
    assert_eq!(
        default_exec_approval_requirement(
            AskForApproval::OnRequest,
            &FileSystemSandboxPolicy::from(&sandbox_policy),
        ),
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        }
    );
}

#[test]
fn restricted_sandbox_requires_exec_approval_on_request() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    assert_eq!(
        default_exec_approval_requirement(
            AskForApproval::OnRequest,
            &FileSystemSandboxPolicy::from(&sandbox_policy)
        ),
        ExecApprovalRequirement::NeedsApproval {
            bypass_sandbox: false,
            reason: None,
            proposed_execpolicy_amendment: None,
        }
    );
}

#[test]
fn default_exec_approval_requirement_rejects_sandbox_prompt_when_granular_disables_it() {
    let policy = AskForApproval::Granular(GranularApprovalConfig {
        sandbox_approval: false,
        rules: true,
        skill_approval: true,
        request_permissions: true,
        mcp_elicitations: true,
    });

    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let requirement =
        default_exec_approval_requirement(policy, &FileSystemSandboxPolicy::from(&sandbox_policy));

    assert_eq!(
        requirement,
        ExecApprovalRequirement::Forbidden {
            reason: "approval policy disallowed sandbox approval prompt".to_string(),
        }
    );
}

#[test]
fn default_exec_approval_requirement_keeps_prompt_when_granular_allows_sandbox_approval() {
    let policy = AskForApproval::Granular(GranularApprovalConfig {
        sandbox_approval: true,
        rules: false,
        skill_approval: true,
        request_permissions: true,
        mcp_elicitations: false,
    });

    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let requirement =
        default_exec_approval_requirement(policy, &FileSystemSandboxPolicy::from(&sandbox_policy));

    assert_eq!(
        requirement,
        ExecApprovalRequirement::NeedsApproval {
            bypass_sandbox: false,
            reason: None,
            proposed_execpolicy_amendment: None,
        }
    );
}

#[test]
fn additional_permissions_allow_bypass_sandbox_first_attempt_when_execpolicy_skips() {
    assert_eq!(
        sandbox_override_for_first_attempt(
            SandboxPermissions::WithAdditionalPermissions,
            &ExecApprovalRequirement::Skip {
                bypass_sandbox: true,
                proposed_execpolicy_amendment: None,
            },
        ),
        SandboxOverride::BypassSandboxFirstAttempt
    );
}

#[test]
fn guardian_bypasses_sandbox_for_explicit_escalation_on_first_attempt() {
    assert_eq!(
        sandbox_override_for_first_attempt(
            SandboxPermissions::RequireEscalated,
            &ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
        ),
        SandboxOverride::BypassSandboxFirstAttempt
    );
}

#[test]
fn force_manual_approval_converts_skip_to_needs_approval() {
    let requirement = ExecApprovalRequirement::Skip {
        bypass_sandbox: false,
        proposed_execpolicy_amendment: None,
    };

    assert_eq!(
        requirement.force_manual_approval(),
        ExecApprovalRequirement::NeedsApproval {
            bypass_sandbox: false,
            reason: None,
            proposed_execpolicy_amendment: None,
        }
    );
}

#[test]
fn force_manual_approval_preserves_bypass_sandbox() {
    let requirement = ExecApprovalRequirement::Skip {
        bypass_sandbox: true,
        proposed_execpolicy_amendment: None,
    };

    assert_eq!(
        requirement.force_manual_approval(),
        ExecApprovalRequirement::NeedsApproval {
            bypass_sandbox: true,
            reason: None,
            proposed_execpolicy_amendment: None,
        }
    );
}

#[test]
fn needs_approval_can_bypass_sandbox_on_first_attempt() {
    assert_eq!(
        sandbox_override_for_first_attempt(
            SandboxPermissions::UseDefault,
            &ExecApprovalRequirement::NeedsApproval {
                bypass_sandbox: true,
                reason: None,
                proposed_execpolicy_amendment: None,
            },
        ),
        SandboxOverride::BypassSandboxFirstAttempt
    );
}

#[test]
fn force_manual_approval_preserves_forbidden() {
    let requirement = ExecApprovalRequirement::Forbidden {
        reason: "denied".to_string(),
    };

    assert_eq!(
        requirement.force_manual_approval(),
        ExecApprovalRequirement::Forbidden {
            reason: "denied".to_string(),
        }
    );
}
