#![cfg(windows)]

mod common;

#[path = "file_system/shared.rs"]
mod shared;
#[path = "file_system/support.rs"]
mod support;

use std::path::Path;
use std::process::Command;

use anyhow::Result;
use codex_exec_server::FileSystemSandboxContext;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_path_uri::PathUri;
use test_case::test_case;

use crate::support::FileSystemImplementation;
use crate::support::create_file_system_context;

fn create_directory_junction(target: &Path, alias: &Path) -> Result<()> {
    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(alias)
        .arg(target)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "mklink /J failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_canonicalize_resolves_directory_junction(
    implementation: FileSystemImplementation,
) -> Result<()> {
    shared::assert_canonicalize_resolves_directory_alias(implementation, create_directory_junction)
        .await
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_canonicalize_resolves_directory_junction(
    implementation: FileSystemImplementation,
) -> Result<()> {
    shared::assert_sandboxed_canonicalize_resolves_directory_alias(
        implementation,
        create_directory_junction,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_remote_fs_helper_respects_windows_sandbox_write_policy() -> Result<()> {
    let context = create_file_system_context(FileSystemImplementation::Remote).await?;
    let file_system = context.file_system;
    let tmp = tempfile::TempDir::new()?;
    let readonly_dir = tmp.path().join("readonly");
    std::fs::create_dir_all(&readonly_dir)?;

    let mut sandbox = read_only_sandbox_for_cwd(readonly_dir.clone())?;
    sandbox.windows_sandbox_level = WindowsSandboxLevel::RestrictedToken;

    let readable_file = readonly_dir.join("readable.txt");
    std::fs::write(&readable_file, b"readable")?;
    let read_result = file_system
        .read_file(&PathUri::from_path(&readable_file)?, Some(&sandbox))
        .await;
    // Some local Windows hosts cannot create restricted tokens. Reaching that
    // error still proves the remote fs helper went through the Windows sandbox
    // launcher; before the wrapper fix this read would have run unsandboxed.
    if is_unsupported_restricted_token_host(&read_result) {
        return Ok(());
    }
    assert_eq!(read_result?, b"readable");

    let blocked_file = readonly_dir.join("blocked.txt");
    let error = file_system
        .write_file(
            &PathUri::from_path(&blocked_file)?,
            b"blocked".to_vec(),
            Some(&sandbox),
        )
        .await
        .expect_err("write outside the sandbox should fail");
    assert!(
        !blocked_file.exists(),
        "sandboxed fs helper must not create blocked file after error: {error}"
    );

    Ok(())
}

fn read_only_sandbox_for_cwd(cwd: std::path::PathBuf) -> Result<FileSystemSandboxContext> {
    Ok(FileSystemSandboxContext::from_legacy_sandbox_policy(
        SandboxPolicy::new_read_only_policy(),
        PathUri::from_path(cwd)?,
    )?)
}

fn is_unsupported_restricted_token_host<T>(result: &std::io::Result<T>) -> bool {
    result.as_ref().err().is_some_and(|err| {
        err.to_string()
            .contains("windows sandbox failed: CreateRestrictedToken failed: 87")
    })
}
