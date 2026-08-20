#![cfg(windows)]
#![allow(clippy::expect_used)]

mod common;

#[path = "file_system/shared.rs"]
mod shared;
#[path = "file_system/support.rs"]
mod support;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::GetMetadataOptions;
use codex_exec_server::ReadFileOptions;
use codex_exec_server::RemoveOptions;
use codex_exec_server::WriteFileOptions;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_path_uri::PathUri;
use test_case::test_case;
use tokio::net::windows::named_pipe::ServerOptions;
use tokio::time::timeout;
use uuid::Uuid;

use crate::support::FileSystemImplementation;
use crate::support::create_file_system_context;
use crate::support::is_unsupported_restricted_token_host;
use crate::support::workspace_write_sandbox;

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

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_operations_can_reject_junctions_in_any_path_component(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let tmp = tempfile::TempDir::new()?;
    let real = tmp.path().join("real");
    std::fs::create_dir(&real)?;
    let existing = real.join("existing.txt");
    std::fs::write(&existing, "unchanged")?;
    let removable = real.join("removable.txt");
    std::fs::write(&removable, "keep")?;
    let directory_junction = tmp.path().join("directory-junction");
    create_directory_junction(&real, &directory_junction)?;

    let no_follow_read = ReadFileOptions {
        follow_symlinks: false,
    };
    let no_follow_write = WriteFileOptions {
        follow_symlinks: false,
    };
    let no_follow_metadata = GetMetadataOptions {
        follow_symlinks: false,
    };
    let no_follow_create = CreateDirectoryOptions {
        recursive: true,
        follow_symlinks: false,
    };
    let no_follow_remove = RemoveOptions {
        recursive: false,
        force: false,
        follow_symlinks: false,
    };
    let uri = |path: &Path| PathUri::from_host_native_path(path);

    assert_eq!(
        context
            .file_system
            .read_file(&uri(&existing)?, no_follow_read, /*sandbox*/ None,)
            .await?,
        b"unchanged"
    );

    let file_link_target = real.join("file-link-target.txt");
    std::fs::write(&file_link_target, "target")?;
    let file_link = tmp.path().join("file-link.txt");
    if std::os::windows::fs::symlink_file(&file_link_target, &file_link).is_ok() {
        assert!(
            context
                .file_system
                .write_file(
                    &uri(&file_link)?,
                    b"changed".to_vec(),
                    no_follow_write,
                    /*sandbox*/ None,
                )
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&file_link_target)?, "target");
    }

    assert!(
        context
            .file_system
            .read_file(
                &uri(&directory_junction.join("existing.txt"))?,
                no_follow_read,
                /*sandbox*/ None,
            )
            .await
            .is_err()
    );
    assert!(
        context
            .file_system
            .write_file(
                &uri(&directory_junction.join("existing.txt"))?,
                b"changed".to_vec(),
                no_follow_write,
                /*sandbox*/ None,
            )
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(&existing)?, "unchanged");
    assert!(
        context
            .file_system
            .get_metadata(
                &uri(&directory_junction)?,
                no_follow_metadata,
                /*sandbox*/ None,
            )
            .await
            .is_err()
    );
    let directory_metadata = context
        .file_system
        .get_metadata(&uri(&real)?, no_follow_metadata, /*sandbox*/ None)
        .await?;
    assert!(directory_metadata.is_directory);
    assert!(
        context
            .file_system
            .create_directory(
                &uri(&directory_junction.join("created"))?,
                no_follow_create,
                /*sandbox*/ None,
            )
            .await
            .is_err()
    );
    assert!(!real.join("created").exists());
    assert!(
        context
            .file_system
            .remove(
                &uri(&directory_junction.join("removable.txt"))?,
                no_follow_remove,
                /*sandbox*/ None,
            )
            .await
            .is_err()
    );
    assert!(removable.exists());
    assert!(
        context
            .file_system
            .remove(
                &uri(&directory_junction)?,
                no_follow_remove,
                /*sandbox*/ None,
            )
            .await
            .is_err()
    );
    assert!(
        directory_junction
            .symlink_metadata()?
            .file_type()
            .is_symlink()
    );

    let sandbox = workspace_write_sandbox(tmp.path().to_path_buf());
    let read_result = context
        .file_system
        .read_file(&uri(&existing)?, no_follow_read, Some(&sandbox))
        .await;
    if is_unsupported_restricted_token_host(&read_result) {
        return Ok(());
    }
    assert_eq!(read_result?, b"unchanged");
    assert!(
        context
            .file_system
            .read_file(
                &uri(&directory_junction.join("existing.txt"))?,
                no_follow_read,
                Some(&sandbox),
            )
            .await
            .is_err()
    );
    assert!(
        context
            .file_system
            .write_file(
                &uri(&directory_junction.join("existing.txt"))?,
                b"changed".to_vec(),
                no_follow_write,
                Some(&sandbox),
            )
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(&existing)?, "unchanged");
    assert!(
        context
            .file_system
            .get_metadata(
                &uri(&directory_junction)?,
                no_follow_metadata,
                Some(&sandbox),
            )
            .await
            .is_err()
    );
    assert!(
        context
            .file_system
            .create_directory(
                &uri(&directory_junction.join("sandbox-created"))?,
                no_follow_create,
                Some(&sandbox),
            )
            .await
            .is_err()
    );
    assert!(!real.join("sandbox-created").exists());
    assert!(
        context
            .file_system
            .remove(
                &uri(&directory_junction.join("removable.txt"))?,
                no_follow_remove,
                Some(&sandbox),
            )
            .await
            .is_err()
    );
    assert!(removable.exists());

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_no_follow_operations_reject_named_pipes(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let pipe_name = format!("codex-fs-no-follow-{}", Uuid::new_v4());
    let server_path = format!(r"\\.\pipe\{pipe_name}");
    let client_path = format!(r"\\localhost\pipe\{pipe_name}");
    let _pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&server_path)?;

    let error = timeout(
        Duration::from_secs(1),
        context.file_system.read_file(
            &PathUri::from_host_native_path(Path::new(&client_path))?,
            ReadFileOptions {
                follow_symlinks: false,
            },
            /*sandbox*/ None,
        ),
    )
    .await
    .expect("strict named-pipe read must not hang")
    .expect_err("strict named-pipe read must be rejected");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);

    let pipe_name = format!("codex-fs-no-follow-write-{}", Uuid::new_v4());
    let server_path = format!(r"\\.\pipe\{pipe_name}");
    let client_path = format!(r"\\localhost\pipe\{pipe_name}");
    let _pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&server_path)?;
    timeout(
        Duration::from_secs(1),
        context.file_system.write_file(
            &PathUri::from_host_native_path(Path::new(&client_path))?,
            b"must not be written".to_vec(),
            WriteFileOptions {
                follow_symlinks: false,
            },
            /*sandbox*/ None,
        ),
    )
    .await
    .expect("strict named-pipe write must not hang")
    .expect_err("strict named-pipe write must be rejected");
    Ok(())
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
        .read_file(
            &PathUri::from_host_native_path(&readable_file)?,
            ReadFileOptions::default(),
            Some(&sandbox),
        )
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
            &PathUri::from_host_native_path(&blocked_file)?,
            b"blocked".to_vec(),
            WriteFileOptions::default(),
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
        PathUri::from_host_native_path(cwd)?,
    )?)
}
