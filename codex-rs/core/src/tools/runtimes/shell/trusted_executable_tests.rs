use super::ParentApprovedIntercept;
use super::trusted_executable_dirs;
use crate::sandboxing::SandboxPermissions;
use crate::tools::sandboxing::ExecApprovalRequirement;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;

fn host_git() -> &'static Path {
    ["/usr/bin/git", "/bin/git", "/opt/homebrew/bin/git"]
        .into_iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .expect("test host should provide git")
}

fn trusted_git_fixture() -> (
    AbsolutePathBuf,
    AbsolutePathBuf,
    PermissionProfile,
    Vec<super::TrustedExecutableDir>,
) {
    let program = AbsolutePathBuf::from_absolute_path(host_git()).unwrap();
    let cwd = AbsolutePathBuf::try_from(std::env::current_dir().unwrap()).unwrap();
    let permission_profile = PermissionProfile::workspace_write();
    let mut env = HashMap::new();
    env.insert(
        "PATH".to_string(),
        program
            .as_path()
            .parent()
            .expect("git should have a parent")
            .display()
            .to_string(),
    );
    let trusted_dirs =
        trusted_executable_dirs(&env, &permission_profile.file_system_sandbox_policy(), &cwd);
    assert!(!trusted_dirs.is_empty());
    (program, cwd, permission_profile, trusted_dirs)
}

#[test]
fn parent_approved_git_intercept_is_narrowly_scoped() {
    let command = vec![
        "/bin/zsh".to_string(),
        "-lc".to_string(),
        "git status --short".to_string(),
    ];
    let heuristic_prompt = ExecApprovalRequirement::NeedsApproval {
        reason: None,
        proposed_execpolicy_amendment: None,
    };
    assert!(
        ParentApprovedIntercept::for_parent_git_approval(
            &command,
            &heuristic_prompt,
            AskForApproval::UnlessTrusted,
            SandboxPermissions::UseDefault,
            /*additional_permissions*/ None,
        )
        .is_some()
    );

    let policy_prompt = ExecApprovalRequirement::NeedsApproval {
        reason: Some("required by policy".to_string()),
        proposed_execpolicy_amendment: None,
    };
    assert!(
        ParentApprovedIntercept::for_parent_git_approval(
            &command,
            &policy_prompt,
            AskForApproval::UnlessTrusted,
            SandboxPermissions::UseDefault,
            /*additional_permissions*/ None,
        )
        .is_none()
    );
    assert!(
        ParentApprovedIntercept::for_parent_git_approval(
            &command,
            &heuristic_prompt,
            AskForApproval::UnlessTrusted,
            SandboxPermissions::RequireEscalated,
            /*additional_permissions*/ None,
        )
        .is_none()
    );
    assert!(
        ParentApprovedIntercept::for_parent_git_approval(
            &command,
            &heuristic_prompt,
            AskForApproval::UnlessTrusted,
            SandboxPermissions::UseDefault,
            Some(&AdditionalPermissionProfile::default()),
        )
        .is_none()
    );
    assert!(
        ParentApprovedIntercept::for_parent_git_approval(
            &["/bin/zsh".into(), "-lc".into(), "env git status".into()],
            &heuristic_prompt,
            AskForApproval::UnlessTrusted,
            SandboxPermissions::UseDefault,
            /*additional_permissions*/ None,
        )
        .is_none()
    );
}

#[test]
fn parent_approval_matches_one_exact_trusted_intercept() {
    let (program, cwd, permission_profile, trusted_dirs) = trusted_git_fixture();
    let approved = ParentApprovedIntercept::new(vec![
        "git".to_string(),
        "status".to_string(),
        "--short".to_string(),
    ]);
    let argv = [
        "git".to_string(),
        "status".to_string(),
        "--short".to_string(),
    ];

    assert!(approved.consume_if_matches(
        &program,
        &argv,
        &trusted_dirs,
        &permission_profile.file_system_sandbox_policy(),
        &cwd,
    ));
    assert!(!approved.consume_if_matches(
        &program,
        &argv,
        &trusted_dirs,
        &permission_profile.file_system_sandbox_policy(),
        &cwd,
    ));
}

#[test]
fn parent_approval_rejects_changed_or_path_qualified_argv() {
    let (program, cwd, permission_profile, trusted_dirs) = trusted_git_fixture();
    for argv in [
        vec!["git".to_string(), "diff".to_string()],
        vec![
            program.to_string_lossy().to_string(),
            "status".to_string(),
            "--short".to_string(),
        ],
    ] {
        let approved = ParentApprovedIntercept::new(vec![
            "git".to_string(),
            "status".to_string(),
            "--short".to_string(),
        ]);
        assert!(!approved.consume_if_matches(
            &program,
            &argv,
            &trusted_dirs,
            &permission_profile.file_system_sandbox_policy(),
            &cwd,
        ));
    }
}

#[test]
fn parent_approval_rejects_writable_path_shadow() {
    let writable_dir = tempfile::tempdir().unwrap();
    let shadow_git = writable_dir.path().join("git");
    std::fs::write(&shadow_git, "not a trusted host executable").unwrap();
    let cwd = AbsolutePathBuf::from_absolute_path(writable_dir.path()).unwrap();
    let permission_profile = PermissionProfile::workspace_write();
    let mut env = HashMap::new();
    env.insert(
        "PATH".to_string(),
        writable_dir.path().display().to_string(),
    );
    let trusted_dirs =
        trusted_executable_dirs(&env, &permission_profile.file_system_sandbox_policy(), &cwd);
    let approved = ParentApprovedIntercept::new(vec!["git".into(), "status".into()]);

    assert!(trusted_dirs.is_empty());
    assert!(!approved.consume_if_matches(
        &AbsolutePathBuf::from_absolute_path(&shadow_git).unwrap(),
        &["git".into(), "status".into()],
        &trusted_dirs,
        &permission_profile.file_system_sandbox_policy(),
        &cwd,
    ));
}
