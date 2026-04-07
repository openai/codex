use super::*;
use codex_protocol::protocol::FileSystemAccessMode;
use codex_protocol::protocol::FileSystemPath;
use codex_protocol::protocol::FileSystemSandboxEntry;
use codex_protocol::protocol::FileSystemSpecialPath;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn test_writable_roots_constraint() {
    // Use a temporary directory as our workspace to avoid touching
    // the real current working directory.
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_path_buf();
    let parent = cwd.parent().unwrap().to_path_buf();

    // Helper to build a single‑entry patch that adds a file at `p`.
    let make_add_change = |p: PathBuf| ApplyPatchAction::new_add_for_test(&p, "".to_string());

    let add_inside = make_add_change(cwd.join("inner.txt"));
    let add_outside = make_add_change(parent.join("outside.txt"));

    // Policy limited to the workspace only; exclude system temp roots so
    // only `cwd` is writable by default.
    let policy_workspace_only = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: false,
        allow_limited_git_writes: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    assert!(is_write_patch_constrained_to_writable_paths(
        &add_inside,
        &FileSystemSandboxPolicy::from(&policy_workspace_only),
        &cwd,
    ));

    assert!(!is_write_patch_constrained_to_writable_paths(
        &add_outside,
        &FileSystemSandboxPolicy::from(&policy_workspace_only),
        &cwd,
    ));

    // With the parent dir explicitly added as a writable root, the
    // outside write should be permitted.
    let policy_with_parent = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![AbsolutePathBuf::try_from(parent).unwrap()],
        read_only_access: Default::default(),
        network_access: false,
        allow_limited_git_writes: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    assert!(is_write_patch_constrained_to_writable_paths(
        &add_outside,
        &FileSystemSandboxPolicy::from(&policy_with_parent),
        &cwd,
    ));
}

#[test]
fn external_sandbox_auto_approves_in_on_request() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_path_buf();
    let add_inside = ApplyPatchAction::new_add_for_test(&cwd.join("inner.txt"), "".to_string());

    let policy = SandboxPolicy::ExternalSandbox {
        network_access: codex_protocol::protocol::NetworkAccess::Enabled,
    };

    assert_eq!(
        assess_patch_safety(
            &add_inside,
            AskForApproval::OnRequest,
            &policy,
            &FileSystemSandboxPolicy::from(&policy),
            &cwd,
            WindowsSandboxLevel::Disabled
        ),
        SafetyCheck::AutoApprove {
            sandbox_type: SandboxType::None,
            user_explicitly_approved: false,
        }
    );
}

#[test]
fn granular_with_all_flags_true_matches_on_request_for_out_of_root_patch() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_path_buf();
    let parent = cwd.parent().unwrap().to_path_buf();
    let add_outside =
        ApplyPatchAction::new_add_for_test(&parent.join("outside.txt"), "".to_string());
    let policy_workspace_only = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: false,
        allow_limited_git_writes: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    assert_eq!(
        assess_patch_safety(
            &add_outside,
            AskForApproval::OnRequest,
            &policy_workspace_only,
            &FileSystemSandboxPolicy::from(&policy_workspace_only),
            &cwd,
            WindowsSandboxLevel::Disabled,
        ),
        SafetyCheck::AskUser,
    );
    assert_eq!(
        assess_patch_safety(
            &add_outside,
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            &policy_workspace_only,
            &FileSystemSandboxPolicy::from(&policy_workspace_only),
            &cwd,
            WindowsSandboxLevel::Disabled,
        ),
        SafetyCheck::AskUser,
    );
}

#[test]
fn granular_sandbox_approval_false_rejects_out_of_root_patch() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_path_buf();
    let parent = cwd.parent().unwrap().to_path_buf();
    let add_outside =
        ApplyPatchAction::new_add_for_test(&parent.join("outside.txt"), "".to_string());
    let policy_workspace_only = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: false,
        allow_limited_git_writes: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    assert_eq!(
        assess_patch_safety(
            &add_outside,
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: false,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            &policy_workspace_only,
            &FileSystemSandboxPolicy::from(&policy_workspace_only),
            &cwd,
            WindowsSandboxLevel::Disabled,
        ),
        SafetyCheck::Reject {
            reason: PATCH_REJECTED_OUTSIDE_PROJECT_REASON.to_string(),
        },
    );
}

#[test]
fn read_only_policy_rejects_patch_with_read_only_reason() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_path_buf();
    let action = ApplyPatchAction::new_add_for_test(&cwd.join("inside.txt"), "".to_string());
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let file_system_sandbox_policy =
        FileSystemSandboxPolicy::from_legacy_sandbox_policy(&sandbox_policy, &cwd);

    assert!(!is_write_patch_constrained_to_writable_paths(
        &action,
        &file_system_sandbox_policy,
        &cwd,
    ));
    assert_eq!(
        assess_patch_safety(
            &action,
            AskForApproval::Never,
            &sandbox_policy,
            &file_system_sandbox_policy,
            &cwd,
            WindowsSandboxLevel::Disabled,
        ),
        SafetyCheck::Reject {
            reason: PATCH_REJECTED_READ_ONLY_REASON.to_string(),
        },
    );
}
#[test]
fn explicit_unreadable_paths_prevent_auto_approval_for_external_sandbox() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_path_buf();
    let blocked_path = cwd.join("blocked.txt");
    let blocked_absolute = AbsolutePathBuf::from_absolute_path(blocked_path.clone()).unwrap();
    let action = ApplyPatchAction::new_add_for_test(&blocked_path, "".to_string());
    let sandbox_policy = SandboxPolicy::ExternalSandbox {
        network_access: codex_protocol::protocol::NetworkAccess::Restricted,
    };
    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: blocked_absolute,
            },
            access: FileSystemAccessMode::None,
        },
    ]);

    assert!(!is_write_patch_constrained_to_writable_paths(
        &action,
        &file_system_sandbox_policy,
        &cwd,
    ));
    assert_eq!(
        assess_patch_safety(
            &action,
            AskForApproval::OnRequest,
            &sandbox_policy,
            &file_system_sandbox_policy,
            &cwd,
            WindowsSandboxLevel::Disabled,
        ),
        SafetyCheck::AskUser,
    );
}

#[test]
fn explicit_read_only_subpaths_prevent_auto_approval_for_external_sandbox() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_path_buf();
    let blocked_path = cwd.join("docs").join("blocked.txt");
    let docs_absolute = AbsolutePathBuf::resolve_path_against_base("docs", &cwd);
    let action = ApplyPatchAction::new_add_for_test(&blocked_path, "".to_string());
    let sandbox_policy = SandboxPolicy::ExternalSandbox {
        network_access: codex_protocol::protocol::NetworkAccess::Restricted,
    };
    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::CurrentWorkingDirectory,
            },
            access: FileSystemAccessMode::Write,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: docs_absolute,
            },
            access: FileSystemAccessMode::Read,
        },
    ]);

    assert!(!is_write_patch_constrained_to_writable_paths(
        &action,
        &file_system_sandbox_policy,
        &cwd,
    ));
    assert_eq!(
        assess_patch_safety(
            &action,
            AskForApproval::OnRequest,
            &sandbox_policy,
            &file_system_sandbox_policy,
            &cwd,
            WindowsSandboxLevel::Disabled,
        ),
        SafetyCheck::AskUser,
    );
}

#[test]
fn missing_project_dot_codex_config_requires_approval() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_path_buf();
    let action =
        ApplyPatchAction::new_add_for_test(&cwd.join(".codex").join("config.toml"), "".to_string());
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: false,
        allow_limited_git_writes: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    let file_system_sandbox_policy =
        FileSystemSandboxPolicy::from_legacy_sandbox_policy(&sandbox_policy, &cwd);

    assert!(!is_write_patch_constrained_to_writable_paths(
        &action,
        &file_system_sandbox_policy,
        &cwd,
    ));
    assert_eq!(
        assess_patch_safety(
            &action,
            AskForApproval::OnRequest,
            &sandbox_policy,
            &file_system_sandbox_policy,
            &cwd,
            WindowsSandboxLevel::Disabled,
        ),
        SafetyCheck::AskUser,
    );
}
