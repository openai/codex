use anyhow::Context;
use anyhow::Result;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_sandboxing::policy_transforms::effective_file_system_sandbox_policy;
use codex_sandboxing::policy_transforms::effective_network_sandbox_policy;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use test_case::test_case;

use super::support::FileSystemImplementation;
use super::support::absolute_path;
use super::support::create_file_system_context;
use super::support::read_only_sandbox;
use super::support::workspace_write_sandbox;

#[test]
fn sandbox_context_from_profile_preserves_workspace_write_read_only_subpaths() -> Result<()> {
    let tmp = TempDir::new()?;
    let writable_dir = tmp.path().join("writable");
    let git_dir = writable_dir.join(".git");
    std::fs::create_dir_all(&git_dir)?;

    let sandbox = workspace_write_sandbox(writable_dir.clone());
    let policy = sandbox.permissions.file_system_sandbox_policy();
    let cwd = absolute_path(writable_dir.clone());
    let writable_roots = policy.get_writable_roots_with_cwd(cwd.as_path());
    let writable_dir = absolute_path(std::fs::canonicalize(writable_dir)?);
    let git_dir = absolute_path(std::fs::canonicalize(git_dir)?);
    let Some(writable_root) = writable_roots
        .iter()
        .find(|writable_root| writable_root.root == writable_dir)
    else {
        panic!("writable root should be preserved");
    };

    assert!(writable_root.read_only_subpaths.contains(&git_dir));

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_get_metadata_reports_files_and_directories(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let file_path = tmp.path().join("note.txt");
    let directory_path = tmp.path().join("notes");
    std::fs::write(&file_path, "hello")?;
    std::fs::create_dir(&directory_path)?;

    let file_metadata = file_system
        .get_metadata(&absolute_path(&file_path), /*sandbox*/ None)
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(file_metadata.is_directory, false);
    assert_eq!(file_metadata.is_file, true);
    assert_eq!(file_metadata.is_symlink, false);
    assert!(file_metadata.modified_at_ms > 0);

    let directory_metadata = file_system
        .get_metadata(&absolute_path(&directory_path), /*sandbox*/ None)
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(directory_metadata.is_directory, true);
    assert_eq!(directory_metadata.is_file, false);
    assert_eq!(directory_metadata.is_symlink, false);
    assert!(directory_metadata.modified_at_ms > 0);

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_create_directory_creates_nested_directories(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let nested_dir = tmp.path().join("source").join("nested");

    file_system
        .create_directory(
            &absolute_path(&nested_dir),
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert!(nested_dir.is_dir());

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_write_file_writes_bytes(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let file_path = tmp.path().join("note.txt");
    file_system
        .write_file(
            &absolute_path(&file_path),
            b"hello from trait".to_vec(),
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(std::fs::read(file_path)?, b"hello from trait");

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_join_and_parent_preserve_lexical_paths(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    let joined_nested = file_system
        .join(&absolute_path(&source_dir), Path::new("nested/note.txt"))
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(
        joined_nested,
        absolute_path(source_dir.join("nested").join("note.txt"))
    );
    let joined_parent = file_system
        .parent(&joined_nested)
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(
        joined_parent,
        Some(absolute_path(source_dir.join("nested")))
    );
    let joined_parent_traversal = file_system
        .join(&absolute_path(&source_dir), Path::new("../outside"))
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(
        joined_parent_traversal,
        absolute_path(source_dir.join("../outside"))
    );

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_read_file_returns_bytes(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let file_path = tmp.path().join("note.txt");
    std::fs::write(&file_path, "hello from trait")?;

    let contents = file_system
        .read_file(&absolute_path(&file_path), /*sandbox*/ None)
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(contents, b"hello from trait");

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_read_file_text_returns_string(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let file_path = tmp.path().join("note.txt");
    std::fs::write(&file_path, "hello from trait")?;

    let contents = file_system
        .read_file_text(&absolute_path(&file_path), /*sandbox*/ None)
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(contents, "hello from trait");

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_copies_file(implementation: FileSystemImplementation) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_file = tmp.path().join("source.txt");
    let copied_file = tmp.path().join("copy.txt");
    std::fs::write(&source_file, "hello from trait")?;

    file_system
        .copy(
            &absolute_path(&source_file),
            &absolute_path(&copied_file),
            CopyOptions { recursive: false },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(std::fs::read_to_string(copied_file)?, "hello from trait");

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_copies_directory_recursively(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    let nested_dir = source_dir.join("nested");
    let nested_file = nested_dir.join("note.txt");
    let copied_dir = tmp.path().join("copied");
    std::fs::create_dir_all(&nested_dir)?;
    std::fs::write(&nested_file, "hello from trait")?;

    file_system
        .copy(
            &absolute_path(&source_dir),
            &absolute_path(&copied_dir),
            CopyOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(
        std::fs::read_to_string(copied_dir.join("nested").join("note.txt"))?,
        "hello from trait"
    );

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_read_directory_lists_entries(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    std::fs::create_dir_all(source_dir.join("nested"))?;
    std::fs::write(source_dir.join("root.txt"), "hello")?;

    let mut entries = file_system
        .read_directory(&absolute_path(&source_dir), /*sandbox*/ None)
        .await
        .with_context(|| format!("mode={implementation}"))?;
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

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_remove_removes_directory(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let directory_path = tmp.path().join("remove-me");
    std::fs::create_dir_all(directory_path.join("nested"))?;

    file_system
        .remove(
            &absolute_path(&directory_path),
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert!(!directory_path.exists());

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_write_file_reports_missing_parent(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let missing_parent_path = tmp.path().join("missing").join("note.txt");

    let error = match file_system
        .write_file(
            &absolute_path(&missing_parent_path),
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
        "mode={implementation}"
    );
    assert!(!missing_parent_path.exists(), "mode={implementation}");

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_directory_without_recursive(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    std::fs::create_dir_all(&source_dir)?;

    let error = file_system
        .copy(
            &absolute_path(&source_dir),
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

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_read_allows_readable_root(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let allowed_dir = tmp.path().join("allowed");
    let file_path = allowed_dir.join("note.txt");
    std::fs::create_dir_all(&allowed_dir)?;
    std::fs::write(&file_path, "sandboxed hello")?;
    let sandbox = read_only_sandbox(allowed_dir);

    let contents = file_system
        .read_file(&absolute_path(&file_path), Some(&sandbox))
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(contents, b"sandboxed hello");

    Ok(())
}

pub(crate) async fn assert_canonicalize_resolves_directory_alias(
    implementation: FileSystemImplementation,
    create_directory_alias: impl FnOnce(&Path, &Path) -> Result<()>,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    let nested_dir = source_dir.join("nested");
    let file_path = nested_dir.join("note.txt");
    let alias_dir = tmp.path().join("source-alias");
    std::fs::create_dir_all(&nested_dir)?;
    std::fs::write(&file_path, "canonical hello")?;
    create_directory_alias(&source_dir, &alias_dir)?;

    let requested_path = absolute_path(alias_dir.join("nested").join("note.txt"));
    let expected_path = absolute_path(std::fs::canonicalize(&file_path)?);
    assert_ne!(requested_path, expected_path);

    let canonical_path = file_system
        .canonicalize(&requested_path, /*sandbox*/ None)
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(canonical_path, expected_path);

    Ok(())
}

pub(crate) async fn assert_sandboxed_canonicalize_resolves_directory_alias(
    implementation: FileSystemImplementation,
    create_directory_alias: impl FnOnce(&Path, &Path) -> Result<()>,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    let nested_dir = source_dir.join("nested");
    let file_path = nested_dir.join("note.txt");
    let alias_dir = tmp.path().join("source-alias");
    std::fs::create_dir_all(&nested_dir)?;
    std::fs::write(&file_path, "sandboxed canonical hello")?;
    create_directory_alias(&source_dir, &alias_dir)?;
    let sandbox = read_only_sandbox(tmp.path().to_path_buf());

    let requested_path = absolute_path(alias_dir.join("nested").join("note.txt"));
    let expected_path = absolute_path(std::fs::canonicalize(&file_path)?);
    assert_ne!(requested_path, expected_path);

    let canonical_path = file_system
        .canonicalize(&requested_path, Some(&sandbox))
        .await
        .with_context(|| format!("mode={implementation}"))?;
    assert_eq!(canonical_path, expected_path);

    Ok(())
}

/// Verifies that effective additional permissions extend a read-only sandbox with a writable root.
#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_sandboxed_write_allows_additional_write_root(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let readable_dir = tmp.path().join("readable");
    let writable_dir = tmp.path().join("writable");
    let file_path = writable_dir.join("note.txt");
    std::fs::create_dir_all(&readable_dir)?;
    std::fs::create_dir_all(&writable_dir)?;

    let mut sandbox = read_only_sandbox(readable_dir);
    let additional_permissions = AdditionalPermissionProfile {
        network: None,
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            /*read*/ None,
            Some(vec![absolute_path(writable_dir)]),
        )),
    };
    let file_system_policy = effective_file_system_sandbox_policy(
        &sandbox.permissions.file_system_sandbox_policy(),
        Some(&additional_permissions),
    );
    let network_policy = effective_network_sandbox_policy(
        sandbox.permissions.network_sandbox_policy(),
        Some(&additional_permissions),
    );
    sandbox.permissions = PermissionProfile::from_runtime_permissions_with_enforcement(
        sandbox.permissions.enforcement(),
        &file_system_policy,
        network_policy,
    );

    file_system
        .write_file(
            &absolute_path(&file_path),
            b"created".to_vec(),
            Some(&sandbox),
        )
        .await
        .with_context(|| format!("write file through additional root mode={implementation}"))?;
    assert_eq!(std::fs::read(&file_path)?, b"created");

    Ok(())
}

#[test_case(FileSystemImplementation::Local ; "local")]
#[test_case(FileSystemImplementation::Remote ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_copying_directory_into_descendant(
    implementation: FileSystemImplementation,
) -> Result<()> {
    let context = create_file_system_context(implementation).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    std::fs::create_dir_all(source_dir.join("nested"))?;

    let error = file_system
        .copy(
            &absolute_path(&source_dir),
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
