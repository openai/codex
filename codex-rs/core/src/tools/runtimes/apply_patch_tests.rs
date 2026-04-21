use super::*;
use crate::tools::sandboxing::SandboxAttempt;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxType;
use core_test_support::PathBufExt;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::Arc;

fn local_environment() -> Arc<Environment> {
    Arc::new(Environment::create(/*exec_server_url*/ None).expect("local environment"))
}

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
        environment_id: None,
        environment: local_environment(),
        cwd: expected_cwd.clone(),
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
        environment_id: None,
        environment: local_environment(),
        cwd: path.clone(),
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
    let mut file_system_policy =
        FileSystemSandboxPolicy::from_legacy_sandbox_policy(&sandbox_policy, path.as_path());
    file_system_policy.entries.push(FileSystemSandboxEntry {
        path: FileSystemPath::Path { path: path.clone() },
        access: FileSystemAccessMode::None,
    });
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

    assert_eq!(sandbox.sandbox_policy, sandbox_policy);
    assert_eq!(sandbox.sandbox_policy_cwd, Some(path.clone()));
    assert_eq!(
        sandbox.file_system_sandbox_policy,
        Some(file_system_policy.clone())
    );
    assert_eq!(sandbox.additional_permissions, Some(additional_permissions));
    assert_eq!(
        sandbox.windows_sandbox_level,
        WindowsSandboxLevel::RestrictedToken
    );
    assert_eq!(sandbox.windows_sandbox_private_desktop, true);
    assert_eq!(sandbox.use_legacy_landlock, true);
}

#[test]
fn file_system_sandbox_context_omits_legacy_equivalent_policy() {
    let path = std::env::temp_dir()
        .join("apply-patch-runtime-legacy-equivalent.txt")
        .abs();
    let req = ApplyPatchRequest {
        environment_id: None,
        environment: local_environment(),
        cwd: path.clone(),
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
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let file_system_policy =
        FileSystemSandboxPolicy::from_legacy_sandbox_policy(&sandbox_policy, path.as_path());
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

    assert_eq!(sandbox.sandbox_policy_cwd, Some(path));
    assert_eq!(sandbox.file_system_sandbox_policy, None);
}

#[test]
fn no_sandbox_attempt_has_no_file_system_context() {
    let path = std::env::temp_dir()
        .join("apply-patch-runtime-none.txt")
        .abs();
    let req = ApplyPatchRequest {
        environment_id: None,
        environment: local_environment(),
        cwd: path.clone(),
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
