#![cfg(unix)]
#![allow(clippy::expect_used)]

mod common;
#[cfg(target_os = "linux")]
#[path = "common/fake_bwrap.rs"]
mod fake_bwrap;

#[path = "file_system/shared.rs"]
mod shared;
#[path = "file_system/support.rs"]
mod support;

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
#[cfg(target_os = "linux")]
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
#[cfg(target_os = "linux")]
use codex_exec_server::Environment;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::GetMetadataOptions;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::ReadFileOptions;
use codex_exec_server::RemoveOptions;
use codex_exec_server::WalkEntry;
use codex_exec_server::WalkEntryKind;
use codex_exec_server::WalkOptions;
use codex_exec_server::WalkOutcome;
use codex_exec_server::WriteFileOptions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use test_case::test_case;
use tokio::time::timeout;

#[cfg(target_os = "linux")]
use crate::common::exec_server::exec_server_with_env;
#[cfg(target_os = "linux")]
use crate::fake_bwrap::write_fake_bwrap;

use crate::support::FileSystemImplementation;
use crate::support::create_file_system_context;
use crate::support::read_only_sandbox;
use crate::support::workspace_write_sandbox;

fn assert_sandbox_denied(error: &std::io::Error) {
    match error.kind() {
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::PermissionDenied => {
            let message = error.to_string();
            assert!(
                message.contains("is not permitted")
                    || message.contains("Operation not permitted")
                    || message.contains("Permission denied"),
                "unexpected sandbox error message: {message}",
            );
        }
        std::io::ErrorKind::NotFound => assert!(
            error.to_string().contains("No such file or directory"),
            "unexpected sandbox not-found message: {error}",
        ),
        std::io::ErrorKind::Other => assert!(
            error.to_string().contains("Read-only file system"),
            "unexpected sandbox other error message: {error}",
        ),
        other => panic!("unexpected sandbox error kind: {other:?}: {error:?}"),
    }
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

fn alias_root_candidate() -> Result<Option<PathBuf>> {
    for root in [Path::new("/tmp").to_path_buf(), std::env::temp_dir()] {
        if root.is_dir() && root.canonicalize().is_ok_and(|canonical| canonical != root) {
            return Ok(Some(root));
        }
    }
    Ok(None)
}

fn create_directory_symlink(target: &Path, alias: &Path) -> Result<()> {
    symlink(target, alias)?;
    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_canonicalize_resolves_directory_symlink(
    implementation: FileSystemImplementation,
) -> Result<()> {
    shared::assert_canonicalize_resolves_directory_alias(implementation, create_directory_symlink)
        .await
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_operations_can_reject_symlinks_in_any_path_component(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let tmp = TempDir::new()?;
    let tmp_path = tmp.path().canonicalize()?;
    let real = tmp_path.join("real");
    std::fs::create_dir(&real)?;
    let existing = real.join("existing.txt");
    std::fs::write(&existing, "unchanged")?;
    let removable = real.join("removable.txt");
    std::fs::write(&removable, "keep")?;
    let directory_link = tmp_path.join("directory-link");
    symlink(&real, &directory_link)?;
    let file_link = tmp_path.join("file-link");
    symlink(&existing, &file_link)?;

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
    #[cfg(target_os = "linux")]
    let strict_sandbox = workspace_write_sandbox(tmp_path);
    #[cfg(target_os = "linux")]
    let sandboxes = [None, Some(&strict_sandbox)];
    #[cfg(not(target_os = "linux"))]
    let sandboxes: [Option<&FileSystemSandboxContext>; 1] = [None];

    for sandbox in sandboxes {
        assert!(
            context
                .file_system
                .read_file(&uri(&file_link)?, no_follow_read, sandbox)
                .await
                .is_err()
        );
        assert!(
            context
                .file_system
                .read_file(
                    &uri(&directory_link.join("existing.txt"))?,
                    no_follow_read,
                    sandbox,
                )
                .await
                .is_err()
        );
        assert!(
            context
                .file_system
                .write_file(
                    &uri(&file_link)?,
                    b"changed".to_vec(),
                    no_follow_write,
                    sandbox,
                )
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&existing)?, "unchanged");
        assert!(
            context
                .file_system
                .write_file(
                    &uri(&directory_link.join("existing.txt"))?,
                    b"changed".to_vec(),
                    no_follow_write,
                    sandbox,
                )
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&existing)?, "unchanged");
        assert!(
            context
                .file_system
                .get_metadata(&uri(&file_link)?, no_follow_metadata, sandbox)
                .await
                .is_err()
        );
        let directory_metadata = context
            .file_system
            .get_metadata(&uri(&real)?, no_follow_metadata, sandbox)
            .await?;
        assert!(directory_metadata.is_directory);
        assert!(
            context
                .file_system
                .create_directory(
                    &uri(&directory_link.join("created"))?,
                    no_follow_create,
                    sandbox,
                )
                .await
                .is_err()
        );
        assert!(!real.join("created").exists());
        assert!(
            context
                .file_system
                .remove(
                    &uri(&directory_link.join("removable.txt"))?,
                    no_follow_remove,
                    sandbox,
                )
                .await
                .is_err()
        );
        assert!(removable.exists());
        assert!(
            context
                .file_system
                .remove(&uri(&file_link)?, no_follow_remove, sandbox)
                .await
                .is_err()
        );
        assert!(file_link.symlink_metadata()?.file_type().is_symlink());
    }

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test]
async fn file_system_no_follow_non_recursive_root_creation_fails(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let result = context
        .file_system
        .create_directory(
            &PathUri::from_host_native_path(Path::new("/"))?,
            CreateDirectoryOptions {
                recursive: false,
                follow_symlinks: false,
            },
            /*sandbox*/ None,
        )
        .await;

    assert!(result.is_err());
    Ok(())
}

#[cfg(target_os = "linux")]
#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test]
async fn file_system_no_follow_metadata_preserves_linux_birthtime(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let tmp = TempDir::new()?;
    let file = tmp.path().join("created.txt");
    std::fs::write(&file, "created")?;
    let expected = match std::fs::metadata(&file)?.created() {
        Ok(created) => Some(i64::try_from(
            created.duration_since(UNIX_EPOCH)?.as_millis(),
        )?),
        Err(error) if error.kind() == std::io::ErrorKind::Unsupported => None,
        Err(error) => return Err(error.into()),
    };

    let metadata = context
        .file_system
        .get_metadata(
            &PathUri::from_host_native_path(&file)?,
            GetMetadataOptions {
                follow_symlinks: false,
            },
            /*sandbox*/ None,
        )
        .await?;

    if let Some(expected) = expected {
        assert!(expected > 0);
        assert_eq!(metadata.created_at_ms, expected);
    } else {
        assert_eq!(metadata.created_at_ms, 0);
    }
    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_no_follow_operations_support_search_only_ancestors(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let tmp = TempDir::new()?;
    let root = tmp.path().canonicalize()?;
    let search_only = root.join("search-only");
    std::fs::create_dir(&search_only)?;
    let existing = search_only.join("existing.txt");
    std::fs::write(&existing, "before")?;
    let unreadable = search_only.join("unreadable.txt");
    std::fs::write(&unreadable, "metadata only")?;
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000))?;
    let socket_path = search_only.join("socket");
    let _socket = UnixListener::bind(&socket_path)?;
    let removable = search_only.join("removable.txt");
    std::fs::write(&removable, "remove")?;
    std::fs::set_permissions(&search_only, std::fs::Permissions::from_mode(0o300))?;

    let uri = |path: &Path| PathUri::from_host_native_path(path);
    let result: Result<()> = async {
        let root_metadata = context
            .file_system
            .get_metadata(
                &uri(Path::new("/"))?,
                GetMetadataOptions {
                    follow_symlinks: false,
                },
                /*sandbox*/ None,
            )
            .await?;
        assert!(root_metadata.is_directory);

        assert_eq!(
            context
                .file_system
                .read_file(
                    &uri(&existing)?,
                    ReadFileOptions {
                        follow_symlinks: false,
                    },
                    /*sandbox*/ None,
                )
                .await?,
            b"before"
        );
        context
            .file_system
            .write_file(
                &uri(&existing)?,
                b"after".to_vec(),
                WriteFileOptions {
                    follow_symlinks: false,
                },
                /*sandbox*/ None,
            )
            .await?;
        assert_eq!(std::fs::read_to_string(&existing)?, "after");

        for metadata_path in [&unreadable, &socket_path] {
            context
                .file_system
                .get_metadata(
                    &uri(metadata_path)?,
                    GetMetadataOptions {
                        follow_symlinks: false,
                    },
                    /*sandbox*/ None,
                )
                .await?;
        }

        let nested = search_only.join("created").join("nested");
        context
            .file_system
            .create_directory(
                &uri(&nested)?,
                CreateDirectoryOptions {
                    recursive: true,
                    follow_symlinks: false,
                },
                /*sandbox*/ None,
            )
            .await?;
        assert!(nested.is_dir());

        context
            .file_system
            .remove(
                &uri(&removable)?,
                RemoveOptions {
                    recursive: false,
                    force: false,
                    follow_symlinks: false,
                },
                /*sandbox*/ None,
            )
            .await?;
        assert!(!removable.exists());
        Ok(())
    }
    .await;

    std::fs::set_permissions(&search_only, std::fs::Permissions::from_mode(0o700))?;
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o600))?;
    result
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_no_follow_write_rejects_fifo_without_blocking(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let tmp = TempDir::new()?;
    let fifo = tmp.path().canonicalize()?.join("fifo");
    let fifo_c = CString::new(fifo.as_os_str().as_bytes())?;
    if unsafe { libc::mkfifo(fifo_c.as_ptr(), 0o600) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    let result = timeout(
        Duration::from_secs(1),
        context.file_system.write_file(
            &PathUri::from_host_native_path(&fifo)?,
            b"must not be written".to_vec(),
            WriteFileOptions {
                follow_symlinks: false,
            },
            /*sandbox*/ None,
        ),
    )
    .await
    .context("strict FIFO write must not block")?;
    assert!(result.is_err());
    assert!(fifo.symlink_metadata()?.file_type().is_fifo());
    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn file_system_no_follow_recursive_mkdir_handles_concurrent_creators(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().canonicalize()?.join("shared").join("nested");
    let path_uri = PathUri::from_host_native_path(&path)?;
    let barrier = Arc::new(tokio::sync::Barrier::new(16));
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let file_system = Arc::clone(&context.file_system);
        let path_uri = path_uri.clone();
        let barrier = Arc::clone(&barrier);
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            file_system
                .create_directory(
                    &path_uri,
                    CreateDirectoryOptions {
                        recursive: true,
                        follow_symlinks: false,
                    },
                    /*sandbox*/ None,
                )
                .await
        }));
    }
    for task in tasks {
        task.await??;
    }
    assert!(path.is_dir());
    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_canonicalize_resolves_directory_symlink(
    implementation: FileSystemImplementation,
) -> Result<()> {
    shared::assert_sandboxed_canonicalize_resolves_directory_alias(
        implementation,
        create_directory_symlink,
    )
    .await
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sandboxed_file_system_helper_finds_bwrap_on_preserved_path() -> Result<()> {
    let tmp = TempDir::new()?;
    let fake_bin_dir = tmp.path().join("bin");
    let fake_bwrap = write_fake_bwrap(&fake_bin_dir)?;
    let mut path_entries = vec![fake_bin_dir];
    if let Some(path) = std::env::var_os("PATH") {
        path_entries.extend(std::env::split_paths(&path));
    }
    let helper_path = std::env::join_paths(path_entries)?;

    let server = exec_server_with_env([("PATH", helper_path.as_os_str())], &[]).await?;
    let environment = Environment::create_for_tests(Some(server.websocket_url().to_string()))?;
    let file_system = environment.get_filesystem();
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace)?;
    let file_path = workspace.join("created.txt");
    let sandbox = workspace_write_sandbox(workspace);

    file_system
        .write_file(
            &PathUri::from_host_native_path(&file_path)?,
            b"written through fs helper".to_vec(),
            Default::default(),
            Some(&sandbox),
        )
        .await?;

    assert_eq!(std::fs::read(&file_path)?, b"written through fs helper");

    let bwrap_log = fake_bwrap.with_file_name("bwrap.log");
    let log = std::fs::read_to_string(&bwrap_log)
        .with_context(|| format!("expected fake bwrap log at {}", bwrap_log.display()))?;
    assert!(
        log.contains("--argv0"),
        "expected fs helper sandbox path to invoke PATH bwrap with --argv0, got: {log}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_read_file_materializes_environment_workspace_roots() -> Result<()> {
    let context = create_file_system_context(FileSystemImplementation::Remote).await?;
    let file_system = context.file_system;
    let tmp = TempDir::new()?;
    let workspace = tmp.path().join("workspace");
    let workspace_file = workspace.join("included.txt");
    let excluded_file = tmp.path().join("excluded.txt");
    std::fs::create_dir(&workspace)?;
    std::fs::write(&workspace_file, b"included")?;
    std::fs::write(&excluded_file, b"excluded")?;

    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
        },
        access: FileSystemAccessMode::Read,
        missing_path_behavior: None,
    }]);
    let mut sandbox = FileSystemSandboxContext::from_permission_profile_with_cwd(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        PathUri::from_host_native_path(tmp.path())?,
    );
    sandbox.workspace_roots = vec![PathUri::from_host_native_path(&workspace)?];

    assert_eq!(
        file_system
            .read_file(
                &PathUri::from_host_native_path(&workspace_file)?,
                Default::default(),
                Some(&sandbox),
            )
            .await?,
        b"included"
    );
    let error = file_system
        .read_file(
            &PathUri::from_host_native_path(&excluded_file)?,
            Default::default(),
            Some(&sandbox),
        )
        .await
        .expect_err("read outside environment workspace roots should fail");
    assert_sandbox_denied(&error);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_read_file_preserves_empty_workspace_roots() -> Result<()> {
    let context = create_file_system_context(FileSystemImplementation::Remote).await?;
    let file_system = context.file_system;
    let tmp = TempDir::new()?;
    let file = tmp.path().join("excluded.txt");
    std::fs::write(&file, b"excluded")?;

    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
        },
        access: FileSystemAccessMode::Read,
        missing_path_behavior: None,
    }]);
    let mut sandbox = FileSystemSandboxContext::from_permission_profile_with_cwd(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        PathUri::from_host_native_path(tmp.path())?,
    );
    sandbox.workspace_roots.clear();

    let error = file_system
        .read_file(
            &PathUri::from_host_native_path(&file)?,
            Default::default(),
            Some(&sandbox),
        )
        .await
        .expect_err("empty workspace roots should not grant cwd access");
    assert_sandbox_denied(&error);

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_metadata_and_directory_listing_follow_symlinks(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let file_path = tmp.path().join("note.txt");
    std::fs::write(&file_path, "hello")?;
    let symlink_path = tmp.path().join("note-link.txt");
    symlink(&file_path, &symlink_path)?;
    let symlink_metadata = file_system
        .get_metadata(
            &PathUri::from_host_native_path(&symlink_path)?,
            Default::default(),
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(
        symlink_metadata,
        FileMetadata {
            is_directory: false,
            is_file: true,
            is_symlink: true,
            size: 5,
            created_at_ms: symlink_metadata.created_at_ms,
            modified_at_ms: symlink_metadata.modified_at_ms,
        }
    );
    assert!(symlink_metadata.modified_at_ms > 0);

    let dir_path = tmp.path().join("notes");
    std::fs::create_dir(&dir_path)?;
    let dir_symlink_path = tmp.path().join("notes-link");
    symlink(&dir_path, &dir_symlink_path)?;
    let dir_symlink_metadata = file_system
        .get_metadata(
            &PathUri::from_host_native_path(&dir_symlink_path)?,
            Default::default(),
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(
        dir_symlink_metadata,
        FileMetadata {
            is_directory: true,
            is_file: false,
            is_symlink: true,
            size: std::fs::metadata(&dir_path)?.len(),
            created_at_ms: dir_symlink_metadata.created_at_ms,
            modified_at_ms: dir_symlink_metadata.modified_at_ms,
        }
    );

    let dangling_symlink_path = tmp.path().join("dangling-link");
    symlink(tmp.path().join("missing"), &dangling_symlink_path)?;
    let error = file_system
        .get_metadata(
            &PathUri::from_host_native_path(&dangling_symlink_path)?,
            Default::default(),
            /*sandbox*/ None,
        )
        .await
        .expect_err("dangling symlink should not resolve");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);

    let mut entries = file_system
        .read_directory(
            &PathUri::from_host_native_path(tmp.path())?,
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;
    entries.retain(|entry| entry.file_name.contains("link"));
    entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));

    assert_eq!(
        entries,
        vec![
            ReadDirectoryEntry {
                file_name: "note-link.txt".to_string(),
                is_directory: false,
                is_file: true,
            },
            ReadDirectoryEntry {
                file_name: "notes-link".to_string(),
                is_directory: true,
                is_file: false,
            },
        ]
    );

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_walk_handles_directory_symlinks(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let root = tmp.path().join("root");
    let target = tmp.path().join("target");
    let target_file = target.join("note.txt");
    let target_link = root.join("target-link");
    let root_link = target.join("root-link");
    std::fs::create_dir_all(&root)?;
    std::fs::create_dir_all(&target)?;
    std::fs::write(&target_file, "target")?;
    symlink(&target, &target_link)?;
    symlink(&root, &root_link)?;
    symlink(&target_file, root.join("file-link"))?;
    symlink(root.join("missing"), root.join("broken-link"))?;

    for root in [&root, &root_link] {
        let target_link = root.join("target-link");

        let outcome = file_system
            .walk(
                &PathUri::from_host_native_path(root)?,
                WalkOptions {
                    max_depth: 2,
                    max_directories: 4,
                    max_entries: 8,
                    follow_directory_symlinks: false,
                    prune_hidden_directories: false,
                },
                /*sandbox*/ None,
            )
            .await
            .with_context(|| format!("mode={implementation}"))?;
        assert_eq!(
            outcome,
            WalkOutcome {
                entries: Vec::new(),
                errors: Vec::new(),
                truncated: false,
            }
        );

        let outcome = file_system
            .walk(
                &PathUri::from_host_native_path(root)?,
                WalkOptions {
                    max_depth: 2,
                    max_directories: 4,
                    max_entries: 8,
                    follow_directory_symlinks: true,
                    prune_hidden_directories: false,
                },
                /*sandbox*/ None,
            )
            .await
            .with_context(|| format!("mode={implementation}"))?;
        assert_eq!(
            outcome,
            WalkOutcome {
                entries: vec![
                    WalkEntry {
                        path: PathUri::from_host_native_path(&target_link)?,
                        kind: WalkEntryKind::Directory,
                    },
                    WalkEntry {
                        path: PathUri::from_host_native_path(target_link.join("note.txt"))?,
                        kind: WalkEntryKind::File,
                    },
                    WalkEntry {
                        path: PathUri::from_host_native_path(target_link.join("root-link"))?,
                        kind: WalkEntryKind::Directory,
                    },
                ],
                errors: Vec::new(),
                truncated: false,
            }
        );
    }

    Ok(())
}

#[cfg(target_os = "linux")]
#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_walk_reports_non_utf8_names(
    implementation: FileSystemImplementation,
) -> Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use codex_exec_server::WalkError;

    let context = create_file_system_context(implementation).await?;
    let tmp = TempDir::new()?;
    std::fs::write(
        tmp.path()
            .join(OsString::from_vec(b"invalid-\xff".to_vec())),
        "contents",
    )?;
    let lossy_path = tmp.path().join("invalid-\u{fffd}");
    let error =
        std::fs::symlink_metadata(&lossy_path).expect_err("the lossy filename must not exist");
    let outcome = context
        .file_system
        .walk(
            &PathUri::from_host_native_path(tmp.path())?,
            WalkOptions {
                max_depth: 0,
                max_directories: 1,
                max_entries: 1,
                follow_directory_symlinks: false,
                prune_hidden_directories: false,
            },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(
        outcome,
        WalkOutcome {
            entries: Vec::new(),
            errors: vec![WalkError {
                path: PathUri::from_host_native_path(lossy_path)?,
                message: error.to_string(),
            }],
            truncated: false,
        }
    );
    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_walk_prunes_hidden_directories_without_claiming_visible_aliases(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let root = tmp.path().join("root");
    let hidden = root.join(".hidden");
    let hidden_nested = hidden.join("nested");
    let visible = root.join("visible");
    std::fs::create_dir_all(&hidden_nested)?;
    std::fs::write(hidden_nested.join("note.txt"), "visible through alias")?;
    symlink(&hidden, &visible)?;

    let outcome = file_system
        .walk(
            &PathUri::from_host_native_path(&root)?,
            WalkOptions {
                max_depth: 3,
                max_directories: 3,
                max_entries: 6,
                follow_directory_symlinks: true,
                prune_hidden_directories: true,
            },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;

    assert_eq!(
        outcome,
        WalkOutcome {
            entries: vec![
                WalkEntry {
                    path: PathUri::from_host_native_path(hidden)?,
                    kind: WalkEntryKind::Directory,
                },
                WalkEntry {
                    path: PathUri::from_host_native_path(&visible)?,
                    kind: WalkEntryKind::Directory,
                },
                WalkEntry {
                    path: PathUri::from_host_native_path(visible.join("nested"))?,
                    kind: WalkEntryKind::Directory,
                },
                WalkEntry {
                    path: PathUri::from_host_native_path(visible.join("nested/note.txt"))?,
                    kind: WalkEntryKind::File,
                },
            ],
            errors: Vec::new(),
            truncated: false,
        }
    );

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_write_rejects_unwritable_path(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let blocked_path = tmp.path().join("blocked.txt");

    let sandbox = read_only_sandbox(tmp.path().to_path_buf());
    let error = match file_system
        .write_file(
            &PathUri::from_host_native_path(&blocked_path)?,
            b"nope".to_vec(),
            Default::default(),
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

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_write_allows_explicit_alias_roots(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let Some(alias_root) = alias_root_candidate()? else {
        return Ok(());
    };

    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = tempfile::Builder::new()
        .prefix("codex-fs-sandbox-alias-")
        .tempdir_in(&alias_root)?;
    let file_path = tmp.path().join("note.txt");
    let sandbox = workspace_write_sandbox(alias_root.clone());

    file_system
        .write_file(
            &PathUri::from_host_native_path(&file_path)?,
            b"created".to_vec(),
            Default::default(),
            Some(&sandbox),
        )
        .await
        .with_context(|| format!("write file through alias root mode={implementation}"))?;
    assert_eq!(std::fs::read(&file_path)?, b"created");

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_read_rejects_symlink_escape(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
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
        .read_file(
            &PathUri::from_host_native_path(&requested_path)?,
            Default::default(),
            Some(&sandbox),
        )
        .await
    {
        Ok(_) => anyhow::bail!("read should be blocked"),
        Err(error) => error,
    };
    assert_sandbox_denied(&error);

    let error = file_system
        .read_file_stream(
            &PathUri::from_host_native_path(&requested_path)?,
            Some(&sandbox),
        )
        .await
        .err()
        .context("streaming read should be blocked")?;
    assert_sandbox_denied(&error);

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_read_rejects_symlink_parent_dotdot_escape(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    let secret_path = tmp.path().join("secret.txt");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;
    std::fs::write(&secret_path, "nope")?;
    symlink(&outside_dir, allowed_dir.join("link"))?;

    let requested_path =
        PathUri::from_host_native_path(allowed_dir.join("link").join("..").join("secret.txt"))?;
    let sandbox = read_only_sandbox(allowed_dir);
    let error = match file_system
        .read_file(&requested_path, Default::default(), Some(&sandbox))
        .await
    {
        Ok(_) => anyhow::bail!("read should fail after path normalization"),
        Err(error) => error,
    };
    // PathUri's native path constructor normalizes `link/../secret.txt` to
    // `allowed/secret.txt` before the request reaches the filesystem layer.
    // Depending on whether the platform/runtime resolves that normalized path
    // through a top-level symlink alias, the request can surface as either
    // "missing file" or an upfront sandbox rejection.
    assert_normalized_path_rejected(&error);

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_write_rejects_symlink_escape(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
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
            &PathUri::from_host_native_path(&requested_path)?,
            b"nope".to_vec(),
            Default::default(),
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

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_write_preserves_existing_hard_link(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let outside_dir = tmp.path().join("outside");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::create_dir_all(&outside_dir)?;

    let outside_file = outside_dir.join("outside.txt");
    let hard_link = allowed_dir.join("hard-link.txt");
    std::fs::write(&outside_file, "outside\n")?;
    std::fs::hard_link(&outside_file, &hard_link)?;

    let sandbox = workspace_write_sandbox(allowed_dir);
    file_system
        .write_file(
            &PathUri::from_host_native_path(&hard_link)?,
            b"updated through existing hard link\n".to_vec(),
            Default::default(),
            Some(&sandbox),
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;

    assert_eq!(
        std::fs::read_to_string(&outside_file)?,
        "updated through existing hard link\n"
    );
    assert_eq!(
        std::fs::read_to_string(&hard_link)?,
        "updated through existing hard link\n"
    );

    let outside_metadata = std::fs::metadata(&outside_file)?;
    let link_metadata = std::fs::metadata(&hard_link)?;
    assert_eq!(
        (link_metadata.dev(), link_metadata.ino()),
        (outside_metadata.dev(), outside_metadata.ino())
    );

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_create_directory_rejects_symlink_escape(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
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
            &PathUri::from_host_native_path(&requested_path)?,
            CreateDirectoryOptions {
                recursive: false,
                follow_symlinks: true,
            },
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

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_read_directory_rejects_symlink_escape(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
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
        .read_directory(
            &PathUri::from_host_native_path(&requested_path)?,
            Some(&sandbox),
        )
        .await
    {
        Ok(_) => anyhow::bail!("read_directory should be blocked"),
        Err(error) => error,
    };
    assert_sandbox_denied(&error);

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_symlink_escape_destination(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
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
            &PathUri::from_host_native_path(allowed_dir.join("source.txt"))?,
            &PathUri::from_host_native_path(&requested_destination)?,
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

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_remove_removes_symlink_not_target(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
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
            &PathUri::from_host_native_path(&symlink_path)?,
            RemoveOptions {
                recursive: false,
                force: false,
                follow_symlinks: true,
            },
            Some(&sandbox),
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;

    assert!(!symlink_path.exists());
    assert!(outside_file.exists());
    assert_eq!(std::fs::read_to_string(outside_file)?, "outside");

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_preserves_symlink_source(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
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
            &PathUri::from_host_native_path(&source_symlink)?,
            &PathUri::from_host_native_path(&copied_symlink)?,
            CopyOptions { recursive: false },
            Some(&sandbox),
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;

    let copied_metadata = std::fs::symlink_metadata(&copied_symlink)?;
    assert!(copied_metadata.file_type().is_symlink());
    assert_eq!(std::fs::read_link(copied_symlink)?, outside_file);

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_remove_rejects_symlink_escape(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
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
            &PathUri::from_host_native_path(&requested_path)?,
            RemoveOptions {
                recursive: false,
                force: false,
                follow_symlinks: true,
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

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_symlink_escape_source(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
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
            &PathUri::from_host_native_path(&requested_source)?,
            &PathUri::from_host_native_path(&requested_destination)?,
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

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_preserves_symlinks_in_recursive_copy(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    let nested_dir = source_dir.join("nested");
    let copied_dir = tmp.path().join("copied");
    std::fs::create_dir_all(&nested_dir)?;
    symlink("nested", source_dir.join("nested-link"))?;

    file_system
        .copy(
            &PathUri::from_host_native_path(&source_dir)?,
            &PathUri::from_host_native_path(&copied_dir)?,
            CopyOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;

    let copied_link = copied_dir.join("nested-link");
    let metadata = std::fs::symlink_metadata(&copied_link)?;
    assert!(metadata.file_type().is_symlink());
    assert_eq!(
        std::fs::read_link(copied_link)?,
        std::path::PathBuf::from("nested")
    );

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_ignores_unknown_special_files_in_recursive_copy(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
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
            &PathUri::from_host_native_path(&source_dir)?,
            &PathUri::from_host_native_path(&copied_dir)?,
            CopyOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;

    assert_eq!(
        std::fs::read_to_string(copied_dir.join("note.txt"))?,
        "hello"
    );
    assert!(!copied_dir.join("named-pipe").exists());

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_standalone_fifo_source(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
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
            &PathUri::from_host_native_path(&fifo_path)?,
            &PathUri::from_host_native_path(tmp.path().join("copied"))?,
            CopyOptions { recursive: false },
            /*sandbox*/ None,
        )
        .await;
    let error = error.expect_err("copying a FIFO should fail");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        error.to_string(),
        "fs/copy only supports regular files, directories, and symlinks"
    );

    Ok(())
}
