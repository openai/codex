use super::*;
use crate::sandboxing::SandboxPermissions;
use crate::tools::hook_names::HookToolName;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxType;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;

#[test]
fn bash_permission_request_payload_omits_missing_description() {
    assert_eq!(
        PermissionRequestPayload::bash("echo hi".to_string(), /*description*/ None),
        PermissionRequestPayload {
            tool_name: HookToolName::bash(),
            tool_input: json!({ "command": "echo hi" }),
        }
    );
}

#[test]
fn bash_permission_request_payload_includes_description_when_present() {
    assert_eq!(
        PermissionRequestPayload::bash(
            "echo hi".to_string(),
            Some("network-access example.com".to_string()),
        ),
        PermissionRequestPayload {
            tool_name: HookToolName::bash(),
            tool_input: json!({
                "command": "echo hi",
                "description": "network-access example.com",
            }),
        }
    );
}

#[test]
fn external_sandbox_skips_exec_approval_on_request() {
    assert_eq!(
        default_exec_approval_requirement(
            AskForApproval::OnRequest,
            &FileSystemSandboxPolicy::external_sandbox(),
        ),
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        }
    );
}

#[test]
fn restricted_sandbox_requires_exec_approval_on_request() {
    assert_eq!(
        default_exec_approval_requirement(
            AskForApproval::OnRequest,
            &FileSystemSandboxPolicy::default()
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

    let requirement =
        default_exec_approval_requirement(policy, &FileSystemSandboxPolicy::default());

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

    let requirement =
        default_exec_approval_requirement(policy, &FileSystemSandboxPolicy::default());

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
            &FileSystemSandboxPolicy::default(),
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
            &FileSystemSandboxPolicy::default(),
        ),
        SandboxOverride::BypassSandboxFirstAttempt
    );
}

#[test]
fn deny_read_blocks_explicit_escalation_and_policy_bypass() {
    let file_system_policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::GlobPattern {
            pattern: "**/*.env".to_string(),
        },
        access: FileSystemAccessMode::Deny,
    }]);

    assert_eq!(
        sandbox_override_for_first_attempt(
            SandboxPermissions::RequireEscalated,
            &ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
            &file_system_policy,
        ),
        SandboxOverride::NoOverride,
        "explicit escalation would drop deny-read filesystem policy, so keep the first attempt sandboxed",
    );
    assert!(!unsandboxed_execution_allowed(&file_system_policy));
    assert_eq!(
        sandbox_permissions_preserving_denied_reads(
            SandboxPermissions::RequireEscalated,
            &file_system_policy,
        ),
        SandboxPermissions::UseDefault,
    );
    assert_eq!(
        sandbox_permissions_preserving_denied_reads(
            SandboxPermissions::WithAdditionalPermissions,
            &file_system_policy,
        ),
        SandboxPermissions::WithAdditionalPermissions,
    );
    assert_eq!(
        sandbox_permissions_preserving_denied_reads(
            SandboxPermissions::RequireEscalated,
            &FileSystemSandboxPolicy::default(),
        ),
        SandboxPermissions::RequireEscalated,
    );
    assert_eq!(
        sandbox_override_for_first_attempt(
            SandboxPermissions::WithAdditionalPermissions,
            &ExecApprovalRequirement::Skip {
                bypass_sandbox: true,
                proposed_execpolicy_amendment: None,
            },
            &file_system_policy,
        ),
        SandboxOverride::NoOverride,
        "exec-policy allow rules would drop deny-read filesystem policy, so keep the first attempt sandboxed",
    );
}

#[test]
fn exec_server_env_keeps_command_native() {
    let cwd: AbsolutePathBuf = std::env::current_dir()
        .expect("current dir")
        .try_into()
        .expect("absolute cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let permissions = codex_protocol::models::PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::default(),
        NetworkSandboxPolicy::Restricted,
    );
    let manager = SandboxManager::new();
    let attempt = SandboxAttempt {
        sandbox: SandboxType::MacosSeatbelt,
        permissions: &permissions,
        enforce_managed_network: false,
        manager: &manager,
        sandbox_cwd: &cwd_uri,
        workspace_roots: std::slice::from_ref(&cwd),
        codex_linux_sandbox_exe: None,
        use_legacy_landlock: false,
        windows_sandbox_level: codex_protocol::config_types::WindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        network_denial_cancellation_token: None,
    };
    let command = SandboxCommand {
        program: "/bin/bash".into(),
        args: vec!["-lc".to_string(), "pwd".to_string()],
        cwd: cwd_uri.clone(),
        env: HashMap::new(),
        additional_permissions: None,
    };
    let options = crate::sandboxing::ExecOptions {
        expiration: crate::exec::ExecExpiration::DefaultTimeout,
        capture_policy: crate::exec::ExecCapturePolicy::ShellTool,
    };

    let request = attempt
        .env_for_exec_server(command, options, /*network*/ None, Some("remote"))
        .expect("prepare remote exec request");

    assert_eq!(
        request.command,
        vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "pwd".to_string()
        ]
    );
    assert_eq!(request.arg0, None);
    assert_eq!(request.sandbox, SandboxType::None);
}
