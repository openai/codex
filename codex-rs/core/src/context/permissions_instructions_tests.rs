use super::*;
use codex_execpolicy::Decision;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

#[test]
fn renders_sandbox_mode_text() {
    assert_eq!(
        sandbox_text(SandboxMode::WorkspaceWrite, NetworkAccess::Restricted),
        "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `workspace-write`: The sandbox permits reading files, and editing files in `cwd` and `writable_roots`. Editing files in other directories requires approval. Network access is restricted."
    );

    assert_eq!(
        sandbox_text(SandboxMode::ReadOnly, NetworkAccess::Restricted),
        "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `read-only`: The sandbox only permits reading files. Network access is restricted."
    );

    assert_eq!(
        sandbox_text(SandboxMode::DangerFullAccess, NetworkAccess::Enabled),
        "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `danger-full-access`: No filesystem sandboxing - all commands are permitted. Network access is enabled."
    );
}

#[test]
fn builds_permissions_with_network_access_override() {
    let instructions = PermissionsInstructions::from_permissions_with_network(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Enabled,
        PermissionsPromptConfig {
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::User,
            exec_policy: &Policy::empty(),
            exec_permission_approvals_enabled: false,
            request_permissions_tool_enabled: false,
        },
        /*writable_roots*/ None,
    );

    let text = instructions.body();
    assert!(
        text.contains("Network access is enabled."),
        "expected network access to be enabled in message"
    );
    assert!(
        text.contains("How to request escalation"),
        "expected approval guidance to be included"
    );
}

#[test]
fn builds_permissions_from_policy() {
    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: true,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    };

    let instructions = PermissionsInstructions::from_policy(
        &policy,
        AskForApproval::UnlessTrusted,
        ApprovalsReviewer::User,
        &Policy::empty(),
        &PathBuf::from("/tmp"),
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );
    let text = instructions.body();
    assert!(text.contains("Network access is enabled."));
    assert!(text.contains("`approval_policy` is `unless-trusted`"));
}

#[test]
fn builds_permissions_from_profile() {
    let cwd = PathBuf::from("/tmp");
    let writable_root =
        AbsolutePathBuf::from_absolute_path(cwd.join("repo")).expect("absolute path");
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: writable_root.clone(),
            },
            access: FileSystemAccessMode::Write,
        }]),
        NetworkSandboxPolicy::Enabled,
    );

    let instructions = PermissionsInstructions::from_permission_profile(
        &permission_profile,
        AskForApproval::UnlessTrusted,
        ApprovalsReviewer::User,
        &Policy::empty(),
        &cwd,
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );
    let text = instructions.body();
    assert!(text.contains("`sandbox_mode` is `workspace-write`"));
    assert!(text.contains("Network access is enabled."));
    assert!(text.contains(writable_root.to_string_lossy().as_ref()));
}

#[test]
fn includes_request_rule_instructions_for_on_request() {
    let mut exec_policy = Policy::empty();
    exec_policy
        .add_prefix_rule(&["git".to_string(), "pull".to_string()], Decision::Allow)
        .expect("add rule");
    let instructions = PermissionsInstructions::from_permissions_with_network(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Enabled,
        PermissionsPromptConfig {
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::User,
            exec_policy: &exec_policy,
            exec_permission_approvals_enabled: false,
            request_permissions_tool_enabled: false,
        },
        /*writable_roots*/ None,
    );

    let text = instructions.body();
    assert!(text.contains("prefix_rule"));
    assert!(text.contains("Approved command prefixes"));
    assert!(text.contains(r#"["git", "pull"]"#));
}

#[test]
fn includes_request_permissions_tool_instructions_for_unless_trusted_when_enabled() {
    let instructions = PermissionsInstructions::from_permissions_with_network(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Enabled,
        PermissionsPromptConfig {
            approval_policy: AskForApproval::UnlessTrusted,
            approvals_reviewer: ApprovalsReviewer::User,
            exec_policy: &Policy::empty(),
            exec_permission_approvals_enabled: false,
            request_permissions_tool_enabled: true,
        },
        /*writable_roots*/ None,
    );

    let text = instructions.body();
    assert!(text.contains("`approval_policy` is `unless-trusted`"));
    assert!(text.contains("# request_permissions Tool"));
}

#[test]
fn includes_request_permissions_tool_instructions_for_on_failure_when_enabled() {
    let instructions = PermissionsInstructions::from_permissions_with_network(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Enabled,
        PermissionsPromptConfig {
            approval_policy: AskForApproval::OnFailure,
            approvals_reviewer: ApprovalsReviewer::User,
            exec_policy: &Policy::empty(),
            exec_permission_approvals_enabled: false,
            request_permissions_tool_enabled: true,
        },
        /*writable_roots*/ None,
    );

    let text = instructions.body();
    assert!(text.contains("`approval_policy` is `on-failure`"));
    assert!(text.contains("# request_permissions Tool"));
}

#[test]
fn includes_request_permission_rule_instructions_for_on_request_when_enabled() {
    let instructions = PermissionsInstructions::from_permissions_with_network(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Enabled,
        PermissionsPromptConfig {
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::User,
            exec_policy: &Policy::empty(),
            exec_permission_approvals_enabled: true,
            request_permissions_tool_enabled: false,
        },
        /*writable_roots*/ None,
    );

    let text = instructions.body();
    assert!(text.contains("with_additional_permissions"));
    assert!(text.contains("additional_permissions"));
}

#[test]
fn includes_request_permissions_tool_instructions_for_on_request_when_tool_is_enabled() {
    let instructions = PermissionsInstructions::from_permissions_with_network(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Enabled,
        PermissionsPromptConfig {
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::User,
            exec_policy: &Policy::empty(),
            exec_permission_approvals_enabled: false,
            request_permissions_tool_enabled: true,
        },
        /*writable_roots*/ None,
    );

    let text = instructions.body();
    assert!(text.contains("# request_permissions Tool"));
    assert!(text.contains("The built-in `request_permissions` tool is available in this session."));
}

#[test]
fn on_request_includes_tool_guidance_alongside_inline_permission_guidance_when_both_exist() {
    let instructions = PermissionsInstructions::from_permissions_with_network(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Enabled,
        PermissionsPromptConfig {
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::User,
            exec_policy: &Policy::empty(),
            exec_permission_approvals_enabled: true,
            request_permissions_tool_enabled: true,
        },
        /*writable_roots*/ None,
    );

    let text = instructions.body();
    assert!(text.contains("with_additional_permissions"));
    assert!(text.contains("# request_permissions Tool"));
}

#[test]
fn auto_review_approvals_append_auto_review_specific_guidance() {
    let text = approval_text(
        AskForApproval::OnRequest,
        ApprovalsReviewer::AutoReview,
        &Policy::empty(),
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );

    assert!(text.contains("`approvals_reviewer` is `auto_review`"));
    assert!(!text.contains("`approvals_reviewer` is `guardian_subagent`"));
    assert!(text.contains("materially safer alternative"));
}

#[test]
fn auto_review_approvals_omit_auto_review_specific_guidance_when_approval_is_never() {
    let text = approval_text(
        AskForApproval::Never,
        ApprovalsReviewer::AutoReview,
        &Policy::empty(),
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );

    assert!(!text.contains("`approvals_reviewer` is `auto_review`"));
    assert!(!text.contains("`approvals_reviewer` is `guardian_subagent`"));
}

fn granular_categories_section(title: &str, categories: &[&str]) -> String {
    format!("{title}\n{}", categories.join("\n"))
}

fn granular_prompt_expected(
    prompted_categories: &[&str],
    rejected_categories: &[&str],
    include_shell_permission_request_instructions: bool,
    include_request_permissions_tool_section: bool,
) -> String {
    let mut sections = vec![granular_prompt_intro_text().to_string()];
    if !prompted_categories.is_empty() {
        sections.push(granular_categories_section(
            "These approval categories may still prompt the user when needed:",
            prompted_categories,
        ));
    }
    if !rejected_categories.is_empty() {
        sections.push(granular_categories_section(
            "These approval categories are automatically rejected instead of prompting the user:",
            rejected_categories,
        ));
    }
    if include_shell_permission_request_instructions {
        sections.push(APPROVAL_POLICY_ON_REQUEST_RULE_REQUEST_PERMISSION.to_string());
    }
    if include_request_permissions_tool_section {
        sections.push(request_permissions_tool_prompt_section().to_string());
    }
    sections.join("\n\n")
}

#[test]
fn granular_policy_lists_prompted_and_rejected_categories_separately() {
    let text = approval_text(
        AskForApproval::Granular(GranularApprovalConfig {
            sandbox_approval: false,
            rules: true,
            skill_approval: false,
            request_permissions: true,
            mcp_elicitations: false,
        }),
        ApprovalsReviewer::User,
        &Policy::empty(),
        /*exec_permission_approvals_enabled*/ true,
        /*request_permissions_tool_enabled*/ false,
    );

    assert_eq!(
        text,
        [
            granular_prompt_intro_text().to_string(),
            granular_categories_section(
                "These approval categories may still prompt the user when needed:",
                &["- `rules`"],
            ),
            granular_categories_section(
                "These approval categories are automatically rejected instead of prompting the user:",
                &[
                    "- `sandbox_approval`",
                    "- `skill_approval`",
                    "- `mcp_elicitations`",
                ],
            ),
        ]
        .join("\n\n")
    );
}

#[test]
fn granular_policy_includes_command_permission_instructions_when_sandbox_approval_can_prompt() {
    let text = approval_text(
        AskForApproval::Granular(GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: true,
        }),
        ApprovalsReviewer::User,
        &Policy::empty(),
        /*exec_permission_approvals_enabled*/ true,
        /*request_permissions_tool_enabled*/ false,
    );

    assert_eq!(
        text,
        granular_prompt_expected(
            &[
                "- `sandbox_approval`",
                "- `rules`",
                "- `skill_approval`",
                "- `mcp_elicitations`",
            ],
            &[],
            /*include_shell_permission_request_instructions*/ true,
            /*include_request_permissions_tool_section*/ false,
        )
    );
}

#[test]
fn granular_policy_omits_shell_permission_instructions_when_inline_requests_are_disabled() {
    let text = approval_text(
        AskForApproval::Granular(GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: true,
        }),
        ApprovalsReviewer::User,
        &Policy::empty(),
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );

    assert_eq!(
        text,
        granular_prompt_expected(
            &[
                "- `sandbox_approval`",
                "- `rules`",
                "- `skill_approval`",
                "- `mcp_elicitations`",
            ],
            &[],
            /*include_shell_permission_request_instructions*/ false,
            /*include_request_permissions_tool_section*/ false,
        )
    );
}

#[test]
fn granular_policy_includes_request_permissions_tool_only_when_that_prompt_can_still_fire() {
    let allowed = approval_text(
        AskForApproval::Granular(GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: true,
        }),
        ApprovalsReviewer::User,
        &Policy::empty(),
        /*exec_permission_approvals_enabled*/ true,
        /*request_permissions_tool_enabled*/ true,
    );
    assert!(allowed.contains("# request_permissions Tool"));

    let rejected = approval_text(
        AskForApproval::Granular(GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: false,
            mcp_elicitations: true,
        }),
        ApprovalsReviewer::User,
        &Policy::empty(),
        /*exec_permission_approvals_enabled*/ true,
        /*request_permissions_tool_enabled*/ true,
    );
    assert!(!rejected.contains("# request_permissions Tool"));
}

#[test]
fn granular_policy_lists_request_permissions_category_without_tool_section_when_tool_unavailable() {
    let text = approval_text(
        AskForApproval::Granular(GranularApprovalConfig {
            sandbox_approval: false,
            rules: false,
            skill_approval: false,
            request_permissions: true,
            mcp_elicitations: false,
        }),
        ApprovalsReviewer::User,
        &Policy::empty(),
        /*exec_permission_approvals_enabled*/ true,
        /*request_permissions_tool_enabled*/ false,
    );

    assert!(!text.contains("- `request_permissions`"));
    assert!(!text.contains("# request_permissions Tool"));
}
