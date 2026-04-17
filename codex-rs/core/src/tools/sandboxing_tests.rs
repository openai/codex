use super::*;
use crate::sandboxing::SandboxPermissions;
use codex_hooks::PermissionSuggestion;
use codex_hooks::PermissionSuggestionDestination;
use codex_hooks::PermissionSuggestionRule;
use codex_hooks::PermissionSuggestionType;
use codex_protocol::approvals::ExecPolicyAmendment;
use codex_protocol::approvals::NetworkApprovalContext;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::NetworkAccess;
use codex_utils_absolute_path::AbsolutePathBuf;
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
fn command_approval_execpolicy_amendment_maps_to_user_settings_suggestion() {
    let suggestions = approval_permission_suggestions(
        /*network_approval_context*/ None,
        Some(&ExecPolicyAmendment::new(vec![
            "rm".to_string(),
            "-rf".to_string(),
            "node_modules".to_string(),
        ])),
        /*additional_permissions*/ None,
        &[PermissionSuggestionDestination::UserSettings],
    );

    assert_eq!(
        suggestions,
        vec![PermissionSuggestion {
            suggestion_type: PermissionSuggestionType::AddRules,
            rules: vec![PermissionSuggestionRule::PrefixRule {
                command: vec![
                    "rm".to_string(),
                    "-rf".to_string(),
                    "node_modules".to_string(),
                ],
            }],
            behavior: PermissionSuggestionBehavior::Allow,
            destination: PermissionSuggestionDestination::UserSettings,
        }]
    );
}

#[test]
fn command_approval_with_additional_permissions_has_no_persistent_suggestions() {
    let suggestions = approval_permission_suggestions(
        /*network_approval_context*/ None,
        Some(&ExecPolicyAmendment::new(vec![
            "cat".to_string(),
            "/tmp/secret".to_string(),
        ])),
        Some(&PermissionProfile {
            network: None,
            file_system: Some(FileSystemPermissions {
                read: Some(vec![
                    AbsolutePathBuf::from_absolute_path("/tmp/secret")
                        .expect("/tmp/secret should be an absolute path"),
                ]),
                write: None,
            }),
        }),
        &[PermissionSuggestionDestination::UserSettings],
    );

    assert_eq!(suggestions, Vec::<PermissionSuggestion>::new());
}

#[test]
fn network_approval_with_execpolicy_amendment_has_no_persistent_suggestions() {
    let suggestions = approval_permission_suggestions(
        Some(&NetworkApprovalContext {
            host: "example.com".to_string(),
            protocol: NetworkApprovalProtocol::Https,
        }),
        Some(&ExecPolicyAmendment::new(vec![
            "curl".to_string(),
            "https://example.com".to_string(),
        ])),
        /*additional_permissions*/ None,
        &[PermissionSuggestionDestination::UserSettings],
    );

    assert_eq!(suggestions, Vec::<PermissionSuggestion>::new());
}
