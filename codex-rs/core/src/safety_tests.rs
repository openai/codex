use super::*;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicyContext;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::FileSystemAccessMode;
use codex_protocol::protocol::FileSystemPath;
use codex_protocol::protocol::FileSystemSandboxEntry;
use codex_protocol::protocol::FileSystemSpecialPath;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use core_test_support::PathExt;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn local_context(cwd: &PathUri) -> FileSystemSandboxPolicyContext<'_> {
    FileSystemSandboxPolicyContext {
        cwd,
        workspace_roots: std::slice::from_ref(cwd),
        user_home_dir: None,
        temporary_directories: None,
    }
}

#[cfg(target_os = "macos")]
#[test]
fn local_system_aliases_do_not_change_remote_patch_permissions() -> std::io::Result<()> {
    let directory = tempfile::tempdir_in("/tmp")?;
    let cwd = PathUri::from_host_native_path(directory.path())?;
    let target =
        PathUri::from_host_native_path(directory.path().canonicalize()?.join("missing/file"))?;
    let action = ApplyPatchAction::new_add_for_test(&target, String::new());
    let permissions = PermissionProfile::workspace_write();
    let policy = permissions.file_system_sandbox_policy();
    assert_eq!(
        assess_patch_safety(
            &action,
            AskForApproval::OnRequest,
            &permissions,
            &PatchSandboxRoute::ExecutorManaged
                .prepare_matching(&policy, &local_context(&cwd))
                .expect("prepare patch matching"),
        )
        .expect("assess patch safety"),
        SafetyCheck::AskUser,
    );
    Ok(())
}

#[test]
fn windows_patch_matching_is_uri_native() {
    let cwd = PathUri::parse("file:///C:/workspace").expect("Windows cwd");
    let context = local_context(&cwd);
    let permission_profile = PermissionProfile::workspace_write();
    let policy = permission_profile.file_system_sandbox_policy();
    let inside = ApplyPatchAction::new_add_for_test(
        &PathUri::parse("file:///C:/workspace/in.txt").expect("inside"),
        String::new(),
    );
    let outside = ApplyPatchAction::new_add_for_test(
        &PathUri::parse("file:///C:/outside.txt").expect("outside"),
        String::new(),
    );

    assert!(
        !is_write_patch_constrained_to_writable_paths(
            &outside,
            &PatchSandboxRoute::ExecutorManaged
                .prepare_matching(&policy, &context)
                .expect("prepare patch matching"),
        )
        .expect("check writable paths")
    );
    assert_eq!(
        assess_patch_safety(
            &inside,
            AskForApproval::Never,
            &permission_profile,
            &PatchSandboxRoute::ExecutorManaged
                .prepare_matching(&policy, &context)
                .expect("prepare patch matching"),
        )
        .expect("assess patch safety"),
        SafetyCheck::AutoApprove,
    );
}

#[test]
fn full_disk_write_uses_executor_path_convention() {
    use FileSystemAccessMode::Deny;
    use FileSystemAccessMode::Read;
    use FileSystemAccessMode::Write;
    use FileSystemSpecialPath::Root;
    use FileSystemSpecialPath::SlashTmp;

    for (root, full_disk_write) in [("file:///", false), ("file:///C:/", true)] {
        let cwd = PathUri::parse(root).expect("executor root");
        let context = local_context(&cwd);
        let mut policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(FileSystemPath::Special { value: Root }, Write),
            FileSystemSandboxEntry::new(FileSystemPath::Special { value: SlashTmp }, Deny),
        ]);
        let action = ApplyPatchAction::new_add_for_test(
            &cwd.join("tmp/blocked.txt").expect("patch target"),
            String::new(),
        );

        assert_eq!(
            policy.has_full_disk_write_access_with_context(&context),
            full_disk_write
        );
        assert_eq!(
            assess_patch_safety(
                &action,
                AskForApproval::OnRequest,
                &PermissionProfile::from_runtime_permissions(
                    &policy,
                    NetworkSandboxPolicy::Restricted
                ),
                &PatchSandboxRoute::ExecutorManaged
                    .prepare_matching(&policy, &context)
                    .expect("prepare patch matching"),
            )
            .expect("assess patch safety"),
            if full_disk_write {
                SafetyCheck::AutoApprove
            } else {
                SafetyCheck::AskUser
            },
        );

        // Literal aliases can override equally specific read grants, but never denies.
        policy.entries[1].access = Read;
        policy.entries.push(FileSystemSandboxEntry::new(
            cwd.join("tmp").expect("tmp alias").into(),
            Write,
        ));
        assert!(policy.has_full_disk_write_access_with_context(&context));
        policy.entries[1].access = Deny;
        assert_eq!(
            policy.has_full_disk_write_access_with_context(&context),
            full_disk_write
        );
        policy.entries[1].path = cwd.clone().into();
        policy.entries[1].access = Read;
        assert!(policy.has_full_disk_write_access_with_context(&context));
        policy.entries[1].access = Deny;
        assert!(!policy.has_full_disk_write_access_with_context(&context));
    }
}

#[test]
fn test_writable_roots_constraint() {
    // Use a temporary directory as our workspace to avoid touching
    // the real current working directory.
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().abs();
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let parent = cwd.parent().unwrap();

    // Helper to build a single‑entry patch that adds a file at `p`.
    let make_add_change = |p: AbsolutePathBuf| {
        ApplyPatchAction::new_add_for_test(&PathUri::from_abs_path(&p), "".to_string())
    };

    let add_inside = make_add_change(cwd.join("inner.txt"));
    let add_outside = make_add_change(parent.join("outside.txt"));

    // Policy limited to the workspace only; exclude system temp roots so
    // only `cwd` is writable by default.
    let workspace_only_file_system_policy = FileSystemSandboxPolicy::workspace_write(
        &[],
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    );

    assert!(
        is_write_patch_constrained_to_writable_paths(
            &add_inside,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&workspace_only_file_system_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("check writable paths")
    );

    assert!(
        !is_write_patch_constrained_to_writable_paths(
            &add_outside,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&workspace_only_file_system_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("check writable paths")
    );

    // With the parent dir explicitly added as a writable root, the
    // outside write should be permitted.
    let file_system_policy_with_parent = FileSystemSandboxPolicy::workspace_write(
        std::slice::from_ref(&parent),
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    );
    assert!(
        is_write_patch_constrained_to_writable_paths(
            &add_outside,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_policy_with_parent, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("check writable paths")
    );
}

#[test]
fn external_sandbox_auto_approves_in_on_request() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().abs();
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let add_inside_path = cwd.join("inner.txt");
    let add_inside = ApplyPatchAction::new_add_for_test(
        &PathUri::from_abs_path(&add_inside_path),
        "".to_string(),
    );

    let permission_profile = PermissionProfile::External {
        network: NetworkSandboxPolicy::Enabled,
    };
    let file_system_sandbox_policy = FileSystemSandboxPolicy::external_sandbox();

    assert_eq!(
        assess_patch_safety(
            &add_inside,
            AskForApproval::OnRequest,
            &permission_profile,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_sandbox_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("assess patch safety"),
        SafetyCheck::AutoApprove
    );
}

#[test]
fn granular_with_all_flags_true_matches_on_request_for_out_of_root_patch() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().abs();
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let parent = cwd.parent().unwrap();
    let outside_path = parent.join("outside.txt");
    let add_outside =
        ApplyPatchAction::new_add_for_test(&PathUri::from_abs_path(&outside_path), "".to_string());
    let permission_profile = PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Restricted,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    );
    let file_system_sandbox_policy = permission_profile.file_system_sandbox_policy();

    assert_eq!(
        assess_patch_safety(
            &add_outside,
            AskForApproval::OnRequest,
            &permission_profile,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_sandbox_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("assess patch safety"),
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
            &permission_profile,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_sandbox_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("assess patch safety"),
        SafetyCheck::AskUser,
    );
}

#[test]
fn granular_sandbox_approval_false_rejects_out_of_root_patch() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().abs();
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let parent = cwd.parent().unwrap();
    let outside_path = parent.join("outside.txt");
    let add_outside =
        ApplyPatchAction::new_add_for_test(&PathUri::from_abs_path(&outside_path), "".to_string());
    let permission_profile = PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Restricted,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    );
    let file_system_sandbox_policy = permission_profile.file_system_sandbox_policy();

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
            &permission_profile,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_sandbox_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("assess patch safety"),
        SafetyCheck::Reject {
            reason: PATCH_REJECTED_OUTSIDE_PROJECT_REASON.to_string(),
        },
    );
}

#[test]
fn read_only_policy_rejects_patch_with_read_only_reason() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().abs();
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let inside_path = cwd.join("inside.txt");
    let action =
        ApplyPatchAction::new_add_for_test(&PathUri::from_abs_path(&inside_path), "".to_string());
    let permission_profile = PermissionProfile::read_only();
    let file_system_sandbox_policy = permission_profile.file_system_sandbox_policy();

    assert!(
        !is_write_patch_constrained_to_writable_paths(
            &action,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_sandbox_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("check writable paths")
    );
    assert_eq!(
        assess_patch_safety(
            &action,
            AskForApproval::Never,
            &permission_profile,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_sandbox_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("assess patch safety"),
        SafetyCheck::Reject {
            reason: PATCH_REJECTED_READ_ONLY_REASON.to_string(),
        },
    );
}
#[test]
fn explicit_unreadable_paths_prevent_auto_approval_for_external_sandbox() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().abs();
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let blocked_path = cwd.join("blocked.txt");
    let blocked_absolute = blocked_path;
    let action = ApplyPatchAction::new_add_for_test(
        &PathUri::from_abs_path(&blocked_absolute),
        "".to_string(),
    );
    let permission_profile = PermissionProfile::External {
        network: NetworkSandboxPolicy::Restricted,
    };
    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: blocked_absolute.into(),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    assert!(
        !is_write_patch_constrained_to_writable_paths(
            &action,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_sandbox_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("check writable paths")
    );
    assert_eq!(
        assess_patch_safety(
            &action,
            AskForApproval::OnRequest,
            &permission_profile,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_sandbox_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("assess patch safety"),
        SafetyCheck::AskUser,
    );
}

#[test]
fn explicit_read_only_subpaths_prevent_auto_approval_for_external_sandbox() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().abs();
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let blocked_path = cwd.join("docs").join("blocked.txt");
    let blocked_absolute = blocked_path;
    let docs_absolute = AbsolutePathBuf::resolve_path_against_base("docs", &cwd);
    let action = ApplyPatchAction::new_add_for_test(
        &PathUri::from_abs_path(&blocked_absolute),
        "".to_string(),
    );
    let permission_profile = PermissionProfile::External {
        network: NetworkSandboxPolicy::Restricted,
    };
    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: docs_absolute.into(),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
    ]);

    assert!(
        !is_write_patch_constrained_to_writable_paths(
            &action,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_sandbox_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("check writable paths")
    );
    assert_eq!(
        assess_patch_safety(
            &action,
            AskForApproval::OnRequest,
            &permission_profile,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_sandbox_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("assess patch safety"),
        SafetyCheck::AskUser,
    );
}

#[test]
fn missing_project_dot_codex_config_requires_approval() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().abs();
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let config_path = cwd.join(".codex").join("config.toml");
    let action =
        ApplyPatchAction::new_add_for_test(&PathUri::from_abs_path(&config_path), "".to_string());
    let permission_profile = PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Restricted,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    );
    let mut file_system_sandbox_policy = permission_profile.file_system_sandbox_policy();
    file_system_sandbox_policy
        .entries
        .push(FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: cwd.join(".codex").into(),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        });

    assert!(
        !is_write_patch_constrained_to_writable_paths(
            &action,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_sandbox_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("check writable paths")
    );
    assert_eq!(
        assess_patch_safety(
            &action,
            AskForApproval::OnRequest,
            &permission_profile,
            &PatchSandboxRoute::Platform(WindowsSandboxLevel::Disabled)
                .prepare_matching(&file_system_sandbox_policy, &local_context(&cwd_uri))
                .expect("prepare patch matching"),
        )
        .expect("assess patch safety"),
        SafetyCheck::AskUser,
    );
}
