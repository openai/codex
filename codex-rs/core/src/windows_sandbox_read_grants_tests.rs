use super::grant_read_root_non_elevated;
use codex_protocol::protocol::SandboxPolicy;
use std::collections::HashMap;
use std::path::Path;
use tempfile::TempDir;

fn policy() -> SandboxPolicy {
    SandboxPolicy::new_workspace_write_policy()
}

#[test]
fn rejects_relative_path() {
    let tmp = TempDir::new().expect("tempdir");
    let err = grant_read_root_non_elevated(
        &policy(),
        tmp.path(),
        tmp.path(),
        &HashMap::new(),
        tmp.path(),
        Path::new("relative"),
    )
    .expect_err("relative path should fail");
    assert!(err.to_string().contains("path must be absolute"));
}

#[test]
fn rejects_missing_path() {
    let tmp = TempDir::new().expect("tempdir");
    let missing = tmp.path().join("does-not-exist");
    let err = grant_read_root_non_elevated(
        &policy(),
        tmp.path(),
        tmp.path(),
        &HashMap::new(),
        tmp.path(),
        missing.as_path(),
    )
    .expect_err("missing path should fail");
    assert!(err.to_string().contains("path does not exist"));
}

#[test]
fn rejects_file_path() {
    let tmp = TempDir::new().expect("tempdir");
    let file_path = tmp.path().join("file.txt");
    std::fs::write(&file_path, "hello").expect("write file");
    let err = grant_read_root_non_elevated(
        &policy(),
        tmp.path(),
        tmp.path(),
        &HashMap::new(),
        tmp.path(),
        file_path.as_path(),
    )
    .expect_err("file path should fail");
    assert!(err.to_string().contains("path must be a directory"));
}
