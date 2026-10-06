use super::*;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use tempfile::tempdir;

#[test]
fn system_bwrap_warning_reports_missing_system_bwrap() {
    assert_eq!(
        system_bwrap_warning_for_path(/*system_bwrap_path*/ None),
        Some(MISSING_BWRAP_WARNING.to_string())
    );
}

#[test]
fn system_bwrap_warning_reports_user_namespace_failures() {
    for failure in USER_NAMESPACE_FAILURES {
        let fake_bwrap = write_fake_bwrap(&format!(
            r#"#!/bin/sh
echo '{failure}' >&2
exit 1
"#
        ));
        let fake_bwrap_path: &Path = fake_bwrap.as_ref();

        assert_eq!(
            system_bwrap_warning_for_path(Some(fake_bwrap_path)),
            Some(USER_NAMESPACE_WARNING.to_string()),
            "{failure}",
        );
    }
}

#[test]
fn system_bwrap_warning_skips_unrelated_bwrap_failures() {
    let fake_bwrap = write_fake_bwrap(
        r#"#!/bin/sh
echo 'bwrap: Unknown option --argv0' >&2
exit 1
"#,
    );
    let fake_bwrap_path: &Path = fake_bwrap.as_ref();

    assert_eq!(system_bwrap_warning_for_path(Some(fake_bwrap_path)), None);
}

#[test]
fn system_bwrap_probe_times_out_without_reporting_a_warning() {
    let fake_bwrap = write_fake_bwrap(
        r#"#!/bin/sh
sleep 1
exit 0
"#,
    );
    let fake_bwrap_path: &Path = fake_bwrap.as_ref();
    let started_at = Instant::now();

    assert!(system_bwrap_has_user_namespace_access(
        fake_bwrap_path,
        Duration::from_millis(10),
    ));
    assert!(started_at.elapsed() < Duration::from_millis(500));
}

#[test]
fn system_bwrap_probe_does_not_wait_for_descendants_holding_stderr_open() {
    let fake_bwrap = write_fake_bwrap(
        r#"#!/bin/sh
echo 'No permissions to create a new namespace' >&2
sleep 1 &
exit 1
"#,
    );
    let fake_bwrap_path: &Path = fake_bwrap.as_ref();
    let started_at = Instant::now();

    assert!(!system_bwrap_has_user_namespace_access(
        fake_bwrap_path,
        Duration::from_millis(100),
    ));
    assert!(started_at.elapsed() < Duration::from_millis(500));
}

#[test]
fn detects_wsl1_proc_version_formats() {
    assert!(proc_version_indicates_wsl1(
        "Linux version 4.4.0-22621-Microsoft"
    ));
    assert!(proc_version_indicates_wsl1(
        "Linux version 5.15.0-microsoft-standard-WSL1"
    ));
    assert!(proc_version_indicates_wsl1(
        "Linux version 5.15.0-wsl-microsoft-standard-WSL1"
    ));
}

#[test]
fn does_not_treat_wsl2_or_native_linux_as_wsl1() {
    assert!(!proc_version_indicates_wsl1(
        "Linux version 6.6.87.2-microsoft-standard-WSL2"
    ));
    assert!(!proc_version_indicates_wsl1(
        "Linux version 6.6.87.2-wsl-microsoft-standard-WSL2"
    ));
    assert!(!proc_version_indicates_wsl1(
        "Linux version 4.19.104-microsoft-standard"
    ));
    assert!(!proc_version_indicates_wsl1(
        "Linux version 6.6.87.2-microsoft-standard-WSL3"
    ));
    assert!(!proc_version_indicates_wsl1("Linux version 6.8.0"));
}

#[test]
fn finds_first_executable_bwrap_in_joined_search_path() {
    let temp_dir = tempdir().expect("temp dir");
    let cwd = temp_dir.path().join("cwd");
    let first_dir = temp_dir.path().join("first");
    let second_dir = temp_dir.path().join("second");
    std::fs::create_dir_all(&cwd).expect("create cwd");
    std::fs::create_dir_all(&first_dir).expect("create first dir");
    std::fs::create_dir_all(&second_dir).expect("create second dir");
    std::fs::write(first_dir.join("bwrap"), "not executable").expect("write non-executable bwrap");
    let expected_bwrap = write_named_fake_bwrap_in(&second_dir);
    let search_path = std::env::join_paths([first_dir, second_dir]).expect("join search path");

    assert_eq!(
        find_executable_in_search_paths(
            "bwrap",
            std::env::split_paths(&search_path),
            &cwd,
            &PermissionProfile::read_only().file_system_sandbox_policy(),
            &cwd,
        ),
        Some(expected_bwrap)
    );
}

#[test]
fn skips_workspace_local_bwrap_in_joined_search_path() {
    let temp_dir = tempdir().expect("temp dir");
    let cwd = temp_dir.path().join("cwd");
    let trusted_dir = temp_dir.path().join("trusted");
    std::fs::create_dir_all(&cwd).expect("create cwd");
    std::fs::create_dir_all(&trusted_dir).expect("create trusted dir");
    let _workspace_bwrap = write_named_fake_bwrap_in(&cwd);
    let expected_bwrap = write_named_fake_bwrap_in(&trusted_dir);
    let search_path = std::env::join_paths([cwd.clone(), trusted_dir]).expect("join search path");

    assert_eq!(
        find_executable_in_search_paths(
            "bwrap",
            std::env::split_paths(&search_path),
            &cwd,
            &PermissionProfile::read_only().file_system_sandbox_policy(),
            &cwd,
        ),
        Some(expected_bwrap)
    );
}

#[test]
fn root_cwd_does_not_hide_system_bwrap_candidates() {
    let temp_dir = tempdir().expect("temp dir");
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let expected_bwrap = write_named_fake_bwrap_in(&bin_dir);
    let search_path = std::env::join_paths([bin_dir]).expect("join search path");

    assert_eq!(
        find_executable_in_search_paths(
            "bwrap",
            std::env::split_paths(&search_path),
            Path::new("/"),
            &PermissionProfile::read_only().file_system_sandbox_policy(),
            Path::new("/"),
        ),
        Some(expected_bwrap)
    );
    assert_eq!(
        find_executable_in_search_paths(
            "bwrap",
            std::env::split_paths(&search_path),
            Path::new("/"),
            &PermissionProfile::Disabled.file_system_sandbox_policy(),
            Path::new("/"),
        ),
        None,
    );
}

#[test]
fn skips_bwrap_in_writable_roots_outside_command_cwd() {
    let temp_dir = tempdir().expect("temp dir");
    let workspace = temp_dir.path().join("workspace");
    let cwd = workspace.join("service");
    let workspace_bin = workspace.join("bin");
    let extra_root = temp_dir.path().join("extra");
    let trusted_dir = temp_dir.path().join("trusted");
    for dir in [&cwd, &workspace_bin, &extra_root, &trusted_dir] {
        std::fs::create_dir_all(dir).expect("create directory");
    }
    write_named_fake_bwrap_in(&workspace_bin);
    write_named_fake_bwrap_in(&extra_root);
    let expected_bwrap = write_named_fake_bwrap_in(&trusted_dir);
    let writable_roots = vec![workspace, extra_root.clone()];
    let policy = PermissionProfile::read_only()
        .file_system_sandbox_policy()
        .with_additional_writable_roots(
            &cwd,
            &writable_roots
                .into_iter()
                .map(|path| AbsolutePathBuf::try_from(path).unwrap())
                .collect::<Vec<_>>(),
        );
    let search_paths = vec![workspace_bin, extra_root];

    assert_eq!(
        find_executable_in_search_paths("bwrap", search_paths.clone(), &cwd, &policy, &cwd),
        None,
    );
    assert_eq!(
        find_executable_in_search_paths(
            "bwrap",
            search_paths.into_iter().chain([trusted_dir]),
            &cwd,
            &policy,
            &cwd,
        ),
        Some(expected_bwrap),
    );
}

#[test]
fn skips_bwrap_in_symlinked_writable_root() {
    let temp_dir = tempdir().expect("temp dir");
    let workspace = temp_dir.path().join("workspace");
    let cwd = workspace.join("service");
    let workspace_bin = workspace.join("bin");
    let alias = temp_dir.path().join("alias");
    std::fs::create_dir_all(&cwd).expect("create cwd");
    std::fs::create_dir_all(&workspace_bin).expect("create bin");
    std::os::unix::fs::symlink(&workspace, &alias).expect("create workspace alias");
    write_named_fake_bwrap_in(&workspace_bin);
    let policy = PermissionProfile::read_only()
        .file_system_sandbox_policy()
        .with_additional_writable_roots(&cwd, &[AbsolutePathBuf::try_from(alias).unwrap()]);

    assert_eq!(
        find_executable_in_search_paths("bwrap", [workspace_bin], &cwd, &policy, &cwd),
        None,
    );
}

#[test]
fn full_disk_write_rejects_owned_bwrap_even_when_chmod_read_only() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().unwrap();
    let bin = temp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let bwrap = write_named_fake_bwrap_in(&bin);
    std::fs::set_permissions(&bwrap, std::fs::Permissions::from_mode(0o555)).unwrap();
    let policy = PermissionProfile::Disabled.file_system_sandbox_policy();

    assert_eq!(
        find_executable_in_search_paths("bwrap", [bin], Path::new("/"), &policy, Path::new("/")),
        None,
    );
}

#[test]
fn read_only_bwrap_carveout_does_not_trust_a_replaceable_parent() {
    let temp = tempdir().unwrap();
    let bin = temp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let bwrap = write_named_fake_bwrap_in(&bin);
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            AbsolutePathBuf::try_from(temp.path()).unwrap().into(),
            FileSystemAccessMode::Write,
        ),
        FileSystemSandboxEntry::new(
            AbsolutePathBuf::try_from(bwrap).unwrap().into(),
            FileSystemAccessMode::Read,
        ),
    ]);

    assert_eq!(
        find_executable_in_search_paths("bwrap", [bin], Path::new("/"), &policy, Path::new("/")),
        None,
    );
}

#[test]
fn root_write_preserves_a_read_only_system_installation() {
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSpecialPath;

    let temp = tempdir().unwrap();
    // Use a system executable as a discovery-only candidate. Never execute it.
    let system_binary = std::fs::canonicalize("/bin/true").unwrap();
    let protected_root = system_binary
        .ancestors()
        .find(|path| path.parent() == Some(Path::new("/")))
        .unwrap();
    std::os::unix::fs::symlink(&system_binary, temp.path().join("bwrap")).unwrap();
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Write,
        ),
        FileSystemSandboxEntry::new(
            AbsolutePathBuf::try_from(protected_root).unwrap().into(),
            FileSystemAccessMode::Read,
        ),
    ]);

    for policy in [
        policy,
        PermissionProfile::Disabled.file_system_sandbox_policy(),
    ] {
        // Root can modify the system installation unless the policy protects it.
        // SAFETY: geteuid has no preconditions.
        let can_modify_system =
            policy.has_full_disk_write_access() && unsafe { libc::geteuid() } == 0;
        assert_eq!(
            find_executable_in_search_paths(
                "bwrap",
                [temp.path().to_path_buf()],
                Path::new("/"),
                &policy,
                Path::new("/"),
            ),
            (!can_modify_system).then(|| system_binary.clone()),
        );
    }
}

fn write_fake_bwrap(contents: &str) -> tempfile::TempPath {
    write_fake_bwrap_in(
        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        contents,
    )
}

fn write_fake_bwrap_in(dir: &Path, contents: &str) -> tempfile::TempPath {
    use tempfile::NamedTempFile;

    // Bazel can mount the OS temp directory `noexec`, so prefer the current
    // working directory for fake executables and fall back to the default temp
    // dir outside that environment.
    let temp_file = NamedTempFile::new_in(dir)
        .ok()
        .unwrap_or_else(|| NamedTempFile::new().expect("temp file"));
    let path = temp_file.into_temp_path();
    // A sibling spawn may retain the temporary file's writable descriptor.
    // Remove that inode before the helper creates the executable in its child.
    std::fs::remove_file(&path).expect("remove empty temporary file");
    codex_utils_cargo_bin::write_executable(&path, contents).expect("write fake bwrap");
    path
}

fn write_named_fake_bwrap_in(dir: &Path) -> PathBuf {
    use std::fs;

    let path = dir.join("bwrap");
    codex_utils_cargo_bin::write_executable(&path, "#!/bin/sh\n").expect("write fake bwrap");
    fs::canonicalize(path).expect("canonicalize fake bwrap")
}
