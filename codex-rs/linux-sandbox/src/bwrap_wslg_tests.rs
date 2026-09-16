//! Regression coverage for excluding WSLg's duplicate filesystem view.

use super::*;
use codex_protocol::protocol::FileSystemSandboxEntry;
use pretty_assertions::assert_eq;

#[test_case::test_case(FileSystemAccessMode::Read; "read_root")]
#[test_case::test_case(FileSystemAccessMode::Write; "write_root")]
fn wslg_mask_follows_filesystem_grants_and_denials(root_access: FileSystemAccessMode) {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let denied = temp_dir.path().join("denied.txt");
    fs::write(&denied, "fixture").expect("write fixture");
    let denied = AbsolutePathBuf::from_absolute_path(denied).expect("absolute fixture");
    let workspace =
        AbsolutePathBuf::from_absolute_path(temp_dir.path()).expect("absolute workspace");

    for denied_path in [
        FileSystemPath::from(denied.clone()),
        FileSystemPath::GlobPattern {
            pattern: format!("{}/*.txt", temp_dir.path().display()),
        },
    ] {
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: root_access,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: workspace.clone().into(),
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: denied_path,
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
        ]);
        for mount_proc in [true, false] {
            for network_mode in [BwrapNetworkMode::FullAccess, BwrapNetworkMode::Isolated] {
                let args = create_bwrap_command_args(
                    vec!["/bin/true".to_string()],
                    &policy,
                    temp_dir.path(),
                    temp_dir.path(),
                    BwrapOptions {
                        mount_proc,
                        network_mode,
                        mask_wslg_distro: true,
                        ..Default::default()
                    },
                )
                .expect("create restricted bwrap args")
                .args;
                let mask = args
                    .windows(6)
                    .position(|args| {
                        args == [
                            "--perms",
                            "000",
                            "--tmpfs",
                            WSLG_DISTRO_ROOT,
                            "--remount-ro",
                            WSLG_DISTRO_ROOT,
                        ]
                    })
                    .expect("unreadable, read-only WSLg mask");
                let last_grant = args
                    .iter()
                    .rposition(|arg| arg == "--bind" || arg == "--ro-bind")
                    .expect("filesystem grant");
                let denial = args
                    .iter()
                    .rposition(|arg| arg == &path_to_string(denied.as_path()))
                    .expect("denied file mask");
                assert!(last_grant < mask && denial < mask);
                assert!(args.windows(2).any(|args| {
                    args == [if mount_proc { "--proc" } else { "--tmpfs" }, "/proc"]
                }));
            }
        }
    }
}

#[test]
fn wslg_mask_is_omitted_when_policy_already_hides_an_ancestor() {
    for denied in ["/mnt", "/mnt/wslg", WSLG_DISTRO_ROOT] {
        let mut policy = FileSystemSandboxPolicy::read_only();
        policy.entries.push(FileSystemSandboxEntry {
            path: AbsolutePathBuf::from_absolute_path(denied)
                .expect("absolute denied directory")
                .into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        });
        let mut commands = Vec::new();
        for mask_wslg_distro in [false, true] {
            commands.push(
                create_bwrap_command_args(
                    vec!["/bin/true".to_string()],
                    &policy,
                    Path::new("/"),
                    Path::new("/"),
                    BwrapOptions {
                        mask_wslg_distro,
                        ..Default::default()
                    },
                )
                .expect("create bwrap args with denied ancestor")
                .args,
            );
        }
        assert_eq!(commands[0], commands[1]);
    }
}

#[test]
fn unrestricted_filesystem_preserves_wslg_with_or_without_network_isolation() {
    for network_mode in [BwrapNetworkMode::FullAccess, BwrapNetworkMode::Isolated] {
        let mut commands = Vec::new();
        for mask_wslg_distro in [false, true] {
            commands.push(
                create_bwrap_command_args(
                    vec!["/bin/true".to_string()],
                    &FileSystemSandboxPolicy::unrestricted(),
                    Path::new("/"),
                    Path::new("/"),
                    BwrapOptions {
                        network_mode,
                        mask_wslg_distro,
                        ..Default::default()
                    },
                )
                .expect("create unrestricted bwrap args")
                .args,
            );
        }
        assert_eq!(commands[0], commands[1]);
    }
}

#[test_case::test_case("grant_read"; "read_grant")]
#[test_case::test_case("grant_write"; "write_grant")]
#[test_case::test_case("policy_cwd"; "policy_cwd")]
#[test_case::test_case("command_cwd"; "command_cwd")]
#[test_case::test_case("executable"; "executable")]
fn explicit_wslg_alias_paths_are_rejected(source: &str) {
    let alias = Path::new(WSLG_DISTRO_ROOT).join("project");
    let mut policy = FileSystemSandboxPolicy::read_only();
    if source.starts_with("grant_") {
        policy.entries.push(FileSystemSandboxEntry {
            path: AbsolutePathBuf::from_absolute_path(&alias)
                .expect("absolute alias")
                .into(),
            access: if source == "grant_write" {
                FileSystemAccessMode::Write
            } else {
                FileSystemAccessMode::Read
            },
            missing_path_behavior: None,
        });
    }
    let executable = if source == "executable" {
        alias.as_path()
    } else {
        Path::new("/bin/true")
    };
    let policy_cwd = if source == "policy_cwd" {
        alias.as_path()
    } else {
        Path::new("/")
    };
    let command_cwd = if source == "command_cwd" {
        alias.as_path()
    } else {
        Path::new("/")
    };
    let error = create_bwrap_command_args(
        vec![path_to_string(executable)],
        &policy,
        policy_cwd,
        command_cwd,
        BwrapOptions {
            mask_wslg_distro: true,
            ..Default::default()
        },
    )
    .expect_err("explicit alias must be rejected before mounts are built");
    assert_eq!(
        error.to_string(),
        format!(
            "Fatal error: restricted sandboxes do not support paths under {WSLG_DISTRO_ROOT}: {}; use the corresponding path under the primary filesystem root instead",
            alias.display()
        )
    );
}

#[test]
fn glob_ancestor_mask_in_no_rg_fallback() {
    // Isolate PATH in a child test process instead of mutating the test runner's
    // environment. This exercises the supported missing-rg branch end to end.
    const CHILD: &str = "CODEX_WSLG_GLOB_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "bwrap::wslg_tests::glob_ancestor_mask_in_no_rg_fallback",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("PATH", "")
            .output()
            .expect("run isolated test");
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let temp = tempfile::tempdir().expect("temp directory");
    // '/' is an ancestor on every test host; no WSL mount is needed. Only
    // inspect arguments: never launch this synthetic filesystem policy.
    let relative_root: PathBuf =
        std::iter::repeat_n("..", temp.path().components().count() - 1).collect();
    std::os::unix::fs::symlink(relative_root, temp.path().join("alias")).expect("relative symlink");
    let mut policy = FileSystemSandboxPolicy::read_only();
    policy.entries.push(FileSystemSandboxEntry {
        path: FileSystemPath::GlobPattern {
            pattern: format!("{}/alias*", temp.path().display()),
        },
        access: FileSystemAccessMode::Deny,
        missing_path_behavior: None,
    });
    let args = create_bwrap_command_args(
        vec!["/bin/true".to_string()],
        &policy,
        Path::new("/"),
        Path::new("/"),
        BwrapOptions {
            mask_wslg_distro: true,
            mount_proc: false,
            ..Default::default()
        },
    )
    .expect("expanded ancestor mask should suffice")
    .args;
    assert!(
        args.windows(4)
            .any(|args| args == ["--tmpfs", "/", "--remount-ro", "/"])
    );
    assert!(!args.iter().any(|arg| arg == WSLG_DISTRO_ROOT));
    assert!(args.windows(2).any(|args| args == ["--tmpfs", "/proc"]));
}
