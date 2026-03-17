use super::*;
use codex_protocol::protocol::GranularApprovalConfig;
use pretty_assertions::assert_eq;
use std::collections::HashMap;

#[test]
fn wants_no_sandbox_approval_granular_respects_sandbox_flag() {
    let runtime = ApplyPatchRuntime::new();
    assert!(runtime.wants_no_sandbox_approval(AskForApproval::OnRequest));
    assert!(
        !runtime.wants_no_sandbox_approval(AskForApproval::Granular(GranularApprovalConfig {
            sandbox_approval: false,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: true,
        }))
    );
    assert!(
        runtime.wants_no_sandbox_approval(AskForApproval::Granular(GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: true,
        }))
    );
}

#[test]
fn guardian_review_request_includes_patch_context() {
    let path = std::env::temp_dir().join("guardian-apply-patch-test.txt");
    let action = ApplyPatchAction::new_add_for_test(&path, "hello".to_string());
    let expected_cwd = action.cwd.clone();
    let expected_patch = action.patch.clone();
    let request = ApplyPatchRequest {
        action,
        file_paths: vec![
            AbsolutePathBuf::from_absolute_path(&path).expect("temp path should be absolute"),
        ],
        changes: HashMap::from([(
            path,
            FileChange::Add {
                content: "hello".to_string(),
            },
        )]),
        exec_approval_requirement: ExecApprovalRequirement::NeedsApproval {
            bypass_sandbox: false,
            reason: None,
            proposed_execpolicy_amendment: None,
        },
        sandbox_permissions: SandboxPermissions::UseDefault,
        additional_permissions: None,
        permissions_preapproved: false,
        timeout_ms: None,
        codex_exe: None,
    };

    let guardian_request = ApplyPatchRuntime::build_guardian_review_request(&request, "call-1");

    assert_eq!(
        guardian_request,
        GuardianApprovalRequest::ApplyPatch {
            id: "call-1".to_string(),
            cwd: expected_cwd,
            files: request.file_paths,
            change_count: 1usize,
            patch: expected_patch,
        }
    );
}
