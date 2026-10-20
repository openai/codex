//! Verify bubblewrap arguments enforce filesystem, namespace, and network policies.

use super::*;

use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::FileSystemAccessMode;
use codex_protocol::protocol::FileSystemPath;
use codex_protocol::protocol::FileSystemSandboxEntry;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use codex_protocol::protocol::FileSystemSpecialPath;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn default_unreadable_glob_scan_has_no_depth_cap() {
    assert_eq!(BwrapOptions::default().glob_scan_max_depth, None);
}

#[test]
fn pid_inheritance_and_legacy_proc_modes_read_only() {
    assert_pid_namespace_args(&PermissionProfile::read_only().file_system_sandbox_policy());
}

#[test]
fn pid_inheritance_and_legacy_proc_modes_full_filesystem() {
    assert_pid_namespace_args(&FileSystemSandboxPolicy::unrestricted());
}

fn assert_pid_namespace_args(policy: &FileSystemSandboxPolicy) {
    for (mount_proc, inherit_pid_namespace) in [(true, false), (false, false), (false, true)] {
        let args = create_bwrap_command_args(
            vec!["/bin/true".to_string()],
            policy,
            Path::new("/"),
            Path::new("/"),
            BwrapOptions {
                mount_proc,
                inherit_pid_namespace,
                network_mode: BwrapNetworkMode::Isolated,
                ..Default::default()
            },
        )
        .expect("create bwrap args")
        .args;

        assert_eq!(
            args.iter().any(|arg| arg == "--unshare-pid"),
            !inherit_pid_namespace
        );
        assert_eq!(args.iter().any(|arg| arg == "--proc"), mount_proc);
        assert!(args.iter().any(|arg| arg == "--unshare-user"));
        assert!(args.iter().any(|arg| arg == "--unshare-ipc"));
        assert!(args.iter().any(|arg| arg == "--unshare-net"));
        assert!(args.windows(2).any(|args| args == ["--cap-drop", "ALL"]));
    }
}

fn unreadable_glob_entry(pattern: String) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry {
        path: FileSystemPath::GlobPattern { pattern },
        access: FileSystemAccessMode::Deny,
        missing_path_behavior: None,
    }
}

fn default_policy_with_unreadable_glob(pattern: String) -> FileSystemSandboxPolicy {
    let mut policy = FileSystemSandboxPolicy::default();
    policy.entries.push(unreadable_glob_entry(pattern));
    policy
}

#[test]
fn full_disk_write_full_network_returns_unwrapped_command() {
    let command = vec!["/bin/true".to_string()];
    let args = create_bwrap_command_args(
        command.clone(),
        &FileSystemSandboxPolicy::unrestricted(),
        Path::new("/"),
        Path::new("/"),
        BwrapOptions {
            mount_proc: true,
            network_mode: BwrapNetworkMode::FullAccess,
            ..Default::default()
        },
    )
    .expect("create bwrap args");

    assert_eq!(args.args, command);
}

#[test]
fn full_disk_write_proxy_only_keeps_full_filesystem_but_unshares_network() {
    let command = vec!["/bin/true".to_string()];
    let args = create_bwrap_command_args(
        command,
        &FileSystemSandboxPolicy::unrestricted(),
        Path::new("/"),
        Path::new("/"),
        BwrapOptions {
            mount_proc: true,
            network_mode: BwrapNetworkMode::ProxyOnly,
            ..Default::default()
        },
    )
    .expect("create bwrap args");

    assert_eq!(
        args.args,
        vec![
            "--new-session".to_string(),
            "--die-with-parent".to_string(),
            "--bind".to_string(),
            "/".to_string(),
            "/".to_string(),
            "--dev".to_string(),
            "/dev".to_string(),
            "--bind-try".to_string(),
            "/dev/shm".to_string(),
            "/dev/shm".to_string(),
            "--unshare-user".to_string(),
            "--unshare-ipc".to_string(),
            "--unshare-pid".to_string(),
            "--unshare-net".to_string(),
            "--proc".to_string(),
            "/proc".to_string(),
            "--cap-drop".to_string(),
            "ALL".to_string(),
            "--".to_string(),
            "/bin/true".to_string(),
        ]
    );
}

#[test]
fn full_disk_write_with_unreadable_glob_still_wraps_and_masks_match() {
    if !ripgrep_available() {
        return;
    }

    let temp_dir = TempDir::new().expect("temp dir");
    let root_env = temp_dir.path().join(".env");
    std::fs::write(&root_env, "secret").expect("write env");
    let root_alias = temp_dir.path().join("root-alias");
    std::os::unix::fs::symlink("/", &root_alias).expect("create root symlink");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        unreadable_glob_entry(format!("{}/**/*.env", temp_dir.path().display())),
        FileSystemSandboxEntry::new(
            AbsolutePathBuf::from_absolute_path(root_alias)
                .expect("absolute root alias")
                .into(),
            FileSystemAccessMode::Write,
        ),
        FileSystemSandboxEntry::new(
            AbsolutePathBuf::try_from("/dev/zero")
                .expect("device path")
                .into(),
            FileSystemAccessMode::Deny,
        ),
    ]);
    let command = vec!["/bin/true".to_string()];

    let args = create_bwrap_command_args(
        command.clone(),
        &policy,
        temp_dir.path(),
        temp_dir.path(),
        BwrapOptions::default(),
    )
    .expect("create bwrap args");

    assert_ne!(
        args.args, command,
        "full-write policy with unreadable globs must still use bwrap"
    );
    assert_file_masked(&args.args, &root_env);
    assert_file_masked(&args.args, Path::new("/dev/zero"));
    assert_eq!(
        args.args
            .windows(3)
            .filter(|window| *window == ["--bind", "/", "/"])
            .count(),
        1,
    );
    let writable_root = args
        .args
        .windows(3)
        .position(|window| window == ["--bind", "/", "/"])
        .expect("writable root");
    let devices = args
        .args
        .windows(5)
        .rposition(|window| window == ["--dev", "/dev", "--bind-try", "/dev/shm", "/dev/shm"])
        .expect("minimal device tree with shared memory");
    let device_mask = args
        .args
        .windows(3)
        .position(|window| window[0] == "--ro-bind-data" && window[2] == "/dev/zero")
        .expect("explicit device deny mask");
    assert!(writable_root < devices && devices < device_mask);
}

#[cfg(unix)]
#[test]
fn restricted_policy_chdirs_to_canonical_command_cwd() {
    let temp_dir = TempDir::new().expect("temp dir");
    let real_root = temp_dir.path().join("real");
    let real_subdir = real_root.join("subdir");
    let link_root = temp_dir.path().join("link");
    std::fs::create_dir_all(&real_subdir).expect("create real subdir");
    std::os::unix::fs::symlink(&real_root, &link_root).expect("create symlinked root");

    let sandbox_policy_cwd = AbsolutePathBuf::from_absolute_path(&link_root)
        .expect("absolute symlinked root")
        .to_path_buf();
    let command_cwd = link_root.join("subdir");
    let canonical_command_cwd = real_subdir
        .canonicalize()
        .expect("canonicalize command cwd");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
    ]);

    let args = create_bwrap_command_args(
        vec!["/bin/true".to_string()],
        &policy,
        sandbox_policy_cwd.as_path(),
        &command_cwd,
        BwrapOptions::default(),
    )
    .expect("create bwrap args");
    let canonical_sandbox_cwd = path_to_string(
        &real_root
            .canonicalize()
            .expect("canonicalize sandbox policy cwd"),
    );
    let canonical_command_cwd = path_to_string(&canonical_command_cwd);
    let link_sandbox_cwd = path_to_string(&link_root);
    let link_command_cwd = path_to_string(&command_cwd);

    assert!(
        args.args
            .windows(2)
            .any(|window| { window == ["--chdir", canonical_command_cwd.as_str()] })
    );
    assert!(args.args.windows(3).any(|window| {
        window
            == [
                "--ro-bind",
                canonical_sandbox_cwd.as_str(),
                canonical_sandbox_cwd.as_str(),
            ]
    }));
    assert!(
        !args
            .args
            .windows(2)
            .any(|window| { window == ["--chdir", link_command_cwd.as_str()] })
    );
    assert!(!args.args.windows(3).any(|window| {
        window
            == [
                "--ro-bind",
                link_sandbox_cwd.as_str(),
                link_sandbox_cwd.as_str(),
            ]
    }));
}

#[cfg(unix)]
#[test]
fn symlinked_writable_roots_bind_real_target_and_remap_carveouts() {
    let temp_dir = TempDir::new().expect("temp dir");
    let real_root = temp_dir.path().join("real");
    let link_root = temp_dir.path().join("link");
    let blocked = real_root.join("blocked");
    std::fs::create_dir_all(&blocked).expect("create blocked dir");
    std::os::unix::fs::symlink(&real_root, &link_root).expect("create symlinked root");

    let link_root =
        AbsolutePathBuf::from_absolute_path(&link_root).expect("absolute symlinked root");
    let link_blocked = link_root.join("blocked");
    let real_root_str = path_to_string(&real_root);
    let real_blocked_str = path_to_string(&blocked);
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: link_root.into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: link_blocked.into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");

    assert!(
        args.args
            .windows(3)
            .any(|window| { window == ["--bind", real_root_str.as_str(), real_root_str.as_str()] })
    );
    assert!(args.args.windows(6).any(|window| {
        window
            == [
                "--perms",
                "000",
                "--tmpfs",
                real_blocked_str.as_str(),
                "--remount-ro",
                real_blocked_str.as_str(),
            ]
    }));
}

#[cfg(unix)]
#[test]
fn writable_roots_under_symlinked_ancestors_bind_real_target() {
    let temp_dir = TempDir::new().expect("temp dir");
    let logical_home = temp_dir.path().join("home");
    let real_codex = temp_dir.path().join("real-codex");
    let logical_codex = logical_home.join(".codex");
    let real_memories = real_codex.join("memories");
    let logical_memories = logical_codex.join("memories");
    std::fs::create_dir_all(&logical_home).expect("create logical home");
    std::fs::create_dir_all(&real_memories).expect("create memories dir");
    std::os::unix::fs::symlink(&real_codex, &logical_codex).expect("create symlinked codex home");

    let logical_memories_root =
        AbsolutePathBuf::from_absolute_path(&logical_memories).expect("absolute memories");
    let real_memories_str = path_to_string(&real_memories);
    let logical_memories_str = path_to_string(&logical_memories);
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: logical_memories_root.into(),
        access: FileSystemAccessMode::Write,
        missing_path_behavior: None,
    }]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");

    assert!(args.args.windows(3).any(|window| {
        window
            == [
                "--bind",
                real_memories_str.as_str(),
                real_memories_str.as_str(),
            ]
    }));
    assert!(!args.args.windows(3).any(|window| {
        window
            == [
                "--bind",
                logical_memories_str.as_str(),
                logical_memories_str.as_str(),
            ]
    }));
}

#[cfg(unix)]
#[test]
fn protected_symlinked_directory_subpaths_fail_closed() {
    let temp_dir = TempDir::new().expect("temp dir");
    let root = temp_dir.path().join("root");
    let agents_target = root.join("agents-target");
    let agents_link = root.join(".agents");
    std::fs::create_dir_all(&agents_target).expect("create agents target");
    std::os::unix::fs::symlink(&agents_target, &agents_link).expect("create symlinked .agents");

    let root = AbsolutePathBuf::from_absolute_path(&root).expect("absolute root");
    let agents_link_str = path_to_string(&agents_link);
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: root.into(),
        access: FileSystemAccessMode::Write,
        missing_path_behavior: None,
    }]);

    let err = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect_err("protected symlinked subpath should fail closed");
    let message = err.to_string();

    assert!(
        message.contains("cannot enforce sandbox read-only path"),
        "{message}"
    );
    assert!(message.contains(&agents_link_str), "{message}");
}

#[cfg(unix)]
#[test]
fn symlinked_writable_roots_nested_symlink_escape_paths_fail_closed() {
    let temp_dir = TempDir::new().expect("temp dir");
    let real_root = temp_dir.path().join("real");
    let link_root = temp_dir.path().join("link");
    let outside = temp_dir.path().join("outside-private");
    let linked_private = real_root.join("linked-private");
    std::fs::create_dir_all(&real_root).expect("create real root");
    std::fs::create_dir_all(&outside).expect("create outside dir");
    std::os::unix::fs::symlink(&real_root, &link_root).expect("create symlinked root");
    std::os::unix::fs::symlink(&outside, &linked_private).expect("create nested escape symlink");

    let link_root =
        AbsolutePathBuf::from_absolute_path(&link_root).expect("absolute symlinked root");
    let link_private = link_root.join("linked-private");
    let real_linked_private_str = path_to_string(&linked_private);
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: link_root.into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: link_private.into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    let err = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect_err("deny-read path crossing writable symlink should fail closed");
    let message = err.to_string();

    assert!(
        message.contains("cannot enforce sandbox deny-read path"),
        "{message}"
    );
    assert!(message.contains(&real_linked_private_str), "{message}");
}

#[test]
fn registry_protection_precedes_denied_parent_mask() {
    let registry_root = synthetic_mount_registry_root();
    let temp_root = registry_root.parent().expect("registry parent");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: AbsolutePathBuf::try_from(temp_root)
                .expect("absolute temp root")
                .into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    let args = create_filesystem_args(&policy, Path::new("/"), BwrapOptions::default())
        .expect("filesystem args");
    let registry_path = path_to_string(&registry_root);
    let temp_path = path_to_string(temp_root);
    let registry_bind = args
        .args
        .windows(3)
        .position(|args| args == ["--ro-bind", registry_path.as_str(), registry_path.as_str()]);
    let temp_mask = args
        .args
        .windows(2)
        .position(|args| args == ["--tmpfs", temp_path.as_str()]);
    assert!(
        registry_bind.expect("registry read-only bind") < temp_mask.expect("temp deny mask"),
        "registry protection must not reopen denied temp paths"
    );
}

#[test]
fn missing_read_only_subpath_uses_empty_file_bind_data() {
    let temp_dir = TempDir::new().expect("temp dir");
    let workspace = temp_dir.path().join("workspace");
    let blocked = workspace.join("blocked");
    let second_blocked = workspace.join("second-blocked");
    std::fs::create_dir_all(&workspace).expect("create workspace");

    let workspace_root =
        AbsolutePathBuf::from_absolute_path(&workspace).expect("absolute workspace");
    let blocked_root = AbsolutePathBuf::from_absolute_path(&blocked).expect("absolute blocked");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: workspace_root.into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: blocked_root.into(),
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: AbsolutePathBuf::from_absolute_path(&second_blocked)
                .expect("absolute second blocked")
                .into(),
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
    ]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");

    assert_empty_file_bound_without_perms(&args.args, &blocked);
    assert_empty_file_bound_without_perms(&args.args, &second_blocked);
    assert_empty_directory_mounted_read_only(&args.args, &workspace.join(".git"));
    assert_empty_directory_mounted_read_only(&args.args, &workspace.join(".agents"));
    assert_empty_directory_mounted_read_only(&args.args, &workspace.join(".codex"));
    assert_eq!(args.preserved_files.len(), 2);
    assert_bind_data_uses_distinct_preserved_fds(&args);
    assert_eq!(
        synthetic_mount_target_paths(&args),
        vec![
            blocked.clone(),
            second_blocked.clone(),
            workspace.join(".git"),
            workspace.join(".agents"),
            workspace.join(".codex"),
            workspace.join(".aws"),
        ]
    );
    assert!(
        !blocked.exists() && !second_blocked.exists(),
        "missing path mask should not materialize host-side metadata paths at arg construction time",
    );
}

#[test]
fn transient_empty_preserved_file_uses_empty_file_bind_data() {
    let temp_dir = TempDir::new().expect("temp dir");
    let workspace = temp_dir.path().join("workspace");
    let dot_git = workspace.join(".git");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    File::create(&dot_git).expect("create empty .git file");

    let workspace_root =
        AbsolutePathBuf::from_absolute_path(&workspace).expect("absolute workspace");
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: workspace_root.into(),
        access: FileSystemAccessMode::Write,
        missing_path_behavior: None,
    }]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");
    let dot_git_str = path_to_string(&dot_git);

    assert_empty_file_bound_without_perms(&args.args, &dot_git);
    assert_empty_directory_mounted_read_only(&args.args, &workspace.join(".agents"));
    assert_empty_directory_mounted_read_only(&args.args, &workspace.join(".codex"));
    assert_eq!(
        synthetic_mount_target_paths(&args),
        vec![
            dot_git.clone(),
            workspace.join(".agents"),
            workspace.join(".codex"),
            workspace.join(".aws"),
        ]
    );
    assert!(
        !args
            .args
            .windows(3)
            .any(|window| window == ["--ro-bind", dot_git_str.as_str(), dot_git_str.as_str()]),
        "transient empty preserved file should not be treated as a stable bind source",
    );
    let metadata = std::fs::symlink_metadata(&dot_git).expect("stat .git");
    assert!(
        !args.synthetic_mount_targets[0].should_remove_after_bwrap(&metadata),
        "pre-existing empty preserved files must not be cleaned up as synthetic targets",
    );
}

#[test]
fn missing_child_git_under_parent_repo_is_mounted_read_only() {
    let temp_dir = TempDir::new().expect("temp dir");
    let repo = temp_dir.path().join("repo");
    let workspace = repo.join("workspace");
    let dot_git = workspace.join(".git");
    std::fs::create_dir_all(repo.join(".git")).expect("create parent .git");
    std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
    std::fs::create_dir_all(&workspace).expect("create workspace");

    let workspace_root =
        AbsolutePathBuf::from_absolute_path(&workspace).expect("absolute workspace");
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: workspace_root.into(),
        access: FileSystemAccessMode::Write,
        missing_path_behavior: None,
    }]);

    let args = create_filesystem_args(&policy, &workspace, BwrapOptions::default())
        .expect("filesystem args");
    assert_empty_directory_mounted_read_only(&args.args, &dot_git);
    assert_empty_directory_mounted_read_only(&args.args, &workspace.join(".agents"));
    assert_empty_directory_mounted_read_only(&args.args, &workspace.join(".codex"));
    assert_eq!(
        synthetic_mount_target_paths(&args),
        vec![
            workspace.join(".codex"),
            dot_git,
            workspace.join(".agents"),
            workspace.join(".aws"),
        ],
    );
}

#[cfg(unix)]
#[test]
fn symlinked_missing_child_git_under_parent_repo_mounts_effective_path_read_only() {
    let temp_dir = TempDir::new().expect("temp dir");
    let repo = temp_dir.path().join("repo");
    let workspace = repo.join("workspace");
    let link_repo = temp_dir.path().join("link-repo");
    let link_workspace = link_repo.join("workspace");
    let dot_git = workspace.join(".git");
    std::fs::create_dir_all(repo.join(".git")).expect("create parent .git");
    std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    std::os::unix::fs::symlink(&repo, &link_repo).expect("create symlinked repo");

    let link_workspace_root =
        AbsolutePathBuf::from_absolute_path(&link_workspace).expect("absolute symlinked workspace");
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: link_workspace_root.into(),
        access: FileSystemAccessMode::Write,
        missing_path_behavior: None,
    }]);

    let args = create_filesystem_args(&policy, &link_workspace, BwrapOptions::default())
        .expect("filesystem args");
    assert_empty_directory_mounted_read_only(&args.args, &dot_git);
    assert_empty_directory_mounted_read_only(&args.args, &workspace.join(".agents"));
    assert_empty_directory_mounted_read_only(&args.args, &workspace.join(".codex"));
    assert_eq!(
        synthetic_mount_target_paths(&args),
        vec![
            workspace.join(".codex"),
            dot_git,
            workspace.join(".agents"),
            workspace.join(".aws"),
        ],
    );
}

#[test]
fn ignores_missing_writable_roots() {
    let temp_dir = TempDir::new().expect("temp dir");
    let existing_root = temp_dir.path().join("existing");
    let missing_root = temp_dir.path().join("missing");
    std::fs::create_dir(&existing_root).expect("create existing root");

    let policy = FileSystemSandboxPolicy::workspace_write(
        &[
            AbsolutePathBuf::try_from(existing_root.as_path()).expect("absolute existing root"),
            AbsolutePathBuf::try_from(missing_root.as_path()).expect("absolute missing root"),
        ],
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    );

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");
    let existing_root = path_to_string(&existing_root);
    let missing_root = path_to_string(&missing_root);

    assert!(
        args.args
            .windows(3)
            .any(|window| { window == ["--bind", existing_root.as_str(), existing_root.as_str()] }),
        "existing writable root should be rebound writable",
    );
    assert!(
        !args.args.iter().any(|arg| arg == &missing_root),
        "missing writable root should be skipped",
    );
}

#[test]
fn missing_project_root_metadata_carveouts_use_metadata_path_masks() {
    let temp_dir = TempDir::new().expect("temp dir");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(Some(".git".into())),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(Some(".agents".into())),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(Some(".codex".into())),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
    ]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");
    let dot_git = path_to_string(&temp_dir.path().join(".git"));
    let dot_agents = path_to_string(&temp_dir.path().join(".agents"));
    let dot_codex = path_to_string(&temp_dir.path().join(".codex"));

    assert_empty_directory_mounted_read_only(&args.args, Path::new(&dot_git));
    assert_empty_directory_mounted_read_only(&args.args, Path::new(&dot_agents));
    assert_empty_directory_mounted_read_only(&args.args, Path::new(&dot_codex));
    assert!(args.preserved_files.is_empty());
    let synthetic_targets = synthetic_mount_target_paths(&args);
    assert!(synthetic_targets.contains(&PathBuf::from(&dot_git)));
    assert!(synthetic_targets.contains(&PathBuf::from(&dot_agents)));
    assert!(synthetic_targets.contains(&PathBuf::from(&dot_codex)));
}

#[test]
fn missing_user_project_root_subpath_rules_are_still_enforced() {
    let temp_dir = TempDir::new().expect("temp dir");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(Some(".vscode".into())),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(Some(".secrets".into())),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");
    let dot_vscode = path_to_string(&temp_dir.path().join(".vscode"));
    let dot_secrets = path_to_string(&temp_dir.path().join(".secrets"));

    assert_empty_file_bound_without_perms(&args.args, Path::new(&dot_vscode));
    assert_empty_file_bound_without_perms(&args.args, Path::new(&dot_secrets));
}

#[test]
fn mounts_dev_before_writable_dev_binds() {
    let sandbox_policy = FileSystemSandboxPolicy::workspace_write(
        &[AbsolutePathBuf::try_from(Path::new("/dev")).expect("/dev path")],
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    );

    let args = create_filesystem_args(&sandbox_policy, Path::new("/"), BwrapOptions::default())
        .expect("bwrap fs args");
    assert!(args.preserved_files.is_empty());
    assert_eq!(
        synthetic_mount_target_paths(&args),
        vec![
            PathBuf::from("/.aws"),
            PathBuf::from("/.git"),
            PathBuf::from("/.agents"),
            PathBuf::from("/.codex"),
            PathBuf::from("/dev/.git"),
            PathBuf::from("/dev/.agents"),
            PathBuf::from("/dev/.codex"),
            PathBuf::from("/dev/.aws"),
        ]
    );
    let dev_mounts = args
        .args
        .windows(2)
        .enumerate()
        .filter_map(|(index, args)| (args == ["--dev", "/dev"]).then_some(index))
        .collect::<Vec<_>>();
    let [initial_dev_mount, restored_dev_mount] = dev_mounts.as_slice() else {
        panic!("expected exactly two /dev mounts, got {dev_mounts:?}");
    };
    let root_bind = args
        .args
        .windows(3)
        .position(|args| args == ["--bind", "/", "/"])
        .expect("root bind");
    let dev_bind = args
        .args
        .windows(3)
        .position(|args| args == ["--bind", "/dev", "/dev"])
        .expect("/dev bind");
    assert!(*initial_dev_mount < root_bind && root_bind < *restored_dev_mount);
    assert!(*restored_dev_mount < dev_bind);
}

#[test]
fn non_utf8_daemon_socket_mask_path_is_rejected() {
    let daemon_directory = PathBuf::from(OsString::from_vec(b"/tmp/codex-daemon-\xff".to_vec()));
    assert!(
        append_daemon_socket_masks(
            &mut Vec::new(),
            Path::new("/"),
            &BTreeSet::from([daemon_directory]),
        )
        .is_err()
    );
}

#[test]
fn read_only_tmp_carveout_keeps_daemon_sockets_masked() {
    let policy = FileSystemSandboxPolicy::restricted(
        [
            ("/", FileSystemAccessMode::Write),
            ("/tmp", FileSystemAccessMode::Read),
        ]
        .into_iter()
        .map(|(path, access)| {
            FileSystemSandboxEntry::new(
                AbsolutePathBuf::from_absolute_path(path).unwrap().into(),
                access,
            )
        })
        .collect(),
    );
    let args = create_filesystem_args(&policy, Path::new("/"), BwrapOptions::default())
        .expect("filesystem args");
    let tmp_bind = args
        .args
        .windows(3)
        .position(|window| window == ["--ro-bind", "/tmp", "/tmp"])
        .expect("read-only tmp bind");
    let daemon_directory = path_to_string(&codex_uds::shared_daemon_socket_directory().unwrap());
    assert_eq!(
        &args.args[tmp_bind + 3..tmp_bind + 9],
        [
            "--perms",
            "000",
            "--tmpfs",
            &daemon_directory,
            "--remount-ro",
            &daemon_directory
        ],
    );
}

#[test]
fn restricted_read_only_uses_scoped_read_roots_instead_of_erroring() {
    let temp_dir = TempDir::new().expect("temp dir");
    let readable_root = temp_dir.path().join("readable");
    std::fs::create_dir(&readable_root).expect("create readable root");

    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: AbsolutePathBuf::try_from(readable_root.as_path())
            .expect("absolute readable root")
            .into(),
        access: FileSystemAccessMode::Read,
        missing_path_behavior: None,
    }]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");

    assert_eq!(args.args[0..4], ["--tmpfs", "/", "--dev", "/dev"]);

    let readable_root_str = path_to_string(&readable_root);
    assert!(args.args.windows(3).any(|window| {
        window
            == [
                "--ro-bind",
                readable_root_str.as_str(),
                readable_root_str.as_str(),
            ]
    }));
}

#[test]
fn restricted_read_only_with_platform_defaults_includes_usr_when_present() {
    let temp_dir = TempDir::new().expect("temp dir");
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::Minimal,
        },
        access: FileSystemAccessMode::Read,
        missing_path_behavior: None,
    }]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");

    assert!(
        args.args
            .starts_with(&["--tmpfs".to_string(), "/".to_string()])
    );

    if Path::new("/usr").exists() {
        assert!(
            args.args
                .windows(3)
                .any(|window| window == ["--ro-bind", "/usr", "/usr"])
        );
    }
}

#[test]
fn split_policy_reapplies_unreadable_carveouts_after_writable_binds() {
    let temp_dir = TempDir::new().expect("temp dir");
    let writable_root = temp_dir.path().join("workspace");
    let blocked = writable_root.join("blocked");
    std::fs::create_dir_all(&blocked).expect("create blocked dir");
    let writable_root =
        AbsolutePathBuf::from_absolute_path(&writable_root).expect("absolute writable root");
    let blocked = AbsolutePathBuf::from_absolute_path(&blocked).expect("absolute blocked dir");
    let writable_root_str = path_to_string(writable_root.as_path());
    let blocked_str = path_to_string(blocked.as_path());
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: writable_root.into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: blocked.into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");

    assert!(args.args.windows(3).any(|window| {
        window
            == [
                "--bind",
                writable_root_str.as_str(),
                writable_root_str.as_str(),
            ]
    }));
    let blocked_mask_index = args
        .args
        .windows(6)
        .position(|window| {
            window
                == [
                    "--perms",
                    "000",
                    "--tmpfs",
                    blocked_str.as_str(),
                    "--remount-ro",
                    blocked_str.as_str(),
                ]
        })
        .expect("blocked directory should be remounted unreadable");

    let writable_root_bind_index = args
        .args
        .windows(3)
        .position(|window| {
            window
                == [
                    "--bind",
                    writable_root_str.as_str(),
                    writable_root_str.as_str(),
                ]
        })
        .expect("writable root should be rebound writable");

    assert!(
        writable_root_bind_index < blocked_mask_index,
        "expected unreadable carveout to be re-applied after writable bind: {:#?}",
        args.args
    );
}

#[test]
fn split_policy_reenables_nested_writable_subpaths_after_read_only_parent() {
    let temp_dir = TempDir::new().expect("temp dir");
    let writable_root = temp_dir.path().join("workspace");
    let docs = writable_root.join("docs");
    let docs_public = docs.join("public");
    std::fs::create_dir_all(&docs_public).expect("create docs/public");
    let writable_root =
        AbsolutePathBuf::from_absolute_path(&writable_root).expect("absolute writable root");
    let docs = AbsolutePathBuf::from_absolute_path(&docs).expect("absolute docs");
    let docs_public =
        AbsolutePathBuf::from_absolute_path(&docs_public).expect("absolute docs/public");
    let mut entries = vec![
        FileSystemSandboxEntry::new(writable_root.into(), FileSystemAccessMode::Write),
        FileSystemSandboxEntry::new(docs.clone().into(), FileSystemAccessMode::Read),
        FileSystemSandboxEntry::new(docs_public.clone().into(), FileSystemAccessMode::Write),
    ];
    for name in [".git", ".agents", ".codex", ".aws"] {
        entries.push(FileSystemSandboxEntry::skip_missing_path(
            docs_public.join(name).into(),
            FileSystemAccessMode::Read,
        ));
    }
    let policy = FileSystemSandboxPolicy::restricted(entries);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");
    let docs_str = path_to_string(docs.as_path());
    let docs_public_str = path_to_string(docs_public.as_path());
    let docs_ro_index = args
        .args
        .windows(3)
        .position(|window| window == ["--ro-bind", docs_str.as_str(), docs_str.as_str()])
        .expect("docs should be remounted read-only");
    let docs_public_rw_index = args
        .args
        .windows(3)
        .position(|window| window == ["--bind", docs_public_str.as_str(), docs_public_str.as_str()])
        .expect("docs/public should be rebound writable");

    assert!(
        docs_ro_index < docs_public_rw_index,
        "expected read-only parent remount before nested writable bind: {:#?}",
        args.args
    );
    for name in [".git", ".agents", ".codex", ".aws"] {
        let metadata_path = path_to_string(docs_public.join(name).as_path());
        let mount_indices = args
            .args
            .windows(2)
            .enumerate()
            .filter_map(|(index, window)| {
                (window == ["--tmpfs", metadata_path.as_str()]).then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(mount_indices.len(), 1, "one mount for {metadata_path}");
        assert!(
            docs_public_rw_index < mount_indices[0],
            "metadata must be mounted after its writable root: {:#?}",
            args.args
        );
    }
}

#[test]
fn split_policy_reenables_writable_subpaths_after_unreadable_parent() {
    let temp_dir = TempDir::new().expect("temp dir");
    let blocked = temp_dir.path().join("blocked");
    let allowed = blocked.join("allowed");
    std::fs::create_dir_all(&allowed).expect("create blocked/allowed");
    let blocked = AbsolutePathBuf::from_absolute_path(&blocked).expect("absolute blocked");
    let allowed = AbsolutePathBuf::from_absolute_path(&allowed).expect("absolute allowed");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: blocked.clone().into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: allowed.clone().into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
    ]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");
    let blocked_str = path_to_string(blocked.as_path());
    let allowed_str = path_to_string(allowed.as_path());
    let blocked_none_index = args
        .args
        .windows(4)
        .position(|window| window == ["--perms", "111", "--tmpfs", blocked_str.as_str()])
        .expect("blocked should be masked first");
    let allowed_dir_index = args
        .args
        .windows(2)
        .position(|window| window == ["--dir", allowed_str.as_str()])
        .expect("allowed mount target should be recreated");
    let blocked_remount_ro_index = args
        .args
        .windows(2)
        .position(|window| window == ["--remount-ro", blocked_str.as_str()])
        .expect("blocked directory should be remounted read-only");
    let allowed_bind_index = args
        .args
        .windows(3)
        .position(|window| window == ["--bind", allowed_str.as_str(), allowed_str.as_str()])
        .expect("allowed path should be rebound writable");

    assert!(
        blocked_none_index < allowed_dir_index
            && allowed_dir_index < blocked_remount_ro_index
            && blocked_remount_ro_index < allowed_bind_index,
        "expected writable child target recreation before remounting and rebinding under unreadable parent: {:#?}",
        args.args
    );
}

#[test]
fn split_policy_reenables_writable_files_after_unreadable_parent() {
    let temp_dir = TempDir::new().expect("temp dir");
    let blocked = temp_dir.path().join("blocked");
    let allowed_dir = blocked.join("allowed");
    let allowed_file = allowed_dir.join("note.txt");
    std::fs::create_dir_all(&allowed_dir).expect("create blocked/allowed");
    std::fs::write(&allowed_file, "ok").expect("create note");
    let blocked = AbsolutePathBuf::from_absolute_path(&blocked).expect("absolute blocked");
    let allowed_dir =
        AbsolutePathBuf::from_absolute_path(&allowed_dir).expect("absolute allowed dir");
    let allowed_file =
        AbsolutePathBuf::from_absolute_path(&allowed_file).expect("absolute allowed file");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: blocked.clone().into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: allowed_file.clone().into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
    ]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");
    let blocked_str = path_to_string(blocked.as_path());
    let allowed_dir_str = path_to_string(allowed_dir.as_path());
    let allowed_file_str = path_to_string(allowed_file.as_path());

    assert!(
        args.args
            .windows(2)
            .any(|window| window == ["--dir", allowed_dir_str.as_str()]),
        "expected ancestor directory to be recreated: {:#?}",
        args.args
    );
    assert!(
        !args
            .args
            .windows(2)
            .any(|window| window == ["--dir", allowed_file_str.as_str()]),
        "writable file target should not be converted into a directory: {:#?}",
        args.args
    );
    let blocked_none_index = args
        .args
        .windows(4)
        .position(|window| window == ["--perms", "111", "--tmpfs", blocked_str.as_str()])
        .expect("blocked should be masked first");
    let allowed_bind_index = args
        .args
        .windows(3)
        .position(|window| {
            window
                == [
                    "--bind",
                    allowed_file_str.as_str(),
                    allowed_file_str.as_str(),
                ]
        })
        .expect("allowed file should be rebound writable");

    assert!(
        blocked_none_index < allowed_bind_index,
        "expected unreadable parent mask before rebinding writable file child: {:#?}",
        args.args
    );
}

#[test]
fn split_policy_reenables_nested_writable_roots_after_unreadable_parent() {
    let temp_dir = TempDir::new().expect("temp dir");
    let writable_root = temp_dir.path().join("workspace");
    let blocked = writable_root.join("blocked");
    let allowed = blocked.join("allowed");
    std::fs::create_dir_all(&allowed).expect("create blocked/allowed dir");
    let writable_root =
        AbsolutePathBuf::from_absolute_path(&writable_root).expect("absolute writable root");
    let blocked = AbsolutePathBuf::from_absolute_path(&blocked).expect("absolute blocked dir");
    let allowed = AbsolutePathBuf::from_absolute_path(&allowed).expect("absolute allowed dir");
    let blocked_str = path_to_string(blocked.as_path());
    let allowed_str = path_to_string(allowed.as_path());
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: writable_root.into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: blocked.into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: allowed.into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
    ]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");
    let blocked_none_index = args
        .args
        .windows(4)
        .position(|window| window == ["--perms", "111", "--tmpfs", blocked_str.as_str()])
        .expect("blocked should be masked first");
    let allowed_dir_index = args
        .args
        .windows(2)
        .position(|window| window == ["--dir", allowed_str.as_str()])
        .expect("allowed mount target should be recreated");
    let allowed_bind_index = args
        .args
        .windows(3)
        .position(|window| window == ["--bind", allowed_str.as_str(), allowed_str.as_str()])
        .expect("allowed path should be rebound writable");

    assert!(
        blocked_none_index < allowed_dir_index && allowed_dir_index < allowed_bind_index,
        "expected unreadable parent mask before recreating and rebinding writable child: {:#?}",
        args.args
    );
}

#[test]
fn split_policy_masks_root_read_directory_carveouts() {
    let temp_dir = TempDir::new().expect("temp dir");
    let blocked = temp_dir.path().join("blocked");
    std::fs::create_dir_all(&blocked).expect("create blocked dir");
    let blocked = AbsolutePathBuf::from_absolute_path(&blocked).expect("absolute blocked dir");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: blocked.clone().into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");
    let blocked_str = path_to_string(blocked.as_path());

    assert!(
        args.args
            .windows(3)
            .any(|window| window == ["--ro-bind", "/", "/"])
    );
    assert!(
        args.args
            .windows(4)
            .any(|window| { window == ["--perms", "000", "--tmpfs", blocked_str.as_str()] })
    );
    assert!(
        args.args
            .windows(2)
            .any(|window| window == ["--remount-ro", blocked_str.as_str()])
    );
}

#[test]
fn split_policy_masks_root_read_file_carveouts() {
    let temp_dir = TempDir::new().expect("temp dir");
    let blocked_file = temp_dir.path().join("blocked.txt");
    let second_blocked_file = temp_dir.path().join("second-blocked.txt");
    std::fs::write(&blocked_file, "secret").expect("create blocked file");
    std::fs::write(&second_blocked_file, "dummy secret").expect("create second blocked file");
    let blocked_file =
        AbsolutePathBuf::from_absolute_path(&blocked_file).expect("absolute blocked file");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: blocked_file.clone().into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: AbsolutePathBuf::from_absolute_path(&second_blocked_file)
                .expect("absolute second blocked file")
                .into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    let args = create_filesystem_args(&policy, temp_dir.path(), BwrapOptions::default())
        .expect("filesystem args");
    assert_eq!(args.preserved_files.len(), 2);
    assert_bind_data_uses_distinct_preserved_fds(&args);
    assert!(args.synthetic_mount_targets.is_empty());
    for blocked_file in [blocked_file.as_path(), second_blocked_file.as_path()] {
        assert!(args.args.windows(5).any(|window| {
            window[0] == "--perms"
                && window[1] == "000"
                && window[2] == "--ro-bind-data"
                && window[4] == path_to_string(blocked_file)
        }));
    }
}

fn assert_bind_data_uses_distinct_preserved_fds(args: &BwrapArgs) {
    let mount_fds = args
        .args
        .windows(3)
        .filter(|window| window[0] == "--ro-bind-data")
        .map(|window| window[1].parse::<i32>().expect("bind-data fd"))
        .collect::<Vec<_>>();
    let distinct_fds = mount_fds.iter().copied().collect::<HashSet<_>>();
    assert_eq!(mount_fds.len(), distinct_fds.len());
    assert_eq!(
        distinct_fds,
        args.preserved_files
            .iter()
            .map(AsRawFd::as_raw_fd)
            .collect()
    );
}

#[test]
fn unreadable_globs_expand_existing_matches_with_configured_depth() {
    if !ripgrep_available() {
        return;
    }

    let temp_dir = TempDir::new().expect("temp dir");
    let root_env = temp_dir.path().join(".env");
    let nested_env = temp_dir.path().join("app").join(".env");
    let too_deep_env = temp_dir.path().join("app").join("deep").join(".env");
    std::fs::create_dir_all(too_deep_env.parent().expect("parent")).expect("create parent");
    std::fs::write(temp_dir.path().join(".gitignore"), ".env\n").expect("write gitignore");
    std::fs::write(&root_env, "secret").expect("write root env");
    std::fs::write(&nested_env, "secret").expect("write nested env");
    std::fs::write(&too_deep_env, "secret").expect("write deep env");
    let policy =
        default_policy_with_unreadable_glob(format!("{}/**/*.env", temp_dir.path().display()));

    let args = create_filesystem_args(
        &policy,
        temp_dir.path(),
        BwrapOptions {
            glob_scan_max_depth: Some(2),
            ..Default::default()
        },
    )
    .expect("filesystem args");

    assert_file_masked(&args.args, &root_env);
    assert_file_masked(&args.args, &nested_env);
    assert!(
        !args
            .args
            .iter()
            .any(|arg| arg == &path_to_string(&too_deep_env)),
        "max depth should keep deeper matches out of bwrap args: {:#?}",
        args.args
    );
}

#[cfg(unix)]
#[test]
fn unreadable_globs_add_canonical_targets_for_symlink_matches() {
    if !ripgrep_available() {
        return;
    }

    let temp_dir = TempDir::new().expect("temp dir");
    let real_root = temp_dir.path().join("real");
    let link_root = temp_dir.path().join("link");
    let real_secret = real_root.join("secret.env");
    std::fs::create_dir_all(&real_root).expect("create real root");
    std::fs::write(&real_secret, "secret").expect("write real secret");
    std::os::unix::fs::symlink(&real_root, &link_root).expect("create symlink");
    let policy = default_policy_with_unreadable_glob(format!("{}/**/*.env", link_root.display()));

    let args = create_filesystem_args(
        &policy,
        temp_dir.path(),
        BwrapOptions {
            glob_scan_max_depth: Some(2),
            ..Default::default()
        },
    )
    .expect("filesystem args");

    assert_file_masked(&args.args, &real_secret);
}

#[test]
fn root_prefix_unreadable_globs_fail_closed_on_linux() {
    let policy = default_policy_with_unreadable_glob("/**/*.env".to_string());
    let error = create_bwrap_command_args(
        vec!["/bin/true".to_string()],
        &policy,
        Path::new("/tmp"),
        Path::new("/tmp"),
        BwrapOptions::default(),
    )
    .expect_err("root-prefix deny-read glob must reject sandbox construction");

    assert_eq!(
        error.to_string(),
        "Fatal error: unreadable glob `/**/*.env` cannot be safely expanded; use a pattern with a non-root directory prefix"
    );
}

#[test]
fn unclosed_character_classes_are_escaped_for_ripgrep() {
    let (search_root, glob) =
        split_pattern_for_ripgrep("/tmp/[*.env", Path::new("/")).expect("split pattern");

    assert_eq!(search_root.as_path(), Path::new("/tmp"));
    assert_eq!(glob, r"\[*.env");
}

fn ripgrep_available() -> bool {
    Command::new(GLOB_SCAN_PROGRAM)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Assert that `path` is masked due to a bwrap arg sequence like:
///
/// `bwrap ... --perms 000 --ro-bind-data FD PATH`
fn assert_file_masked(args: &[String], path: &Path) {
    let path = path_to_string(path);
    assert!(
        args.windows(5).any(|window| {
            window[0] == "--perms"
                && window[1] == "000"
                && window[2] == "--ro-bind-data"
                && window[4] == path
        }),
        "expected file mask for {path}: {args:#?}"
    );
}

/// Assert that `path` is backed by an fd-supplied empty file without
/// changing the next mount operation's permissions.
fn assert_empty_file_bound_without_perms(args: &[String], path: &Path) {
    let path = path_to_string(path);
    assert!(
        args.windows(3)
            .any(|window| { window[0] == "--ro-bind-data" && window[2] == path }),
        "expected empty file bind for {path}: {args:#?}"
    );
    assert!(
        !args.windows(5).any(|window| {
            window[0] == "--perms"
                && window[1] == "000"
                && window[2] == "--ro-bind-data"
                && window[4] == path
        }),
        "missing path bind should not set explicit file perms for {path}: {args:#?}"
    );
}

fn assert_empty_directory_mounted_read_only(args: &[String], path: &Path) {
    let path = path_to_string(path);
    assert!(
        args.windows(4)
            .any(|window| window == ["--perms", "555", "--tmpfs", path.as_str()]),
        "expected empty directory mount for {path}: {args:#?}"
    );
    assert!(
        args.windows(2)
            .any(|window| window == ["--remount-ro", path.as_str()]),
        "expected read-only remount for {path}: {args:#?}"
    );
}

fn synthetic_mount_target_paths(args: &BwrapArgs) -> Vec<PathBuf> {
    args.synthetic_mount_targets
        .iter()
        .map(|target| target.path().to_path_buf())
        .collect()
}
