//! Covers permission profile preparation and rendering from explicit facts and catalog overrides.

use super::*;
use crate::ResolvedModelMessages;
use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::RenderedFragment;
use codex_protocol::models::ContentItemMetadata;
use codex_protocol::openai_models::ApprovalMessages;
use codex_protocol::openai_models::PermissionMessages;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol_test_support::ContentItemTestExt;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;

const WORKSPACE_CONTEXT: PermissionsRenderContext<'static> = PermissionsRenderContext {
    sandbox_mode: SandboxMode::WorkspaceWrite,
    network_access: NetworkAccess::Enabled,
    approval_policy: AskForApproval::OnRequest,
    approved_command_prefixes: &[],
    writable_roots: &[],
    denied_read_paths: &[],
    denied_read_globs: &[],
    exec_permission_approvals_enabled: false,
    request_permissions_tool_enabled: false,
};

fn user_approval_context() -> ApprovalPromptContext<'static> {
    ApprovalPromptContext::new(ApprovalsReviewer::User, ResolvedModelMessages::bundled())
}

#[test]
fn builds_permissions_from_profile() {
    let cwd = test_path_buf("/workspace");
    let writable_root =
        AbsolutePathBuf::from_absolute_path(cwd.join("repo")).expect("absolute path");
    let denied_root =
        AbsolutePathBuf::from_absolute_path(cwd.join("blocked")).expect("absolute path");
    let denied_glob = cwd
        .join("blocked")
        .join("**")
        .to_string_lossy()
        .into_owned();
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: writable_root.clone().into(),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry::new(
                FileSystemPath::Path {
                    path: denied_root.clone().into(),
                },
                FileSystemAccessMode::Deny,
            ),
            FileSystemSandboxEntry::new(
                FileSystemPath::GlobPattern {
                    pattern: denied_glob.clone(),
                },
                FileSystemAccessMode::Deny,
            ),
        ]),
        NetworkSandboxPolicy::Enabled,
    );

    let instructions = PermissionsInstructions::from_permission_profile(
        &permission_profile,
        AskForApproval::UnlessTrusted,
        user_approval_context(),
        &Policy::empty(),
        &cwd,
        /*paths*/ None,
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );
    let text = instructions.body().expect_text();
    assert!(text.contains("`sandbox_mode` is `workspace-write`"));
    assert!(text.contains("Network access is enabled."));
    assert!(text.contains(writable_root.to_string_lossy().as_ref()));
    assert!(text.contains("Do not request escalation or additional permissions"));
    assert!(text.contains(&format!("path `{}`", denied_root.display())));
    assert!(text.contains(&format!("glob `{denied_glob}`")));
}

#[test]
fn builds_permissions_from_profile_with_executor_denied_reads() {
    let paths = FileSystemSandboxPolicyContext {
        cwd: &"file:///C:/workspace".parse().unwrap(),
        workspace_roots: &[],
        user_home_dir: None,
        temporary_directories: None,
    };
    let denied_root = paths.cwd.join("blocked").unwrap();
    let denied_glob = r"C:\workspace\blocked\**";
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Tmpdir,
                },
                FileSystemAccessMode::Deny,
            ),
            FileSystemSandboxEntry::new(
                FileSystemPath::GlobPattern {
                    pattern: "~/private/**".into(),
                },
                FileSystemAccessMode::Deny,
            ),
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: denied_root },
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::GlobPattern {
                    pattern: denied_glob.into(),
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
            ResolvedModelMessages::bundled(),
        ),
        &Policy::empty(),
        &test_path_buf("/unused-local-cwd"),
        Some(&paths),
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );
    let text = instructions.body().expect_text();
    assert!(text.contains("## Denied filesystem reads"));
    assert!(text.contains("Do not request escalation or additional permissions"));
    assert!(text.contains(r"path `C:\workspace\blocked`"));
    assert!(text.contains(&format!("glob `{denied_glob}`")));
}

#[test]
fn renders_sandbox_mode_text() {
    assert_eq!(
        sandbox_text(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Restricted,
            ResolvedPermissionMessages::new(/*messages*/ None),
        ),
        "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `workspace-write`: The sandbox permits reading files, and editing files in `cwd` and `writable_roots`. Editing files in other directories requires approval. Network access is restricted."
    );

    assert_eq!(
        sandbox_text(
            SandboxMode::ReadOnly,
            NetworkAccess::Restricted,
            ResolvedPermissionMessages::new(/*messages*/ None),
        ),
        "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `read-only`: The sandbox only permits reading files. Network access is restricted."
    );

    assert_eq!(
        sandbox_text(
            SandboxMode::DangerFullAccess,
            NetworkAccess::Enabled,
            ResolvedPermissionMessages::new(/*messages*/ None),
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
        approved_prefixes: None,
        ..PermissionMessages::default()
    };

    for (mode, expected) in [
        (SandboxMode::DangerFullAccess, "catalog danger"),
        (SandboxMode::WorkspaceWrite, "catalog workspace enabled"),
        (SandboxMode::ReadOnly, "catalog read only enabled"),
    ] {
        assert_eq!(
            sandbox_text(
                mode,
                NetworkAccess::Enabled,
                ResolvedPermissionMessages::new(Some(&messages))
            ),
            expected
        );
    }
}

#[test]
fn catalog_permission_text_equal_to_bundled_template_preserves_literal_whitespace() {
    let messages = PermissionMessages {
        danger_full_access: None,
        workspace_write: Some(WORKSPACE_WRITE_TEMPLATE.to_string()),
        read_only: None,
        approved_prefixes: None,
        ..PermissionMessages::default()
    };
    let catalog_text = sandbox_text(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Restricted,
        ResolvedPermissionMessages::new(Some(&messages)),
    );
    let bundled_text = sandbox_text(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Restricted,
        ResolvedPermissionMessages::new(/*messages*/ None),
    );

    // Identical source bytes still take distinct paths: only bundled templates are trimmed.
    assert_eq!(
        catalog_text,
        WORKSPACE_WRITE_TEMPLATE.replace("{{ network_access }}", "restricted")
    );
    assert_eq!(bundled_text, catalog_text.trim_end());
}

#[test]
fn missing_catalog_permission_message_uses_legacy_sandbox_text() {
    let legacy = sandbox_text(
        SandboxMode::WorkspaceWrite,
        NetworkAccess::Restricted,
        ResolvedPermissionMessages::new(/*messages*/ None),
    );
    let messages = PermissionMessages {
        danger_full_access: None,
        workspace_write: None,
        read_only: Some("unused".to_string()),
        approved_prefixes: None,
        ..PermissionMessages::default()
    };

    assert_eq!(
        sandbox_text(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Restricted,
            ResolvedPermissionMessages::new(Some(&messages)),
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
            approved_prefixes: None,
            ..PermissionMessages::default()
        };
        assert_eq!(
            sandbox_text(
                SandboxMode::WorkspaceWrite,
                NetworkAccess::Restricted,
                ResolvedPermissionMessages::new(Some(&messages)),
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
        approved_prefixes: None,
        ..PermissionMessages::default()
    };

    assert_eq!(
        sandbox_text(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Restricted,
            ResolvedPermissionMessages::new(Some(&messages)),
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
        approved_prefixes: None,
        ..PermissionMessages::default()
    };
    let writable_root = "/tmp/repo".to_string();
    let text = PermissionsInstructions::from_resolved(
        PermissionsRenderContext {
            network_access: NetworkAccess::Restricted,
            approval_policy: AskForApproval::Never,
            writable_roots: std::slice::from_ref(&writable_root),
            ..WORKSPACE_CONTEXT
        },
        ApprovalPromptContext {
            reviewer: ApprovalsReviewer::User,
            messages: ResolvedApprovalMessages::new(/*messages*/ None),
            permission_messages: ResolvedPermissionMessages::new(Some(&messages)),
        },
    )
    .body()
    .expect_text();

    assert!(!text.contains("Filesystem sandboxing defines"));
    assert!(text.contains("Approval policy is currently never"));
    assert!(text.contains(&writable_root));
}

#[test]
fn includes_request_rule_instructions_for_on_request() {
    let approved_command_prefixes = vec![vec!["git".to_string(), "pull".to_string()]];
    let text = PermissionsInstructions::from_resolved(
        PermissionsRenderContext {
            approved_command_prefixes: &approved_command_prefixes,
            ..WORKSPACE_CONTEXT
        },
        user_approval_context(),
    )
    .body()
    .expect_text();

    assert!(text.contains("prefix_rule"));
    assert!(text.contains("Approved command prefixes"));
    assert!(text.contains(r#"["git", "pull"]"#));
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
fn includes_request_permissions_tool_instructions_for_unless_trusted_when_enabled() {
    let text = PermissionsInstructions::from_resolved(
        PermissionsRenderContext {
            approval_policy: AskForApproval::UnlessTrusted,
            request_permissions_tool_enabled: true,
            ..WORKSPACE_CONTEXT
        },
        user_approval_context(),
    )
    .body()
    .expect_text();

    assert!(text.contains("`approval_policy` is `unless-trusted`"));
    assert!(text.contains("# request_permissions Tool"));
}

#[test]
fn on_request_permission_guidance_matches_enabled_capabilities() {
    for (exec_permission_approvals_enabled, request_permissions_tool_enabled) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let context = PermissionsRenderContext {
            exec_permission_approvals_enabled,
            request_permissions_tool_enabled,
            ..WORKSPACE_CONTEXT
        };
        let text = PermissionsInstructions::from_resolved(context, user_approval_context())
            .body()
            .expect_text();
        for (enabled, expected) in [
            (
                exec_permission_approvals_enabled,
                "with_additional_permissions",
            ),
            (exec_permission_approvals_enabled, "additional_permissions"),
            (
                request_permissions_tool_enabled,
                "# request_permissions Tool",
            ),
            (
                request_permissions_tool_enabled,
                "The built-in `request_permissions` tool is available in this session.",
            ),
        ] {
            assert_eq!(
                text.contains(expected),
                enabled,
                "{expected:?}: {context:?}"
            );
        }
    }
}

#[test]
fn catalog_approval_messages_select_reviewer_variant() {
    let messages = ApprovalMessages {
        on_request: Some("user catalog approvals".to_string()),
        on_request_auto_review: Some("auto-review catalog approvals".to_string()),
        never: Some("never catalog approvals".to_string()),
        unless_trusted: Some("unless-trusted catalog approvals".to_string()),
        granular: None,
        ..Default::default()
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
                ApprovalPromptContext {
                    reviewer,
                    messages: ResolvedApprovalMessages::new(Some(&messages)),
                    ..user_approval_context()
                },
                &[],
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
        granular: None,
        ..Default::default()
    };
    let approved_command_prefixes = vec![vec!["git".to_string(), "pull".to_string()]];
    let writable_root = "/tmp/repo".to_string();

    let text = PermissionsInstructions::from_resolved(
        PermissionsRenderContext {
            network_access: NetworkAccess::Restricted,
            approved_command_prefixes: &approved_command_prefixes,
            writable_roots: std::slice::from_ref(&writable_root),
            exec_permission_approvals_enabled: true,
            request_permissions_tool_enabled: true,
            ..WORKSPACE_CONTEXT
        },
        ApprovalPromptContext {
            reviewer: ApprovalsReviewer::User,
            messages: ResolvedApprovalMessages::new(Some(&messages)),
            permission_messages: ResolvedPermissionMessages::new(/*messages*/ None),
        },
    )
    .body()
    .expect_text();

    assert!(text.contains("`sandbox_mode` is `workspace-write`"));
    assert!(text.contains("Network access is restricted."));
    assert!(text.contains(&writable_root));
    assert!(!text.contains("How to request escalation"));
    assert!(!text.contains("request_permissions Tool"));
    assert!(!text.contains("Approved command prefixes"));
}

#[test]
fn missing_catalog_key_uses_legacy_approval_text() {
    for (reviewer, messages) in [
        (
            ApprovalsReviewer::User,
            ApprovalMessages {
                on_request: None,
                on_request_auto_review: Some("unused catalog approvals".to_string()),
                never: None,
                unless_trusted: None,
                granular: None,
                ..Default::default()
            },
        ),
        (
            ApprovalsReviewer::AutoReview,
            ApprovalMessages {
                on_request: Some("unused catalog approvals".to_string()),
                on_request_auto_review: None,
                never: None,
                unless_trusted: None,
                granular: None,
                ..Default::default()
            },
        ),
    ] {
        let approval_context = ApprovalPromptContext {
            reviewer,
            messages: ResolvedApprovalMessages::new(Some(&messages)),
            ..user_approval_context()
        };
        let on_request = approval_text(
            AskForApproval::OnRequest,
            approval_context,
            &[],
            /*exec_permission_approvals_enabled*/ false,
            /*request_permissions_tool_enabled*/ false,
        );
        let never = approval_text(
            AskForApproval::Never,
            approval_context,
            &[],
            /*exec_permission_approvals_enabled*/ false,
            /*request_permissions_tool_enabled*/ false,
        );
        assert!(on_request.contains("How to request escalation"));
        assert_eq!(
            on_request.contains(AUTO_REVIEW_SUFFIX),
            reviewer == ApprovalsReviewer::AutoReview
        );
        assert_eq!(
            never,
            ResolvedApprovalMessages::new(/*messages*/ None)
                .never
                .text()
        );
    }
}

#[test]
fn empty_catalog_non_on_request_approval_messages_suppress_legacy_approval_text() {
    let messages = ApprovalMessages {
        on_request: None,
        on_request_auto_review: None,
        never: Some(String::new()),
        unless_trusted: Some(String::new()),
        granular: None,
        ..Default::default()
    };

    for approval_policy in [AskForApproval::Never, AskForApproval::UnlessTrusted] {
        assert_eq!(
            approval_text(
                approval_policy,
                ApprovalPromptContext {
                    reviewer: ApprovalsReviewer::AutoReview,
                    messages: ResolvedApprovalMessages::new(Some(&messages)),
                    ..user_approval_context()
                },
                &[],
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
        ApprovalPromptContext::new(
            ApprovalsReviewer::AutoReview,
            ResolvedModelMessages::bundled(),
        ),
        &[],
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
        ApprovalPromptContext::new(
            ApprovalsReviewer::AutoReview,
            ResolvedModelMessages::bundled(),
        ),
        &[],
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );

    assert!(!text.contains("`approvals_reviewer` is `auto_review`"));
    assert!(!text.contains("`approvals_reviewer` is `guardian_subagent`"));
}

const ALL_APPROVALS_ENABLED: GranularApprovalConfig = GranularApprovalConfig {
    sandbox_approval: true,
    rules: true,
    skill_approval: true,
    request_permissions: true,
    mcp_elicitations: true,
};

const ALL_PROMPTED_CATEGORIES: &str =
    "These approval categories may still prompt the user when needed:
- `sandbox_approval`
- `rules`
- `skill_approval`
- `mcp_elicitations`";

#[test]
fn saved_prefix_heading_requires_rollout_and_plain_bounded_wording() {
    let boundary = "é".repeat(250);
    let oversized = format!("{boundary}x");
    for (heading, expected) in [
        (Some(boundary.as_str()), Some(boundary.as_str())),
        (Some(oversized.as_str()), None),
        (Some("<model_switch>"), None),
        (Some("Saved > prefixes"), None),
        (Some("Saved\u{2028}prefixes"), None),
        (Some("Saved\u{2029}prefixes"), None),
        (Some("Saved\rprefixes"), None),
        (Some("Saved\tprefixes"), None),
        (Some("Saved\n- injected prefix"), None),
        (Some("{{ approved_prefixes }}"), None),
        (Some("  "), None),
        (None, None),
    ] {
        let messages = PermissionMessages {
            approved_command_prefix_saved: heading.map(str::to_owned),
            approved_prefixes: Some("Unrelated full-list template".to_owned()),
            ..Default::default()
        };
        let context = ApprovalPromptContext {
            permission_messages: ResolvedPermissionMessages::new(Some(&messages)),
            ..user_approval_context()
        };
        assert_eq!(context.approved_command_prefix_saved_heading(), expected);
    }
}

#[test]
fn granular_approval_headings_follow_independent_tool_and_sandbox_gates() {
    let messages = ApprovalMessages {
        request_permissions: Some("Catalog permission tool".into()),
        auto_review_suffix: Some("Catalog reviewer".into()),
        ..Default::default()
    };
    let context = ApprovalPromptContext {
        messages: ResolvedApprovalMessages::new(Some(&messages)),
        ..user_approval_context()
    };
    for (sandbox_allowed, tool_allowed) in
        [(true, false), (false, true), (true, true), (false, false)]
    {
        let text = approval_text(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: sandbox_allowed,
                request_permissions: tool_allowed,
                ..ALL_APPROVALS_ENABLED
            }),
            context,
            &[],
            /*exec_permission_approvals_enabled*/ true,
            /*request_permissions_tool_enabled*/ true,
        );
        assert_eq!(text.contains(REQUEST_PERMISSION_RULE), sandbox_allowed);
        assert_eq!(text.contains("Catalog permission tool"), tool_allowed);
        assert!(!text.contains("Catalog reviewer"));
    }
}

#[test]
fn catalog_heading_fallback_and_policy_selection_are_independent() {
    let messages = ApprovalMessages {
        request_permissions: Some("Catalog permission tool".into()),
        auto_review_suffix: Some("{{ unknown }}".into()),
        ..Default::default()
    };
    let bundled = ApprovalPromptContext::new(
        ApprovalsReviewer::AutoReview,
        ResolvedModelMessages::bundled(),
    );
    let catalog = ApprovalPromptContext {
        messages: ResolvedApprovalMessages::new(Some(&messages)),
        ..bundled
    };
    for policy in [
        AskForApproval::Never,
        AskForApproval::UnlessTrusted,
        AskForApproval::OnRequest,
    ] {
        for tool_enabled in [false, true] {
            let render = |context| {
                approval_text(
                    policy,
                    context,
                    &[],
                    /*exec_permission_approvals_enabled*/ true,
                    tool_enabled,
                )
            };
            let default = render(bundled);
            let expected = if tool_enabled && policy != AskForApproval::Never {
                assert!(default.contains("# request_permissions Tool"));
                default.replace("# request_permissions Tool", "Catalog permission tool")
            } else {
                default
            };
            assert_eq!(render(catalog), expected);
        }
    }
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
        user_approval_context(),
        &[],
        /*exec_permission_approvals_enabled*/ true,
        /*request_permissions_tool_enabled*/ false,
    );

    assert_eq!(
        text,
        [
            GRANULAR_INTRO,
            "These approval categories may still prompt the user when needed:\n- `rules`",
            "These approval categories are automatically rejected instead of prompting the user:\n- `sandbox_approval`\n- `skill_approval`\n- `mcp_elicitations`",
        ]
        .join("\n\n")
    );
}

#[test]
fn granular_policy_includes_command_permission_instructions_when_sandbox_approval_can_prompt() {
    let text = approval_text(
        AskForApproval::Granular(ALL_APPROVALS_ENABLED),
        user_approval_context(),
        &[],
        /*exec_permission_approvals_enabled*/ true,
        /*request_permissions_tool_enabled*/ false,
    );

    assert_eq!(
        text,
        [
            GRANULAR_INTRO,
            ALL_PROMPTED_CATEGORIES,
            REQUEST_PERMISSION_RULE,
        ]
        .join("\n\n")
    );
}

#[test]
fn granular_policy_omits_shell_permission_instructions_when_inline_requests_are_disabled() {
    let text = approval_text(
        AskForApproval::Granular(ALL_APPROVALS_ENABLED),
        user_approval_context(),
        &[],
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );

    assert_eq!(
        text,
        [GRANULAR_INTRO, ALL_PROMPTED_CATEGORIES,].join("\n\n")
    );
}

#[test]
fn granular_policy_includes_request_permissions_tool_only_when_that_prompt_can_still_fire() {
    let allowed = approval_text(
        AskForApproval::Granular(ALL_APPROVALS_ENABLED),
        user_approval_context(),
        &[],
        /*exec_permission_approvals_enabled*/ true,
        /*request_permissions_tool_enabled*/ true,
    );
    assert!(allowed.contains("# request_permissions Tool"));

    let rejected = approval_text(
        AskForApproval::Granular(GranularApprovalConfig {
            request_permissions: false,
            ..ALL_APPROVALS_ENABLED
        }),
        user_approval_context(),
        &[],
        /*exec_permission_approvals_enabled*/ true,
        /*request_permissions_tool_enabled*/ true,
    );
    assert!(!rejected.contains("# request_permissions Tool"));
}

#[test]
fn preserves_supplied_path_spellings_and_order() {
    let writable_roots = vec![r"C:\work\z".to_string(), "/work/a".to_string()];
    let denied_read_paths = vec![r"C:\private".to_string(), "/private".to_string()];
    let denied_read_globs = vec![r"C:\private\**".to_string(), "/private/**".to_string()];
    let approval_messages = ApprovalMessages {
        on_request: Some(String::new()),
        on_request_auto_review: None,
        never: None,
        unless_trusted: None,
        granular: None,
        ..Default::default()
    };
    let permission_messages = PermissionMessages {
        danger_full_access: None,
        workspace_write: Some(String::new()),
        read_only: None,
        approved_prefixes: None,
        ..PermissionMessages::default()
    };

    let instructions = PermissionsInstructions::from_resolved(
        PermissionsRenderContext {
            writable_roots: &writable_roots,
            denied_read_paths: &denied_read_paths,
            denied_read_globs: &denied_read_globs,
            ..WORKSPACE_CONTEXT
        },
        ApprovalPromptContext {
            reviewer: ApprovalsReviewer::User,
            messages: ResolvedApprovalMessages::new(Some(&approval_messages)),
            permission_messages: ResolvedPermissionMessages::new(Some(&permission_messages)),
        },
    );
    let expected_body = concat!(
        "\n The writable roots are `C:\\work\\z`, `/work/a`.\n",
        "## Denied filesystem reads\n",
        "The active permission profile denies reading these paths/globs. Do not request escalation or additional permissions to read them; these denials are policy restrictions.\n",
        "- path `C:\\private`\n- path `/private`\n",
        "- glob `C:\\private\\**`\n- glob `/private/**`\n",
    );
    assert_eq!(
        instructions.render_fragment(),
        RenderedFragment::new(
            "developer",
            AnnotatedContent::text(
                format!("<permissions instructions>{expected_body}</permissions instructions>"),
                ContentItemKind("permissions.instructions".to_string()),
                ContentItemMetadata::harness(),
            ),
        ),
    );
}

#[test]
fn bounds_permission_paths_at_whole_entry_boundaries() {
    let render =
        |writable_roots: &[String], denied_read_paths: &[String], denied_read_globs: &[String]| {
            PermissionsInstructions::from_resolved(
                PermissionsRenderContext {
                    writable_roots,
                    denied_read_paths,
                    denied_read_globs,
                    ..WORKSPACE_CONTEXT
                },
                user_approval_context(),
            )
            .body()
            .expect_text()
        };
    // Any two fit, but all three exceed the shared budget. Include multibyte paths.
    let writable = format!("/writable/{}", "é".repeat(6_000));
    let denied = format!("/denied/{}", "é".repeat(6_000));
    let glob = format!("/glob/{}/**", "é".repeat(6_000));
    let oversized = format!("/oversized/{}", "x".repeat(MAX_PERMISSION_PATH_BYTES));
    let text = render(
        &[oversized, writable.clone()],
        std::slice::from_ref(&denied),
        &[glob, "/short/**".into()],
    );
    let expected = render(&[writable], &[denied], &["/short/**".into()]);
    assert_eq!(text, format!("{expected}{OMITTED_PERMISSION_PATHS}\n"));
}

#[test]
fn catalog_writable_roots_preserve_fragment_facts() {
    let messages = PermissionMessages {
        writable_root: Some("Catalog root: {{ roots }}.".into()),
        writable_roots: Some("Catalog roots: {{ roots }}.".into()),
        ..PermissionMessages::default()
    };
    let catalog_context = ApprovalPromptContext {
        permission_messages: ResolvedPermissionMessages::new(Some(&messages)),
        ..user_approval_context()
    };
    let denied = vec!["/private".to_string()];
    for roots in [
        vec![],
        vec!["/a/{{ roots }}".to_string()],
        vec![r"C:\work\z".to_string(), "/a/é\n{literal}".to_string()],
    ] {
        let context = PermissionsRenderContext {
            writable_roots: &roots,
            denied_read_paths: &denied,
            ..WORKSPACE_CONTEXT
        };
        let original = PermissionsInstructions::from_resolved(context, user_approval_context());
        let expected = PermissionsInstructions {
            text: original
                .text
                .replace(" The writable root is ", "Catalog root: ")
                .replace(" The writable roots are ", "Catalog roots: "),
        };
        assert_eq!(
            PermissionsInstructions::from_resolved(context, catalog_context,).render_fragment(),
            expected.render_fragment(),
        );
    }
}

#[test]
fn catalog_writable_roots_select_displayed_count_after_truncation() {
    let messages = PermissionMessages {
        writable_root: Some("Catalog root: {{ roots }}.".into()),
        writable_roots: Some("Catalog roots: {{ roots }}.".into()),
        ..PermissionMessages::default()
    };
    let roots = vec!["x".repeat(MAX_PERMISSION_PATH_BYTES), "/short".to_string()];
    let context = PermissionsRenderContext {
        writable_roots: &roots,
        ..WORKSPACE_CONTEXT
    };
    let original = PermissionsInstructions::from_resolved(context, user_approval_context());
    let expected = PermissionsInstructions {
        text: original
            .text
            .replace(" The writable root is ", "Catalog root: "),
    };
    let actual = PermissionsInstructions::from_resolved(
        context,
        ApprovalPromptContext {
            permission_messages: ResolvedPermissionMessages::new(Some(&messages)),
            ..user_approval_context()
        },
    );
    assert_eq!(actual.render_fragment(), expected.render_fragment());
    assert!(
        actual
            .text
            .ends_with(&format!("{OMITTED_PERMISSION_PATHS}\n"))
    );
}

#[test]
fn catalog_path_warnings_preserve_facts_selection_and_default_bytes() {
    let messages = PermissionMessages {
        denied_reads: Some("Catalog denials:\n{{ denied_reads }}\nEnd denials.".into()),
        omitted_paths: Some("Catalog omission.".into()),
        writable_root: Some("Catalog root: {{ roots }}.".into()),
        ..Default::default()
    };
    // Fits the legacy budget with the short denials below, even after an oversized entry.
    let roots = ["x".repeat(MAX_PERMISSION_PATH_BYTES), "r".repeat(32_500)];
    let denied = ["/é/{{ denied_reads }}\n{literal}".to_string()];
    let globs = [r"C:\private\**".to_string()];
    let context = PermissionsRenderContext {
        writable_roots: &roots,
        denied_read_paths: &denied,
        denied_read_globs: &globs,
        ..WORKSPACE_CONTEXT
    };
    let catalog = ApprovalPromptContext {
        permission_messages: ResolvedPermissionMessages::new(Some(&messages)),
        ..user_approval_context()
    };
    let bundled = PermissionsInstructions::from_resolved(context, user_approval_context());
    let original = PermissionsInstructions {
        text: bundled
            .text
            .replace(" The writable root is ", "Catalog root: "),
    };
    let expected = PermissionsInstructions {
        text: original
            .text
            .replace("## Denied filesystem reads", "Catalog denials:")
            .replace(
                OMITTED_PERMISSION_PATHS,
                &format!("End denials.\nCatalog omission.\n{OMITTED_PERMISSION_PATHS}"),
            ),
    };
    let actual = PermissionsInstructions::from_resolved(context, catalog);
    assert_eq!(actual.render_fragment(), expected.render_fragment());
}

#[test]
fn unusable_path_warning_copy_falls_back_independently() {
    let roots = ["x".repeat(MAX_PERMISSION_PATH_BYTES)];
    let denied = ["/private".to_string()];
    let context = PermissionsRenderContext {
        writable_roots: &roots,
        denied_read_paths: &denied,
        ..WORKSPACE_CONTEXT
    };
    let original = PermissionsInstructions::from_resolved(context, user_approval_context());
    for invalid in [
        "",
        " ",
        "{{ unknown }}",
        "<permissions instructions>",
        "</permissions instructions>",
        "\t",
        "\r",
        "\0",
        "\u{2028}",
        "\u{2029}",
        &"é".repeat(251),
    ] {
        for (omitted_paths, denied_reads, expected) in [
            (
                invalid.to_string(),
                "Blocked:\n{{ denied_reads }}".into(),
                original
                    .text
                    .replace("## Denied filesystem reads", "Blocked:"),
            ),
            (
                "Omission:".into(),
                format!("{invalid}{{{{ denied_reads }}}}"),
                original.text.replace(
                    OMITTED_PERMISSION_PATHS,
                    &format!("Omission:\n{OMITTED_PERMISSION_PATHS}"),
                ),
            ),
        ] {
            // A marker on its own is valid: test an empty/whitespace template without adding it.
            let denied_reads = if invalid.trim().is_empty() && omitted_paths == "Omission:" {
                invalid.to_string()
            } else {
                denied_reads
            };
            let messages = PermissionMessages {
                omitted_paths: Some(omitted_paths),
                denied_reads: Some(denied_reads),
                ..Default::default()
            };
            let actual = PermissionsInstructions::from_resolved(
                context,
                ApprovalPromptContext {
                    permission_messages: ResolvedPermissionMessages::new(Some(&messages)),
                    ..user_approval_context()
                },
            );
            assert_eq!(actual.text, expected, "invalid={invalid:?}");
        }
    }
    for denied_reads in [
        "No placeholder",
        "{{denied_reads}}",
        "{{ denied_reads }} {{ denied_reads }}",
        "{{{ denied_reads }}}",
    ] {
        let messages = PermissionMessages {
            denied_reads: Some(denied_reads.into()),
            ..Default::default()
        };
        let actual = PermissionsInstructions::from_resolved(
            context,
            ApprovalPromptContext {
                permission_messages: ResolvedPermissionMessages::new(Some(&messages)),
                ..user_approval_context()
            },
        );
        assert_eq!(actual.text, original.text);
    }
}

#[test]
fn path_warning_copy_is_bounded_without_suppressing_omitted_denials() {
    let denied = ["x".repeat(MAX_PERMISSION_PATH_BYTES), "/private".into()];
    for bytes in [500, 501] {
        let lead = "x".repeat(bytes);
        let heading = "y".repeat(bytes - "\n{{ denied_reads }}".len());
        let messages = PermissionMessages {
            omitted_paths: Some(lead.clone()),
            denied_reads: Some(format!("{heading}\n{{{{ denied_reads }}}}")),
            ..Default::default()
        };
        for paths in [&denied[..], &denied[..1], &denied[..0]] {
            let context = PermissionsRenderContext {
                denied_read_paths: paths,
                ..WORKSPACE_CONTEXT
            };
            let original = PermissionsInstructions::from_resolved(context, user_approval_context());
            let expected = if bytes == 500 {
                original
                    .text
                    .replace("## Denied filesystem reads", &heading)
                    .replace(
                        OMITTED_PERMISSION_PATHS,
                        &format!("{lead}\n{OMITTED_PERMISSION_PATHS}"),
                    )
            } else {
                original.text
            };
            let actual = PermissionsInstructions::from_resolved(
                context,
                ApprovalPromptContext {
                    permission_messages: ResolvedPermissionMessages::new(Some(&messages)),
                    ..user_approval_context()
                },
            );
            assert_eq!(actual.text, expected);
        }
    }
}
