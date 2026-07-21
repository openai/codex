#![cfg(unix)]
#![allow(clippy::expect_used)]

mod common;

#[path = "file_system/shared.rs"]
mod shared;
#[path = "file_system/support.rs"]
mod support;

use std::os::unix::fs::MetadataExt;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
#[cfg(target_os = "linux")]
use codex_exec_server::Environment;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::RemoveOptions;
use codex_exec_server::WalkEntry;
use codex_exec_server::WalkEntryKind;
use codex_exec_server::WalkOptions;
use codex_exec_server::WalkOutcome;
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

#[cfg(target_os = "linux")]
use crate::common::exec_server::exec_server_with_env;

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

#[cfg(target_os = "linux")]
fn write_fake_bwrap(bin_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(bin_dir)?;
    let fake_bwrap = bin_dir.join("bwrap");
    std::fs::write(
        &fake_bwrap,
        r#"#!/bin/bash
set -euo pipefail

for arg in "$@"; do
  if [[ "${arg}" == "--help" ]]; then
    echo "Usage: bwrap --argv0 --perms"
    exit 0
  fi
done

printf '%s\n' "$*" >> "${0}.log"

args=("$@")
argv0=""
command_start=-1
for i in "${!args[@]}"; do
  if [[ "${args[$i]}" == "--argv0" && $((i + 1)) -lt ${#args[@]} ]]; then
    argv0="${args[$((i + 1))]}"
  fi
  if [[ "${args[$i]}" == "--" ]]; then
    command_start=$((i + 1))
    break
  fi
done

if [[ "${command_start}" -lt 0 || "${command_start}" -ge "${#args[@]}" ]]; then
  echo "fake bwrap did not find an inner command" >&2
  exit 125
fi

cmd=("${args[@]:$command_start}")
if [[ -n "${argv0}" ]]; then
  exec -a "${argv0}" "${cmd[@]}"
fi
exec "${cmd[@]}"
"#,
    )?;
    let mut permissions = std::fs::metadata(&fake_bwrap)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&fake_bwrap, permissions)?;
    Ok(fake_bwrap)
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

    let server = exec_server_with_env([("PATH", helper_path.as_os_str())]).await?;
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
                Some(&sandbox),
            )
            .await?,
        b"included"
    );
    let error = file_system
        .read_file(
            &PathUri::from_host_native_path(&excluded_file)?,
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
        .read_file(&PathUri::from_host_native_path(&file)?, Some(&sandbox))
        .await
        .expect_err("empty workspace roots should not grant cwd access");
    assert_sandbox_denied(&error);

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_get_metadata_reports_symlink_targets(
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

    let outcome = file_system
        .walk(
            &PathUri::from_host_native_path(&root)?,
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
            &PathUri::from_host_native_path(&root)?,
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
            Some(&sandbox),
        )
        .await
    {
        Ok(_) => anyhow::bail!("read should be blocked"),
        Err(error) => error,
    };
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
    let error = match file_system.read_file(&requested_path, Some(&sandbox)).await {
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
