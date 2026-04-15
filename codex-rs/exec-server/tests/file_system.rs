#![cfg(unix)]

mod common;

use std::os::unix::fs::symlink;
use std::process::Command;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::Environment;
use codex_exec_server::ExecServerRuntimePaths;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::LocalFileSystem;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_protocol::protocol::ReadOnlyAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use test_case::test_case;

use common::exec_server::ExecServerHarness;
use common::exec_server::TestCodexHelperPaths;
use common::exec_server::exec_server;
use common::exec_server::test_codex_helper_paths;

struct FileSystemContext {
    file_system: Arc<dyn ExecutorFileSystem>,
    _helper_paths: Option<TestCodexHelperPaths>,
    _server: Option<ExecServerHarness>,
}

async fn create_file_system_context(use_remote: bool) -> Result<FileSystemContext> {
    if use_remote {
        let server = exec_server().await?;
        let environment = Environment::create(Some(server.websocket_url().to_string())).await?;
        Ok(FileSystemContext {
            file_system: environment.get_filesystem(),
            _helper_paths: None,
            _server: Some(server),
        })
    } else {
        let helper_paths = test_codex_helper_paths()?;
        let runtime_paths = ExecServerRuntimePaths::new(
            helper_paths.codex_exe.clone(),
            helper_paths.codex_linux_sandbox_exe.clone(),
        )?;
        Ok(FileSystemContext {
            file_system: Arc::new(LocalFileSystem::with_runtime_paths(runtime_paths)),
            _helper_paths: Some(helper_paths),
            _server: None,
        })
    }
}

fn absolute_path(path: std::path::PathBuf) -> AbsolutePathBuf {
    assert!(
        path.is_absolute(),
        "path must be absolute: {}",
        path.display()
    );
    match AbsolutePathBuf::try_from(path) {
        Ok(path) => path,
        Err(err) => panic!("path should be absolute: {err}"),
    }
}

fn read_only_sandbox(readable_root: std::path::PathBuf) -> FileSystemSandboxContext {
    FileSystemSandboxContext::new(SandboxPolicy::ReadOnly {
        access: ReadOnlyAccess::Restricted {
            include_platform_defaults: false,
            readable_roots: vec![absolute_path(readable_root)],
        },
        network_access: false,
    })
}

fn workspace_write_sandbox(writable_root: std::path::PathBuf) -> FileSystemSandboxContext {
    FileSystemSandboxContext::new(SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![absolute_path(writable_root)],
        read_only_access: ReadOnlyAccess::Restricted {
            include_platform_defaults: false,
            readable_roots: vec![],
        },
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    })
}

fn assert_sandbox_denied(error: &std::io::Error) {
    assert!(
        matches!(
            error.kind(),
            std::io::ErrorKind::InvalidInput | std::io::ErrorKind::PermissionDenied
        ),
        "unexpected sandbox error kind: {error:?}",
    );
    let message = error.to_string();
    assert!(
        message.contains("is not permitted")
            || message.contains("Operation not permitted")
            || message.contains("Permission denied"),
        "unexpected sandbox error message: {message}",
    );
}

fn assert_normalized_path_rejected(error: &std::io::Error) {
    match error.kind() {
        std::io::ErrorKind::NotFound => assert!(
            error.to_string().contains("No such file or directory"),
            "unexpected not-found message: {error}",
        ),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::PermissionDenied => {
            let message = error.to_string();
            assert!(
                message.contains("is not permitted")
                    || message.contains("Operation not permitted")
                    || message.contains("Permission denied"),
                "unexpected rejection message: {message}",
            );
        }
        other => panic!("unexpected normalized-path error kind: {other:?}: {error:?}"),
    }
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_get_metadata_returns_expected_fields(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let file_path = tmp.path().join("note.txt");
    std::fs::write(&file_path, "hello")?;

    let metadata = file_system
        .get_metadata(&absolute_path(file_path.clone()), /*sandbox*/ None)
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(metadata.is_directory, false);
    assert_eq!(metadata.is_file, true);
    assert_eq!(metadata.is_symlink, false);
    assert!(metadata.modified_at_ms > 0);

    let symlink_path = tmp.path().join("note-link.txt");
    symlink(&file_path, &symlink_path)?;
    let symlink_metadata = file_system
        .get_metadata(&absolute_path(symlink_path.clone()), /*sandbox*/ None)
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(symlink_metadata.is_directory, false);
    assert_eq!(symlink_metadata.is_file, true);
    assert_eq!(symlink_metadata.is_symlink, true);
    assert!(symlink_metadata.modified_at_ms > 0);

    let dir_path = tmp.path().join("notes");
    std::fs::create_dir(&dir_path)?;
    let dir_symlink_path = tmp.path().join("notes-link");
    symlink(&dir_path, &dir_symlink_path)?;
    let dir_symlink_metadata = file_system
        .get_metadata(&absolute_path(dir_symlink_path), /*sandbox*/ None)
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(dir_symlink_metadata.is_directory, true);
    assert_eq!(dir_symlink_metadata.is_file, false);
    assert_eq!(dir_symlink_metadata.is_symlink, true);

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_methods_cover_surface_area(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    let nested_dir = source_dir.join("nested");
    let source_file = source_dir.join("root.txt");
    let nested_file = nested_dir.join("note.txt");
    let copied_dir = tmp.path().join("copied");
    let copied_file = tmp.path().join("copy.txt");

    file_system
        .create_directory(
            &absolute_path(nested_dir.clone()),
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;

    file_system
        .write_file(
            &absolute_path(nested_file.clone()),
            b"hello from trait".to_vec(),
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    file_system
        .write_file(
            &absolute_path(source_file.clone()),
            b"hello from source root".to_vec(),
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;

    let nested_file_contents = file_system
        .read_file(&absolute_path(nested_file.clone()), /*sandbox*/ None)
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(nested_file_contents, b"hello from trait");

    let nested_file_text = file_system
        .read_file_text(&absolute_path(nested_file.clone()), /*sandbox*/ None)
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(nested_file_text, "hello from trait");

    file_system
        .copy(
            &absolute_path(nested_file),
            &absolute_path(copied_file.clone()),
            CopyOptions { recursive: false },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(std::fs::read_to_string(copied_file)?, "hello from trait");

    file_system
        .copy(
            &absolute_path(source_dir.clone()),
            &absolute_path(copied_dir.clone()),
            CopyOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(
        std::fs::read_to_string(copied_dir.join("nested").join("note.txt"))?,
        "hello from trait"
    );

    symlink(
        source_dir.join("missing-target"),
        source_dir.join("broken-link"),
    )?;

    let mut entries = file_system
        .read_directory(&absolute_path(source_dir), /*sandbox*/ None)
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    assert_eq!(
        entries,
        vec![
            ReadDirectoryEntry {
                file_name: "nested".to_string(),
                is_directory: true,
                is_file: false,
            },
            ReadDirectoryEntry {
                file_name: "root.txt".to_string(),
                is_directory: false,
                is_file: true,
            },
        ]
    );

    file_system
        .remove(
            &absolute_path(copied_dir.clone()),
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert!(!copied_dir.exists());

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_write_file_reports_missing_parent(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let missing_parent_path = tmp.path().join("missing").join("note.txt");

    let error = match file_system
        .write_file(
            &absolute_path(missing_parent_path.clone()),
            b"hello from trait".to_vec(),
            /*sandbox*/ None,
        )
        .await
    {
        Ok(()) => anyhow::bail!("write should fail when parent directory is absent"),
        Err(error) => error,
    };
    assert_eq!(
        error.kind(),
        std::io::ErrorKind::NotFound,
        "mode={use_remote}"
    );
    assert!(!missing_parent_path.exists(), "mode={use_remote}");

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_directory_without_recursive(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    std::fs::create_dir_all(&source_dir)?;

    let error = file_system
        .copy(
            &absolute_path(source_dir),
            &absolute_path(tmp.path().join("dest")),
            CopyOptions { recursive: false },
            /*sandbox*/ None,
        )
        .await;
    let error = match error {
        Ok(()) => panic!("copy should fail"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        error.to_string(),
        "fs/copy requires recursive: true when sourcePath is a directory"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_read_allows_readable_root() -> Result<()> {
    let context = create_file_system_context(/*use_remote*/ false).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let file_path = allowed_dir.join("note.txt");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::write(&file_path, "sandboxed hello")?;
    let sandbox = read_only_sandbox(allowed_dir);

    let contents = file_system
        .read_file(&absolute_path(file_path), Some(&sandbox))
        .await?;
    assert_eq!(contents, b"sandboxed hello");

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_write_rejects_unwritable_path(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let blocked_path = tmp.path().join("blocked.txt");
    std::fs::create_dir_all(&allowed_dir)?;

    let sandbox = read_only_sandbox(allowed_dir);
    let error = match file_system
        .write_file(
            &absolute_path(blocked_path.clone()),
            b"nope".to_vec(),
            Some(&sandbox),
        )
        .await
    {
        Ok(()) => anyhow::bail!("write should be blocked"),
        Err(error) => error,
    };
    assert_sandbox_denied(&error);
    assert!(!blocked_path.exists());

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_read_rejects_symlink_escape(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;
    std::fs::write(outside_dir.join("secret.txt"), "nope")?;
    symlink(&outside_dir, allowed_dir.join("link"))?;

    let requested_path = allowed_dir.join("link").join("secret.txt");
    let sandbox = read_only_sandbox(allowed_dir);
    let error = match file_system
        .read_file(&absolute_path(requested_path.clone()), Some(&sandbox))
        .await
    {
        Ok(_) => anyhow::bail!("read should be blocked"),
        Err(error) => error,
    };
    assert_sandbox_denied(&error);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_read_rejects_symlink_parent_dotdot_escape() -> Result<()> {
    let context = create_file_system_context(/*use_remote*/ false).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    let secret_path = tmp.path().join("secret.txt");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;
    std::fs::write(&secret_path, "nope")?;
    symlink(&outside_dir, allowed_dir.join("link"))?;

    let requested_path = absolute_path(allowed_dir.join("link").join("..").join("secret.txt"));
    let sandbox = read_only_sandbox(allowed_dir);
    let error = match file_system.read_file(&requested_path, Some(&sandbox)).await {
        Ok(_) => anyhow::bail!("read should fail after path normalization"),
        Err(error) => error,
    };
    // AbsolutePathBuf normalizes `link/../secret.txt` to `allowed/secret.txt`
    // before the request reaches the filesystem layer. Depending on whether
    // the platform/runtime resolves that normalized path through a top-level
    // symlink alias, the request can surface as either "missing file" or an
    // upfront sandbox rejection.
    assert_normalized_path_rejected(&error);

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_write_rejects_symlink_escape(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;
    symlink(&outside_dir, allowed_dir.join("link"))?;

    let requested_path = allowed_dir.join("link").join("blocked.txt");
    let sandbox = workspace_write_sandbox(allowed_dir);
    let error = match file_system
        .write_file(
            &absolute_path(requested_path.clone()),
            b"nope".to_vec(),
            Some(&sandbox),
        )
        .await
    {
        Ok(()) => anyhow::bail!("write should be blocked"),
        Err(error) => error,
    };
    assert_sandbox_denied(&error);
    assert!(!outside_dir.join("blocked.txt").exists());

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_create_directory_rejects_symlink_escape(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;
    symlink(&outside_dir, allowed_dir.join("link"))?;

    let requested_path = allowed_dir.join("link").join("created");
    let sandbox = workspace_write_sandbox(allowed_dir);
    let error = match file_system
        .create_directory(
            &absolute_path(requested_path.clone()),
            CreateDirectoryOptions { recursive: false },
            Some(&sandbox),
        )
        .await
    {
        Ok(()) => anyhow::bail!("create_directory should be blocked"),
        Err(error) => error,
    };
    assert_sandbox_denied(&error);
    assert!(!outside_dir.join("created").exists());

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_get_metadata_rejects_symlink_escape(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;
    std::fs::write(outside_dir.join("secret.txt"), "nope")?;
    symlink(&outside_dir, allowed_dir.join("link"))?;

    let requested_path = allowed_dir.join("link").join("secret.txt");
    let sandbox = read_only_sandbox(allowed_dir);
    let error = match file_system
        .get_metadata(&absolute_path(requested_path.clone()), Some(&sandbox))
        .await
    {
        Ok(_) => anyhow::bail!("get_metadata should be blocked"),
        Err(error) => error,
    };
    assert_sandbox_denied(&error);

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_read_directory_rejects_symlink_escape(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;
    std::fs::write(outside_dir.join("secret.txt"), "nope")?;
    symlink(&outside_dir, allowed_dir.join("link"))?;

    let requested_path = allowed_dir.join("link");
    let sandbox = read_only_sandbox(allowed_dir);
    let error = match file_system
        .read_directory(&absolute_path(requested_path.clone()), Some(&sandbox))
        .await
    {
        Ok(_) => anyhow::bail!("read_directory should be blocked"),
        Err(error) => error,
    };
    assert_sandbox_denied(&error);

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_symlink_escape_destination(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;
    std::fs::write(allowed_dir.join("source.txt"), "hello")?;
    symlink(&outside_dir, allowed_dir.join("link"))?;

    let requested_destination = allowed_dir.join("link").join("copied.txt");
    let sandbox = workspace_write_sandbox(allowed_dir.clone());
    let error = match file_system
        .copy(
            &absolute_path(allowed_dir.join("source.txt")),
            &absolute_path(requested_destination.clone()),
            CopyOptions { recursive: false },
            Some(&sandbox),
        )
        .await
    {
        Ok(()) => anyhow::bail!("copy should be blocked"),
        Err(error) => error,
    };
    assert_sandbox_denied(&error);
    assert!(!outside_dir.join("copied.txt").exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_remove_removes_symlink_not_target() -> Result<()> {
    let context = create_file_system_context(/*use_remote*/ false).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    let outside_file = outside_dir.join("keep.txt");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;
    std::fs::write(&outside_file, "outside")?;
    let symlink_path = allowed_dir.join("link");
    symlink(&outside_file, &symlink_path)?;

    let sandbox = workspace_write_sandbox(allowed_dir);
    file_system
        .remove(
            &absolute_path(symlink_path.clone()),
            RemoveOptions {
                recursive: false,
                force: false,
            },
            Some(&sandbox),
        )
        .await?;

    assert!(!symlink_path.exists());
    assert!(outside_file.exists());
    assert_eq!(std::fs::read_to_string(outside_file)?, "outside");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_preserves_symlink_source() -> Result<()> {
    let context = create_file_system_context(/*use_remote*/ false).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    let outside_file = outside_dir.join("outside.txt");
    let source_symlink = allowed_dir.join("link");
    let copied_symlink = allowed_dir.join("copied-link");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;
    std::fs::write(&outside_file, "outside")?;
    symlink(&outside_file, &source_symlink)?;

    let sandbox = workspace_write_sandbox(allowed_dir.clone());
    file_system
        .copy(
            &absolute_path(source_symlink),
            &absolute_path(copied_symlink.clone()),
            CopyOptions { recursive: false },
            Some(&sandbox),
        )
        .await?;

    let copied_metadata = std::fs::symlink_metadata(&copied_symlink)?;
    assert!(copied_metadata.file_type().is_symlink());
    assert_eq!(std::fs::read_link(copied_symlink)?, outside_file);

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_remove_rejects_symlink_escape(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    let outside_file = outside_dir.join("secret.txt");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;
    std::fs::write(&outside_file, "outside")?;
    symlink(&outside_dir, allowed_dir.join("link"))?;

    let requested_path = allowed_dir.join("link").join("secret.txt");
    let sandbox = workspace_write_sandbox(allowed_dir);
    let error = match file_system
        .remove(
            &absolute_path(requested_path.clone()),
            RemoveOptions {
                recursive: false,
                force: false,
            },
            Some(&sandbox),
        )
        .await
    {
        Ok(()) => anyhow::bail!("remove should be blocked"),
        Err(error) => error,
    };
    assert_sandbox_denied(&error);
    assert_eq!(std::fs::read_to_string(outside_file)?, "outside");

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_symlink_escape_source(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    let outside_file = outside_dir.join("secret.txt");
    let requested_destination = allowed_dir.join("copied.txt");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;
    std::fs::write(&outside_file, "outside")?;
    symlink(&outside_dir, allowed_dir.join("link"))?;

    let requested_source = allowed_dir.join("link").join("secret.txt");
    let sandbox = workspace_write_sandbox(allowed_dir);
    let error = match file_system
        .copy(
            &absolute_path(requested_source.clone()),
            &absolute_path(requested_destination.clone()),
            CopyOptions { recursive: false },
            Some(&sandbox),
        )
        .await
    {
        Ok(()) => anyhow::bail!("copy should be blocked"),
        Err(error) => error,
    };
    assert_sandbox_denied(&error);
    assert!(!requested_destination.exists());

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_copying_directory_into_descendant(
    use_remote: bool,
) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    std::fs::create_dir_all(source_dir.join("nested"))?;

    let error = file_system
        .copy(
            &absolute_path(source_dir.clone()),
            &absolute_path(source_dir.join("nested").join("copy")),
            CopyOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await;
    let error = match error {
        Ok(()) => panic!("copy should fail"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        error.to_string(),
        "fs/copy cannot copy a directory to itself or one of its descendants"
    );

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_preserves_symlinks_in_recursive_copy(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    let nested_dir = source_dir.join("nested");
    let copied_dir = tmp.path().join("copied");
    std::fs::create_dir_all(&nested_dir)?;
    symlink("nested", source_dir.join("nested-link"))?;

    file_system
        .copy(
            &absolute_path(source_dir),
            &absolute_path(copied_dir.clone()),
            CopyOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;

    let copied_link = copied_dir.join("nested-link");
    let metadata = std::fs::symlink_metadata(&copied_link)?;
    assert!(metadata.file_type().is_symlink());
    assert_eq!(
        std::fs::read_link(copied_link)?,
        std::path::PathBuf::from("nested")
    );

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_ignores_unknown_special_files_in_recursive_copy(
    use_remote: bool,
) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    let copied_dir = tmp.path().join("copied");
    std::fs::create_dir_all(&source_dir)?;
    std::fs::write(source_dir.join("note.txt"), "hello")?;

    let fifo_path = source_dir.join("named-pipe");
    let output = Command::new("mkfifo").arg(&fifo_path).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "mkfifo failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    file_system
        .copy(
            &absolute_path(source_dir),
            &absolute_path(copied_dir.clone()),
            CopyOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;

    assert_eq!(
        std::fs::read_to_string(copied_dir.join("note.txt"))?,
        "hello"
    );
    assert!(!copied_dir.join("named-pipe").exists());

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_standalone_fifo_source(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let fifo_path = tmp.path().join("named-pipe");
    let output = Command::new("mkfifo").arg(&fifo_path).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "mkfifo failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let error = file_system
        .copy(
            &absolute_path(fifo_path),
            &absolute_path(tmp.path().join("copied")),
            CopyOptions { recursive: false },
            /*sandbox*/ None,
        )
        .await;
    let error = match error {
        Ok(()) => panic!("copy should fail"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        error.to_string(),
        "fs/copy only supports regular files, directories, and symlinks"
    );

    Ok(())
}
