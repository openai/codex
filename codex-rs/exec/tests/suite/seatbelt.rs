use super::sandbox::spawn_command_under_sandbox;
use codex_core::spawn::StdioPolicy;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::process::Output;
use std::time::Duration;
use tempfile::TempDir;

fn workspace_fixture() -> (TempDir, AbsolutePathBuf) {
    let temp_dir = TempDir::new().expect("create temporary directory");
    let workspace = temp_dir.path().join("workspace");
    fs::create_dir(&workspace).expect("create workspace");
    let workspace = AbsolutePathBuf::from_absolute_path(
        workspace.canonicalize().expect("canonicalize workspace"),
    )
    .expect("workspace is absolute");
    (temp_dir, workspace)
}

fn permission_profile(
    workspace: &AbsolutePathBuf,
    restricted_paths: Vec<(FileSystemPath, FileSystemAccessMode)>,
) -> PermissionProfile {
    let mut file_system_policy = FileSystemSandboxPolicy::read_only();
    file_system_policy.entries.push(FileSystemSandboxEntry::new(
        workspace.clone().into(),
        FileSystemAccessMode::Write,
    ));
    file_system_policy.entries.extend(
        restricted_paths
            .into_iter()
            .map(|(path, access)| FileSystemSandboxEntry::new(path, access)),
    );

    PermissionProfile::from_runtime_permissions(
        &file_system_policy,
        NetworkSandboxPolicy::Restricted,
    )
}

fn denied_glob_profile(workspace: &AbsolutePathBuf, pattern: String) -> PermissionProfile {
    permission_profile(
        workspace,
        vec![(
            FileSystemPath::GlobPattern { pattern },
            FileSystemAccessMode::Deny,
        )],
    )
}

async fn run_sandboxed_command(
    command: Vec<String>,
    workspace: &AbsolutePathBuf,
    permission_profile: &PermissionProfile,
) -> Output {
    spawn_command_under_sandbox(
        command,
        workspace.clone(),
        permission_profile,
        workspace,
        StdioPolicy::RedirectForShellTool,
        HashMap::new(),
    )
    .await
    .expect("spawn command under production Seatbelt policy")
    .wait_with_output()
    .await
    .expect("wait for sandboxed command")
}

async fn run_path_command(
    executable: &str,
    path: &AbsolutePathBuf,
    workspace: &AbsolutePathBuf,
    profile: &PermissionProfile,
) -> Output {
    run_sandboxed_command(
        vec![executable.to_string(), path.display().to_string()],
        workspace,
        profile,
    )
    .await
}

async fn assert_denied_read(
    path: &AbsolutePathBuf,
    workspace: &AbsolutePathBuf,
    profile: &PermissionProfile,
) {
    let output = run_path_command("/bin/cat", path, workspace, profile).await;
    assert!(
        !output.status.success(),
        "Seatbelt allowed reading {}: {output:?}",
        path.display()
    );
    assert!(output.stdout.is_empty(), "protected contents leaked");
}

fn assert_file_contents(path: &AbsolutePathBuf, contents: &str) {
    assert_eq!(fs::read_to_string(path).expect("read fixture"), contents);
}

async fn run_shell_command(
    script: &str,
    path: &AbsolutePathBuf,
    workspace: &AbsolutePathBuf,
    profile: &PermissionProfile,
) -> Output {
    run_sandboxed_command(
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            script.to_string(),
            "sh".to_string(),
            path.display().to_string(),
        ],
        workspace,
        profile,
    )
    .await
}

async fn run_after_policy_applied(
    operation: &str,
    target: &AbsolutePathBuf,
    extra_args: Vec<String>,
    workspace: &AbsolutePathBuf,
    profile: &PermissionProfile,
    setup: impl FnOnce(),
) -> Output {
    let label = target
        .as_path()
        .file_name()
        .expect("filename")
        .to_string_lossy();
    let ready = workspace.join(format!("{label}.ready.txt"));
    let proceed = workspace.join(format!("{label}.proceed.txt"));
    let script = format!(
        "printf ready > \"$2\"; while [ ! -e \"$3\" ]; do /bin/sleep 0.01; done; {operation}"
    );
    let mut command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        script,
        "sh".to_string(),
        target.display().to_string(),
        ready.display().to_string(),
        proceed.display().to_string(),
    ];
    command.extend(extra_args);
    let child = spawn_command_under_sandbox(
        command,
        workspace.clone(),
        profile,
        workspace,
        StdioPolicy::RedirectForShellTool,
        HashMap::new(),
    )
    .await
    .expect("spawn command before creating protected fixture");

    tokio::time::timeout(Duration::from_secs(5), async {
        while !ready.as_path().exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("sandboxed command did not signal policy application");
    setup();
    fs::write(&proceed, "proceed").expect("release sandboxed command");
    child
        .wait_with_output()
        .await
        .expect("wait for sandboxed command")
}

#[tokio::test]
async fn seatbelt_enforces_brace_alternation_and_escaped_deny_globs() {
    core_test_support::skip_if_sandbox!();

    let (temp_dir, workspace) = workspace_fixture();
    let dotenv = workspace.join(".env");
    let secrets = workspace.join("secrets.yml");
    let escaped = workspace.join("config?.env");
    let allowed = workspace.join("notes.txt");
    for path in [&dotenv, &secrets, &escaped] {
        fs::write(path, "protected contents").expect("write protected fixture");
    }
    fs::write(&allowed, "allowed contents").expect("write allowed fixture");

    let config = format!(
        r#"default_permissions = "test"
[permissions.test]
extends = ":read-only"
[permissions.test.filesystem]
"{workspace}" = "write"
"{workspace}/{{.env,secrets.yml}}*" = "deny"
'{workspace}/config\?.env' = "deny"
"#,
        workspace = workspace.display()
    );
    fs::write(temp_dir.path().join("config.toml"), config).expect("write permissions config");
    let profile = core_test_support::load_default_config_for_test(&temp_dir)
        .await
        .permissions
        .effective_permission_profile();

    for denied_path in [&dotenv, &secrets, &escaped] {
        assert_denied_read(denied_path, &workspace, &profile).await;
    }

    let output = run_path_command("/bin/cat", &allowed, &workspace, &profile).await;
    assert!(output.status.success(), "allowed read failed: {output:?}");
    assert_eq!(output.stdout, b"allowed contents");
}

#[tokio::test]
async fn seatbelt_blocks_renaming_ancestors_of_explicit_protected_files() {
    core_test_support::skip_if_sandbox!();

    for access in [FileSystemAccessMode::Read, FileSystemAccessMode::Deny] {
        let (_temp_dir, workspace) = workspace_fixture();
        let private = workspace.join("private");
        let protected = private.join("secret.env");
        let renamed = workspace.join("exposed");
        fs::create_dir(&private).expect("create protected directory");
        fs::write(&protected, "protected contents").expect("write protected fixture");

        let profile = permission_profile(&workspace, vec![(protected.clone().into(), access)]);
        let command = vec![
            "/bin/mv".to_string(),
            private.display().to_string(),
            renamed.display().to_string(),
        ];
        let output = run_sandboxed_command(command.clone(), &workspace, &profile).await;

        assert!(
            !output.status.success(),
            "Seatbelt allowed renaming an ancestor of a {access:?} path: {output:?}"
        );
        assert_file_contents(&protected, "protected contents");
        assert!(!renamed.as_path().exists(), "rename unexpectedly succeeded");

        let unprotected = permission_profile(&workspace, Vec::new());
        let control = run_sandboxed_command(command, &workspace, &unprotected).await;
        assert!(
            control.status.success(),
            "rename without a protected descendant should succeed: {control:?}"
        );
    }
}

#[tokio::test]
async fn seatbelt_blocks_renaming_dynamic_glob_ancestors() {
    core_test_support::skip_if_sandbox!();

    for component in [".githu?", "{.github,.gitlab}", r".githu\b", "*"] {
        let (_temp_dir, workspace) = workspace_fixture();
        let github = workspace.join(".github");
        let workflows = github.join("workflows");
        let protected = workflows.join("release.yml");
        let renamed = workspace.join("exposed");
        fs::create_dir_all(&workflows).expect("create protected directory");
        fs::write(&protected, "protected workflow").expect("write protected fixture");

        let profile = denied_glob_profile(
            &workspace,
            format!("{}/{component}/workflows/*.yml", workspace.display()),
        );
        let command = vec![
            "/bin/mv".to_string(),
            github.display().to_string(),
            renamed.display().to_string(),
        ];
        let output = run_sandboxed_command(command.clone(), &workspace, &profile).await;

        assert!(
            !output.status.success(),
            "Seatbelt allowed renaming ancestor matched by {component:?}: {output:?}"
        );
        assert_file_contents(&protected, "protected workflow");

        let unprotected = permission_profile(&workspace, Vec::new());
        let control = run_sandboxed_command(command, &workspace, &unprotected).await;
        assert!(
            control.status.success(),
            "rename without the deny glob should succeed: {control:?}"
        );
    }
}

#[tokio::test]
async fn seatbelt_blocks_renaming_protected_workspace_into_another_writable_root() {
    core_test_support::skip_if_sandbox!();

    let (_temp_dir, workspace) = workspace_fixture();
    let destination = workspace
        .parent()
        .expect("workspace has a parent")
        .join("destination");
    let moved_workspace = destination.join("moved");
    let protected = workspace.join("secret.env");
    fs::create_dir(&destination).expect("create writable destination");
    fs::write(&protected, "protected contents").expect("write protected fixture");

    let profile = permission_profile(
        &workspace,
        vec![
            (destination.clone().into(), FileSystemAccessMode::Write),
            (protected.clone().into(), FileSystemAccessMode::Read),
        ],
    );
    let output = run_sandboxed_command(
        vec![
            "/bin/mv".to_string(),
            workspace.display().to_string(),
            moved_workspace.display().to_string(),
        ],
        &workspace,
        &profile,
    )
    .await;

    assert!(
        !output.status.success(),
        "Seatbelt allowed moving the workspace into another writable root: {output:?}"
    );
    assert_file_contents(&protected, "protected contents");
    assert!(
        !moved_workspace.as_path().exists(),
        "workspace unexpectedly moved"
    );
}

#[tokio::test]
async fn seatbelt_ancestor_protection_does_not_block_unlinking_regular_files() {
    core_test_support::skip_if_sandbox!();

    let (_temp_dir, workspace) = workspace_fixture();
    let regular_file = workspace.join("regular-file");
    let hypothetical_descendant = regular_file.join("secret.env");
    fs::write(&regular_file, "replaceable").expect("write regular file");
    let profile = permission_profile(
        &workspace,
        vec![(hypothetical_descendant.into(), FileSystemAccessMode::Read)],
    );

    let output = run_path_command("/bin/rm", &regular_file, &workspace, &profile).await;

    assert!(
        output.status.success(),
        "directory-only ancestor protection blocked a regular file: {output:?}"
    );
    assert!(
        !regular_file.as_path().exists(),
        "regular file was not removed"
    );
}

#[tokio::test]
async fn seatbelt_deny_globs_block_writes_to_existing_and_new_files() {
    core_test_support::skip_if_sandbox!();

    let (_temp_dir, workspace) = workspace_fixture();
    let protected = workspace.join("secret.env");
    let allowed = workspace.join("notes.txt");
    fs::write(&protected, "original").expect("write protected fixture");
    let profile = denied_glob_profile(&workspace, format!("{}/*.env", workspace.display()));

    for (operation, script) in [
        ("overwrite", "printf changed >> \"$1\""),
        ("truncate", ": > \"$1\""),
    ] {
        let output = run_shell_command(script, &protected, &workspace, &profile).await;

        assert!(
            !output.status.success(),
            "Seatbelt allowed {operation} on a denied file: {output:?}"
        );
        assert_file_contents(&protected, "original");
    }

    let unlink = run_path_command("/bin/rm", &protected, &workspace, &profile).await;
    assert!(
        !unlink.status.success(),
        "Seatbelt unexpectedly allowed unlinking a denied glob: {unlink:?}"
    );
    assert_file_contents(&protected, "original");

    for (operation, write_script) in [
        ("overwrite", "printf changed >> \"$1\""),
        ("truncate", ": > \"$1\""),
    ] {
        let late_protected = workspace.join(format!("late-{operation}.env"));
        let output = run_after_policy_applied(
            write_script,
            &late_protected,
            Vec::new(),
            &workspace,
            &profile,
            || {
                fs::write(&late_protected, "created after policy generation")
                    .expect("create protected fixture after sandbox launch");
            },
        )
        .await;

        assert!(
            !output.status.success(),
            "Seatbelt allowed {operation} on a file created after policy generation: {output:?}"
        );
        assert_file_contents(&late_protected, "created after policy generation");
    }

    let allowed_write =
        run_shell_command("printf allowed > \"$1\"", &allowed, &workspace, &profile).await;
    assert!(
        allowed_write.status.success(),
        "unrelated write should remain allowed: {allowed_write:?}"
    );
    assert_file_contents(&allowed, "allowed");
}

#[tokio::test]
async fn seatbelt_standard_profiles_preserve_expected_filesystem_access() {
    core_test_support::skip_if_sandbox!();
    let (_temp_dir, workspace) = workspace_fixture();
    let inside = workspace.join("notes.txt");
    let outside = workspace.parent().expect("parent").join("outside.txt");
    fs::write(&inside, "original").expect("write workspace fixture");
    let profile = permission_profile(&workspace, Vec::new());
    let script = "set -e; /bin/cat \"$1\" >/dev/null; printf changed >> \"$1\"; /bin/mkdir \"$1.dir\"; /bin/mv \"$1\" \"$1.dir/moved\"; /bin/rm \"$1.dir/moved\"; /bin/rmdir \"$1.dir\"";
    let inside_output = run_shell_command(script, &inside, &workspace, &profile).await;
    assert!(
        inside_output.status.success(),
        "ordinary workspace operations failed: {inside_output:?}"
    );

    let outside_output =
        run_shell_command("printf denied > \"$1\"", &outside, &workspace, &profile).await;
    assert!(
        !outside_output.status.success(),
        "write outside the workspace unexpectedly succeeded: {outside_output:?}"
    );
    assert!(!outside.as_path().exists(), "outside file was created");
    let readable = workspace.join("readable.txt");
    fs::write(&readable, "readable contents").expect("write read-only fixture");
    let profile = PermissionProfile::read_only();
    let read = run_path_command("/bin/cat", &readable, &workspace, &profile).await;
    assert!(read.status.success(), "ordinary read failed: {read:?}");
    assert_eq!(read.stdout, b"readable contents");

    let write = run_shell_command("printf changed > \"$1\"", &readable, &workspace, &profile).await;
    assert!(
        !write.status.success(),
        "read-only write succeeded: {write:?}"
    );
    assert_file_contents(&readable, "readable contents");
}

#[tokio::test]
async fn seatbelt_blocks_protected_reads_and_writes_through_symlink_aliases() {
    core_test_support::skip_if_sandbox!();

    let (_temp_dir, workspace) = workspace_fixture();
    let private = workspace.join("private");
    let alias = workspace.join("alias");
    let protected = private.join("secret.env");
    let alias_path = alias.join("secret.env");
    fs::create_dir(&private).expect("create protected directory");
    fs::write(&protected, "protected contents").expect("write protected fixture");
    symlink(private.as_path(), alias.as_path()).expect("create directory symlink");

    for (restricted_path, accessed_path) in [
        (FileSystemPath::from(protected.clone()), alias_path.clone()),
        (
            FileSystemPath::GlobPattern {
                pattern: format!("{}/*.env", private.display()),
            },
            alias_path.clone(),
        ),
        (
            FileSystemPath::GlobPattern {
                pattern: format!("{}/*.env", alias.display()),
            },
            protected.clone(),
        ),
    ] {
        let profile = permission_profile(
            &workspace,
            vec![(restricted_path, FileSystemAccessMode::Deny)],
        );
        assert_denied_read(&accessed_path, &workspace, &profile).await;

        let write = run_shell_command(
            "printf changed > \"$1\"",
            &accessed_path,
            &workspace,
            &profile,
        )
        .await;
        assert!(
            !write.status.success(),
            "Seatbelt allowed writing through protected symlink alias: {write:?}"
        );
        assert_file_contents(&protected, "protected contents");
    }
}

#[tokio::test]
async fn seatbelt_globstar_protects_files_and_ancestors_at_every_depth() {
    core_test_support::skip_if_sandbox!();

    let (_temp_dir, workspace) = workspace_fixture();
    let first = workspace.join("first");
    let second = first.join("second");
    fs::create_dir_all(&second).expect("create nested fixture directories");
    let protected_paths = [
        workspace.join("root.env"),
        first.join("first.env"),
        second.join("second.env"),
    ];
    for path in &protected_paths {
        fs::write(path, "protected contents").expect("write protected fixture");
    }
    let profile = denied_glob_profile(&workspace, format!("{}/**/*.env", workspace.display()));

    for path in &protected_paths {
        assert_denied_read(path, &workspace, &profile).await;
    }

    for ancestor in [&first, &second] {
        let destination = ancestor.with_extension("renamed");
        let output = run_sandboxed_command(
            vec![
                "/bin/mv".to_string(),
                ancestor.display().to_string(),
                destination.display().to_string(),
            ],
            &workspace,
            &profile,
        )
        .await;
        assert!(
            !output.status.success(),
            "globstar allowed renaming protected ancestor {}: {output:?}",
            ancestor.display()
        );
    }
}

#[tokio::test]
async fn seatbelt_brace_alternatives_with_slashes_protect_each_branch() {
    core_test_support::skip_if_sandbox!();

    let (_temp_dir, workspace) = workspace_fixture();
    let nested = workspace.join("private/nested");
    let other = workspace.join("other");
    let allowed = workspace.join("notes.txt");
    fs::create_dir_all(&nested).expect("create nested protected directory");
    fs::create_dir(&other).expect("create alternate protected directory");
    let protected_paths = [nested.join("secret.env"), other.join("secret.env")];
    for path in &protected_paths {
        fs::write(path, "protected contents").expect("write protected fixture");
    }
    fs::write(&allowed, "allowed contents").expect("write allowed fixture");
    let profile = denied_glob_profile(
        &workspace,
        format!("{}/{{private/nested,other}}/*.env", workspace.display()),
    );

    for path in &protected_paths {
        assert_denied_read(path, &workspace, &profile).await;
    }

    for ancestor in [&workspace.join("private"), &nested, &other] {
        let destination = ancestor.with_extension("renamed");
        let output = run_sandboxed_command(
            vec![
                "/bin/mv".to_string(),
                ancestor.display().to_string(),
                destination.display().to_string(),
            ],
            &workspace,
            &profile,
        )
        .await;
        assert!(
            !output.status.success(),
            "brace alternative allowed renaming {}: {output:?}",
            ancestor.display()
        );
    }

    let output = run_path_command("/bin/cat", &allowed, &workspace, &profile).await;
    assert!(
        output.status.success(),
        "brace alternatives produced an invalid or overly broad policy: {output:?}"
    );
    assert_eq!(output.stdout, b"allowed contents");
}

#[tokio::test]
async fn seatbelt_protects_directory_trees_created_after_policy_application() {
    core_test_support::skip_if_sandbox!();

    for operation in ["read", "rename"] {
        let (_temp_dir, workspace) = workspace_fixture();
        let late_directory = workspace.join("late/deep");
        let protected = late_directory.join("secret.env");
        let destination = workspace.join("exposed");
        let profile = denied_glob_profile(&workspace, format!("{}/**/*.env", workspace.display()));
        let operation_script = match operation {
            "read" => "/bin/cat \"$1\"",
            "rename" => "/bin/mv \"$4\" \"$5\"",
            _ => unreachable!("unknown operation"),
        };
        let output = run_after_policy_applied(
            operation_script,
            &protected,
            vec![
                workspace.join("late").display().to_string(),
                destination.display().to_string(),
            ],
            &workspace,
            &profile,
            || {
                fs::create_dir_all(&late_directory)
                    .expect("create protected directory after policy");
                fs::write(&protected, "protected contents").expect("write protected fixture");
            },
        )
        .await;

        assert!(
            !output.status.success(),
            "Seatbelt allowed {operation} after protected directory appeared: {output:?}"
        );
        assert_file_contents(&protected, "protected contents");
    }
}

#[tokio::test]
async fn seatbelt_blocks_atomic_rename_exchange_and_destination_replacement() {
    core_test_support::skip_if_sandbox!();

    for (operation, flags) in [("exchange", libc::RENAME_SWAP), ("replace", 0)] {
        let (_temp_dir, workspace) = workspace_fixture();
        let private = workspace.join("private");
        let destination = workspace.join("destination");
        let protected = private.join("secret.env");
        fs::create_dir(&private).expect("create protected directory");
        fs::create_dir(&destination).expect("create replacement directory");
        fs::write(&protected, "protected contents").expect("write protected fixture");
        let profile = permission_profile(
            &workspace,
            vec![(protected.clone().into(), FileSystemAccessMode::Read)],
        );
        let script = "import ctypes, os, sys; libc = ctypes.CDLL(None, use_errno=True); result = libc.renameatx_np(-2, os.fsencode(sys.argv[1]), -2, os.fsencode(sys.argv[2]), int(sys.argv[3])); sys.exit(0 if result == 0 else ctypes.get_errno())";
        let command = vec![
            "/usr/bin/python3".to_string(),
            "-c".to_string(),
            script.to_string(),
            private.display().to_string(),
            destination.display().to_string(),
            flags.to_string(),
        ];
        let output = run_sandboxed_command(command.clone(), &workspace, &profile).await;

        assert!(
            !output.status.success(),
            "Seatbelt allowed atomic {operation} of a protected ancestor: {output:?}"
        );
        assert_file_contents(&protected, "protected contents");

        let unprotected = permission_profile(&workspace, Vec::new());
        let control = run_sandboxed_command(command, &workspace, &unprotected).await;
        assert!(
            control.status.success(),
            "unprotected atomic {operation} should succeed: {control:?}"
        );
    }
}

#[tokio::test]
async fn seatbelt_blocks_protected_ancestor_moves_across_writable_roots() {
    core_test_support::skip_if_sandbox!();

    for destination_is_nested in [false, true] {
        for use_glob in [false, true] {
            let (_temp_dir, workspace) = workspace_fixture();
            let private = workspace.join("private");
            let protected = private.join("secret.env");
            let destination = if destination_is_nested {
                workspace.join("destination")
            } else {
                workspace
                    .parent()
                    .expect("workspace has a parent")
                    .join("destination")
            };
            let moved = destination.join("moved");
            fs::create_dir(&private).expect("create protected directory");
            fs::create_dir(&destination).expect("create writable destination");
            fs::write(&protected, "protected contents").expect("write protected fixture");
            let protected_path = if use_glob {
                FileSystemPath::GlobPattern {
                    pattern: format!("{}/*.env", private.display()),
                }
            } else {
                protected.clone().into()
            };
            let profile = permission_profile(
                &workspace,
                vec![
                    (destination.clone().into(), FileSystemAccessMode::Write),
                    (protected_path, FileSystemAccessMode::Deny),
                ],
            );

            let output = run_sandboxed_command(
                vec![
                    "/bin/mv".to_string(),
                    private.display().to_string(),
                    moved.display().to_string(),
                ],
                &workspace,
                &profile,
            )
            .await;
            assert!(
                !output.status.success(),
                "protected ancestor crossed writable roots: nested={destination_is_nested}, glob={use_glob}, output={output:?}"
            );
            assert_file_contents(&protected, "protected contents");
            assert!(!moved.as_path().exists(), "protected ancestor was moved");
        }
    }
}

#[tokio::test]
async fn seatbelt_blocks_creating_files_matched_by_denied_globs() {
    core_test_support::skip_if_sandbox!();

    let (_temp_dir, workspace) = workspace_fixture();
    let nested = workspace.join("nested");
    fs::create_dir(&nested).expect("create nested directory");
    let profile = denied_glob_profile(&workspace, format!("{}/**/*.env", workspace.display()));

    for path in [workspace.join("created.env"), nested.join("created.env")] {
        let output =
            run_shell_command("printf created > \"$1\"", &path, &workspace, &profile).await;
        assert!(
            !output.status.success(),
            "Seatbelt allowed creation at a denied path: {output:?}"
        );
        assert!(!path.as_path().exists(), "denied file was created");
    }

    let allowed = nested.join("created.txt");
    let output = run_path_command("/usr/bin/touch", &allowed, &workspace, &profile).await;
    assert!(
        output.status.success(),
        "unrelated file creation should succeed: {output:?}"
    );
    assert!(allowed.as_path().exists(), "unrelated file was not created");
}
