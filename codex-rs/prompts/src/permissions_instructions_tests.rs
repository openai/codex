use super::*;
use codex_execpolicy::Decision;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

#[test]
fn renders_sandbox_mode_text() {
    assert_eq!(
        sandbox_text(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Restricted,
            /*permission_messages*/ None,
        ),
        "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `workspace-write`: The sandbox permits reading files, and editing files in `cwd` and `writable_roots`. Editing files in other directories requires approval. Network access is restricted."
    );

    assert_eq!(
        sandbox_text(
            SandboxMode::ReadOnly,
            NetworkAccess::Restricted,
            /*permission_messages*/ None,
        ),
        "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `read-only`: The sandbox only permits reading files. Network access is restricted."
    );

    assert_eq!(
        sandbox_text(
            SandboxMode::DangerFullAccess,
            NetworkAccess::Enabled,
            /*permission_messages*/ None,
        ),
        "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `danger-full-access`: No filesystem sandboxing - all commands are permitted. Network access is enabled."
    );
}

#[test]
fn catalog_permission_messages_select_sandbox_mode_and_render_network_access() {
    let messages = PermissionMessages {
        danger_full_access: Some("catalog danger".to_string()),
        workspace_write: Some("catalog workspace {{ network_access }}".to_string()),
        read_only: Some("catalog read only {{ network_access }}".to_string()),
    };

    for (mode, expected) in [
        (SandboxMode::DangerFullAccess, "catalog danger"),
        (SandboxMode::WorkspaceWrite, "catalog workspace enabled"),
        (SandboxMode::ReadOnly, "catalog read only enabled"),
    ] {
        assert_eq!(
            sandbox_text(mode, NetworkAccess::Enabled, Some(&messages)),
            expected
        );
    }
}

#[test]
fn missing_catalog_permission_message_uses_legacy_sandbox_text() {
    let legacy = sandbox_text(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Restricted,
        /*permission_messages*/ None,
    );
    let messages = PermissionMessages {
        danger_full_access: None,
        workspace_write: None,
        read_only: Some("unused".to_string()),
    };

    assert_eq!(
        sandbox_text(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Restricted,
            Some(&messages),
        ),
        legacy
    );
}

#[test]
fn invalid_catalog_permission_message_is_preserved_verbatim() {
    for workspace_write in ["{{ unterminated", "{{ unsupported }}"] {
        let messages = PermissionMessages {
            danger_full_access: None,
            workspace_write: Some(workspace_write.to_string()),
            read_only: None,
        };
        assert_eq!(
            sandbox_text(
                SandboxMode::WorkspaceWrite,
                NetworkAccess::Restricted,
                Some(&messages),
            ),
            workspace_write
        );
    }
}

#[test]
fn catalog_permission_message_renders_network_access_and_preserves_other_placeholders() {
    let source = "network={{ network_access }} compact={{network_access}} other={{ other }}";
    let messages = PermissionMessages {
        danger_full_access: None,
        workspace_write: Some(source.to_string()),
        read_only: None,
    };

    assert_eq!(
        sandbox_text(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Restricted,
            Some(&messages),
        ),
        "network=restricted compact={{network_access}} other={{ other }}"
    );
}

#[test]
fn empty_catalog_permission_message_preserves_non_sandbox_sections() {
    let messages = PermissionMessages {
        danger_full_access: None,
        workspace_write: Some(String::new()),
        read_only: None,
    };
    let writable_root =
        AbsolutePathBuf::from_absolute_path(test_path_buf("/tmp/repo")).expect("absolute path");
    let instructions = PermissionsInstructions::from_permissions_with_network(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Restricted,
        PermissionsPromptConfig {
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            approval_messages: None,
            permission_messages: Some(&messages),
            exec_policy: &Policy::empty(),
            exec_permission_approvals_enabled: false,
            request_permissions_tool_enabled: false,
        },
        Some(vec![WritableRoot {
            root: writable_root.clone(),
            read_only_subpaths: Vec::new(),
            protected_metadata_names: Vec::new(),
        }]),
    );
    let text = instructions.body();

    assert!(!text.contains("Filesystem sandboxing defines"));
    assert!(text.contains("Approval policy is currently never"));
    assert!(text.contains(writable_root.to_string_lossy().as_ref()));
}

#[test]
fn builds_permissions_with_network_access_override() {
    let instructions = PermissionsInstructions::from_permissions_with_network(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Enabled,
        PermissionsPromptConfig {
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::User,
            approval_messages: None,
            permission_messages: None,
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
            missing_path_behavior: None,
        }]),
        NetworkSandboxPolicy::Enabled,
    );

    let instructions = PermissionsInstructions::from_permission_profile(
        &permission_profile,
        AskForApproval::UnlessTrusted,
        ApprovalPromptContext::new(
            ApprovalsReviewer::User,
            /*messages*/ None,
            /*permission_messages*/ None,
        ),
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
fn builds_permissions_from_profile_with_denied_reads() {
    let cwd = test_path_buf("/tmp");
    let denied_root =
        AbsolutePathBuf::from_absolute_path(cwd.join("blocked")).expect("absolute path");
    let denied_glob = cwd.join("blocked").join("**");
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: codex_protocol::permissions::FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: denied_root.clone(),
                },
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::GlobPattern {
                    pattern: denied_glob.to_string_lossy().into_owned(),
                },
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
        ]),
        NetworkSandboxPolicy::Restricted,
    );

    let instructions = PermissionsInstructions::from_permission_profile(
        &permission_profile,
        AskForApproval::OnRequest,
        ApprovalPromptContext::new(
            ApprovalsReviewer::AutoReview,
            /*messages*/ None,
            /*permission_messages*/ None,
        ),
        &Policy::empty(),
        &cwd,
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );
    let text = instructions.body();
    assert!(text.contains("## Denied filesystem reads"));
    assert!(text.contains("Do not request escalation or additional permissions"));
    assert!(text.contains(denied_root.to_string_lossy().as_ref()));
    assert!(text.contains(&format!("glob `{}`", denied_glob.to_string_lossy())));
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
            approval_messages: None,
            permission_messages: None,
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
            approval_messages: None,
            permission_messages: None,
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
fn includes_request_permission_rule_instructions_for_on_request_when_enabled() {
    let instructions = PermissionsInstructions::from_permissions_with_network(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Enabled,
        PermissionsPromptConfig {
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::User,
            approval_messages: None,
            permission_messages: None,
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
            approval_messages: None,
            permission_messages: None,
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
            approval_messages: None,
            permission_messages: None,
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
fn catalog_approval_messages_select_reviewer_variant() {
    let messages = ApprovalMessages {
        on_request: Some("user catalog approvals".to_string()),
        on_request_auto_review: Some("auto-review catalog approvals".to_string()),
        never: Some("never catalog approvals".to_string()),
        unless_trusted: Some("unless-trusted catalog approvals".to_string()),
    };

    for (approval_policy, reviewer, expected) in [
        (
            AskForApproval::OnRequest,
            ApprovalsReviewer::User,
            "user catalog approvals",
        ),
        (
            AskForApproval::OnRequest,
            ApprovalsReviewer::AutoReview,
            "auto-review catalog approvals",
        ),
        (
            AskForApproval::Never,
            ApprovalsReviewer::AutoReview,
            "never catalog approvals",
        ),
        (
            AskForApproval::UnlessTrusted,
            ApprovalsReviewer::AutoReview,
            "unless-trusted catalog approvals",
        ),
    ] {
        assert_eq!(
            approval_text(
                approval_policy,
                reviewer,
                Some(&messages),
                &Policy::empty(),
                /*exec_permission_approvals_enabled*/ true,
                /*request_permissions_tool_enabled*/ true,
            ),
            expected
        );
    }
}

#[test]
fn empty_catalog_approval_message_suppresses_legacy_approval_section() {
    let messages = ApprovalMessages {
        on_request: Some(String::new()),
        on_request_auto_review: None,
        never: None,
        unless_trusted: None,
    };
    let mut exec_policy = Policy::empty();
    exec_policy
        .add_prefix_rule(&["git".to_string(), "pull".to_string()], Decision::Allow)
        .expect("add rule");
    let writable_root =
        AbsolutePathBuf::from_absolute_path(test_path_buf("/tmp/repo")).expect("absolute path");

    let instructions = PermissionsInstructions::from_permissions_with_network(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Restricted,
        PermissionsPromptConfig {
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::User,
            approval_messages: Some(&messages),
            permission_messages: None,
            exec_policy: &exec_policy,
            exec_permission_approvals_enabled: true,
            request_permissions_tool_enabled: true,
        },
        Some(vec![WritableRoot {
            root: writable_root.clone(),
            read_only_subpaths: Vec::new(),
            protected_metadata_names: Vec::new(),
        }]),
    );
    let text = instructions.body();

    assert!(text.contains("`sandbox_mode` is `workspace-write`"));
    assert!(text.contains("Network access is restricted."));
    assert!(text.contains(writable_root.to_string_lossy().as_ref()));
    assert!(!text.contains("How to request escalation"));
    assert!(!text.contains("request_permissions Tool"));
    assert!(!text.contains("Approved command prefixes"));
}

#[test]
fn missing_catalog_key_uses_legacy_approval_text() {
    let messages = ApprovalMessages {
        on_request: None,
        on_request_auto_review: Some("unused catalog approvals".to_string()),
        never: None,
        unless_trusted: None,
    };

    let on_request = approval_text(
        AskForApproval::OnRequest,
        ApprovalsReviewer::User,
        Some(&messages),
        &Policy::empty(),
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );
    let never = approval_text(
        AskForApproval::Never,
        ApprovalsReviewer::User,
        Some(&messages),
        &Policy::empty(),
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );

    assert!(on_request.contains("How to request escalation"));
    assert_eq!(never, APPROVAL_POLICY_NEVER);
}

#[test]
fn empty_catalog_non_on_request_approval_messages_suppress_legacy_approval_text() {
    let messages = ApprovalMessages {
        on_request: None,
        on_request_auto_review: None,
        never: Some(String::new()),
        unless_trusted: Some(String::new()),
    };

    for approval_policy in [AskForApproval::Never, AskForApproval::UnlessTrusted] {
        assert_eq!(
            approval_text(
                approval_policy,
                ApprovalsReviewer::AutoReview,
                Some(&messages),
                &Policy::empty(),
                /*exec_permission_approvals_enabled*/ true,
                /*request_permissions_tool_enabled*/ true,
            ),
            ""
        );
    }
}

#[test]
fn auto_review_approvals_append_auto_review_specific_guidance() {
    let text = approval_text(
        AskForApproval::OnRequest,
        ApprovalsReviewer::AutoReview,
        /*approval_messages*/ None,
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
        /*approval_messages*/ None,
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
        /*approval_messages*/ None,
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
        /*approval_messages*/ None,
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
        /*approval_messages*/ None,
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
        /*approval_messages*/ None,
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
        /*approval_messages*/ None,
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
        /*approval_messages*/ None,
        &Policy::empty(),
        /*exec_permission_approvals_enabled*/ true,
        /*request_permissions_tool_enabled*/ false,
    );

    assert!(!text.contains("- `request_permissions`"));
    assert!(!text.contains("# request_permissions Tool"));
}
