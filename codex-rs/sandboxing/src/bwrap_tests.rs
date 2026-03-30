use super::*;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn system_bwrap_warning_reports_missing_system_bwrap() {
    let warning = system_bwrap_warning_for_lookup(/*system_bwrap_path*/ None)
        .expect("missing system bwrap should emit a warning");

    assert!(warning.contains("could not find system bubblewrap"));
}

#[test]
fn system_bwrap_warning_skips_too_old_system_bwrap() {
    let fake_bwrap = write_fake_bwrap(
        r#"#!/bin/sh
if [ "$1" = "--help" ]; then
  echo 'usage: bwrap [OPTION...] COMMAND'
  exit 0
fi
exit 1
"#,
    );
    let fake_bwrap_path: &Path = fake_bwrap.as_ref();

    assert_eq!(
        system_bwrap_warning_for_lookup(Some(fake_bwrap_path.to_path_buf())),
        None,
        "Do not warn even if bwrap does not support `--argv0`",
    );
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
    path
}
