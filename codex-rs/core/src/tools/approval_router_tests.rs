use super::*;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::ReviewDecision;
use core_test_support::PathBufExt;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::test]
async fn guardian_route_bypasses_session_cache() {
    let (_session, mut turn) = crate::codex::make_session_and_context().await;
    let mut config = (*turn.config).clone();
    config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
    turn.config = Arc::new(config);

    let request = ApprovalRequest {
        intent: ApprovalIntent::Shell(ShellApprovalRequest {
            call_id: "call-1".to_string(),
            command: vec!["echo".to_string(), "hello".to_string()],
            cwd: turn.cwd.to_path_buf(),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("need output".to_string()),
            reason: Some("need output".to_string()),
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
        }),
        retry_reason: None,
        cache: Some(ApprovalCache::new("shell", vec!["cached-key"])),
    };

    assert!(!should_consult_session_cache(
        routes_approval_to_guardian(&turn),
        request.retry_reason.as_ref(),
        request.cache.as_ref(),
    ));
}

#[test]
fn shell_intent_translates_to_guardian_and_user_requests() {
    let shell = ShellApprovalRequest {
        call_id: "call-1".to_string(),
        command: vec!["git".to_string(), "status".to_string()],
        cwd: PathBuf::from("/repo"),
        sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
        additional_permissions: None,
        justification: Some("inspect repo".to_string()),
        reason: Some("inspect repo".to_string()),
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
    };

    assert_eq!(
        ApprovalIntent::Shell(shell.clone()).into_guardian_request(),
        GuardianApprovalRequest::Shell {
            id: "call-1".to_string(),
            command: vec!["git".to_string(), "status".to_string()],
            cwd: PathBuf::from("/repo"),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("inspect repo".to_string()),
        }
    );
    assert_eq!(
        ApprovalIntent::Shell(shell).into_user_request(),
        UserApprovalRequest::Command(CommandApprovalRequest {
            call_id: "call-1".to_string(),
            approval_id: None,
            command: vec!["git".to_string(), "status".to_string()],
            cwd: PathBuf::from("/repo"),
            reason: Some("inspect repo".to_string()),
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
            additional_permissions: None,
            available_decisions: None,
        })
    );
}

#[test]
fn apply_patch_intent_translates_to_guardian_and_user_requests() {
    let path = std::env::temp_dir()
        .join("approval-router-apply-patch.txt")
        .abs();
    let patch = ApplyPatchApprovalRequest {
        call_id: "call-1".to_string(),
        cwd: path.parent().expect("parent").to_path_buf(),
        files: vec![path.clone()],
        patch: "*** Begin Patch\n*** Add File: approval-router-apply-patch.txt\n+hello\n*** End Patch\n"
            .to_string(),
        changes: HashMap::from([(
            path.to_path_buf(),
            FileChange::Add {
                content: "hello".to_string(),
            },
        )]),
        reason: Some("apply patch".to_string()),
        grant_root: None,
    };

    assert_eq!(
        ApprovalIntent::ApplyPatch(patch.clone()).into_guardian_request(),
        GuardianApprovalRequest::ApplyPatch {
            id: "call-1".to_string(),
            cwd: patch.cwd.clone(),
            files: vec![path.clone()],
            patch: patch.patch.clone(),
        }
    );
    assert_eq!(
        ApprovalIntent::ApplyPatch(patch).into_user_request(),
        UserApprovalRequest::Patch(PatchApprovalRequest {
            call_id: "call-1".to_string(),
            changes: HashMap::from([(
                path.to_path_buf(),
                FileChange::Add {
                    content: "hello".to_string(),
                },
            )]),
            reason: Some("apply patch".to_string()),
            grant_root: None,
        })
    );
}

#[cfg(unix)]
#[test]
fn execve_intent_preserves_available_decisions_for_user_prompt() {
    let execve = ExecveApprovalRequest {
        call_id: "call-1".to_string(),
        approval_id: Some("approval-1".to_string()),
        source: codex_protocol::approvals::GuardianCommandSource::Shell,
        program: "/bin/ls".to_string(),
        argv: vec!["-l".to_string()],
        command: vec!["/bin/ls".to_string(), "-l".to_string()],
        cwd: PathBuf::from("/tmp"),
        additional_permissions: None,
        reason: None,
        available_decisions: vec![ReviewDecision::Approved, ReviewDecision::Abort],
    };

    assert_eq!(
        ApprovalIntent::Execve(execve).into_user_request(),
        UserApprovalRequest::Command(CommandApprovalRequest {
            call_id: "call-1".to_string(),
            approval_id: Some("approval-1".to_string()),
            command: vec!["/bin/ls".to_string(), "-l".to_string()],
            cwd: PathBuf::from("/tmp"),
            reason: None,
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
            additional_permissions: None,
            available_decisions: Some(vec![ReviewDecision::Approved, ReviewDecision::Abort,]),
        })
    );
}
