use super::*;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::path::PathBuf;
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
        find_system_bwrap_in_search_paths(std::env::split_paths(&search_path), &cwd),
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
        find_system_bwrap_in_search_paths(std::env::split_paths(&search_path), &cwd),
        Some(expected_bwrap)
    );
}

fn write_fake_bwrap(contents: &str) -> tempfile::TempPath {
    write_fake_bwrap_in(
        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        contents,
    )
}

fn write_fake_bwrap_in(dir: &Path, contents: &str) -> tempfile::TempPath {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::NamedTempFile;

    // Bazel can mount the OS temp directory `noexec`, so prefer the current
    // working directory for fake executables and fall back to the default temp
    // dir outside that environment.
    let temp_file = NamedTempFile::new_in(dir)
        .ok()
        .unwrap_or_else(|| NamedTempFile::new().expect("temp file"));
    // Linux rejects exec-ing a file that is still open for writing.
    let path = temp_file.into_temp_path();
    fs::write(&path, contents).expect("write fake bwrap");
    let permissions = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&path, permissions).expect("chmod fake bwrap");
    path
}

fn write_named_fake_bwrap_in(dir: &Path) -> PathBuf {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("bwrap");
    fs::write(&path, "#!/bin/sh\n").expect("write fake bwrap");
    let permissions = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&path, permissions).expect("chmod fake bwrap");
    fs::canonicalize(path).expect("canonicalize fake bwrap")
}
