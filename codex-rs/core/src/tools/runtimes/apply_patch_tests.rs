use super::*;
use crate::tools::sandboxing::SandboxAttempt;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxType;
use codex_sandboxing::policy_transforms::merge_permission_profiles;
use core_test_support::PathBufExt;
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
    let path = std::env::temp_dir()
        .join("guardian-apply-patch-test.txt")
        .abs();
    let action = ApplyPatchAction::new_add_for_test(&path, "hello".to_string());
    let expected_cwd = action.cwd.clone();
    let expected_patch = action.patch.clone();
    let request = ApplyPatchRequest {
        action,
        file_paths: vec![path.clone()],
        changes: HashMap::from([(
            path.to_path_buf(),
            FileChange::Add {
                content: "hello".to_string(),
            },
        )]),
        exec_approval_requirement: ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        },
        additional_permissions: None,
        permissions_preapproved: false,
    };

    let guardian_request = ApplyPatchRuntime::build_guardian_review_request(&request, "call-1");

    assert_eq!(
        guardian_request,
        GuardianApprovalRequest::ApplyPatch {
            id: "call-1".to_string(),
            cwd: expected_cwd,
            files: request.file_paths,
            patch: expected_patch,
        }
    );
}

#[test]
fn permission_request_payload_uses_apply_patch_hook_name_and_aliases() {
    let runtime = ApplyPatchRuntime::new();
    let path = std::env::temp_dir()
        .join("apply-patch-permission-request-payload.txt")
        .abs();
    let action = ApplyPatchAction::new_add_for_test(&path, "hello".to_string());
    let expected_patch = action.patch.clone();
    let req = ApplyPatchRequest {
        action,
        file_paths: vec![path],
        changes: HashMap::new(),
        exec_approval_requirement: ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        },
        additional_permissions: None,
        permissions_preapproved: false,
    };

    let payload = runtime
        .permission_request_payload(&req)
        .expect("permission request payload");

    assert_eq!(payload.tool_name.name(), "apply_patch");
    assert_eq!(
        payload.tool_name.matcher_aliases(),
        &["Write".to_string(), "Edit".to_string()]
    );
    assert_eq!(payload.command, expected_patch);
    assert_eq!(payload.description, None);
}

#[test]
fn file_system_sandbox_context_uses_active_attempt() {
    let path = std::env::temp_dir()
        .join("apply-patch-runtime-attempt.txt")
        .abs();
    let additional_permissions = PermissionProfile {
        network: None,
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![path.clone()]),
            Some(Vec::new()),
        )),
    };
    let req = ApplyPatchRequest {
        action: ApplyPatchAction::new_add_for_test(&path, "hello".to_string()),
        file_paths: vec![path.clone()],
        changes: HashMap::new(),
        exec_approval_requirement: ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        },
        additional_permissions: Some(additional_permissions.clone()),
        permissions_preapproved: false,
    };
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let file_system_policy = FileSystemSandboxPolicy::from(&sandbox_policy);
    let manager = SandboxManager::new();
    let attempt = SandboxAttempt {
        sandbox: SandboxType::MacosSeatbelt,
        policy: &sandbox_policy,
        file_system_policy: &file_system_policy,
        network_policy: NetworkSandboxPolicy::Restricted,
        enforce_managed_network: false,
        manager: &manager,
        sandbox_cwd: &path,
        codex_linux_sandbox_exe: None,
        use_legacy_landlock: true,
        windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
        windows_sandbox_private_desktop: true,
    };

    let sandbox = ApplyPatchRuntime::file_system_sandbox_context_for_attempt(&req, &attempt)
        .expect("sandbox context");

    let base_permissions = PermissionProfile::from_runtime_permissions(
        &file_system_policy,
        NetworkSandboxPolicy::Restricted,
    );
    let Some(expected_permissions) =
        merge_permission_profiles(Some(&base_permissions), Some(&additional_permissions))
    else {
        panic!("merged permissions should not be empty");
    };
    assert_eq!(sandbox.permissions, expected_permissions);
    assert_eq!(
        sandbox.windows_sandbox_level,
        WindowsSandboxLevel::RestrictedToken
    );
    assert_eq!(sandbox.windows_sandbox_private_desktop, true);
    assert_eq!(sandbox.use_legacy_landlock, true);
}

#[test]
fn no_sandbox_attempt_has_no_file_system_context() {
    let path = std::env::temp_dir()
        .join("apply-patch-runtime-none.txt")
        .abs();
    let req = ApplyPatchRequest {
        action: ApplyPatchAction::new_add_for_test(&path, "hello".to_string()),
        file_paths: vec![path.clone()],
        changes: HashMap::new(),
        exec_approval_requirement: ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        },
        additional_permissions: None,
        permissions_preapproved: false,
    };
    let sandbox_policy = SandboxPolicy::DangerFullAccess;
    let file_system_policy = FileSystemSandboxPolicy::from(&sandbox_policy);
    let manager = SandboxManager::new();
    let attempt = SandboxAttempt {
        sandbox: SandboxType::None,
        policy: &sandbox_policy,
        file_system_policy: &file_system_policy,
        network_policy: NetworkSandboxPolicy::Enabled,
        enforce_managed_network: false,
        manager: &manager,
        sandbox_cwd: &path,
        codex_linux_sandbox_exe: None,
        use_legacy_landlock: false,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
    };

    assert_eq!(
        ApplyPatchRuntime::file_system_sandbox_context_for_attempt(&req, &attempt),
        None
    );
}
