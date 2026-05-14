use codex_file_system::CopyOptions;
use codex_file_system::CreateDirectoryOptions;
use codex_file_system::ExecutorFileSystem;
use codex_file_system::FileSystemSandboxContext;
use codex_file_system::InMemoryFileSystem;
use codex_file_system::RemoveOptions;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use std::io;
use std::path::PathBuf;

fn path(unix_path: &str) -> AbsolutePathBuf {
    AbsolutePathBuf::try_from(test_path_buf(unix_path))
        .unwrap_or_else(|err| panic!("test path should be absolute: {err}"))
}

fn path_buf(path: PathBuf) -> AbsolutePathBuf {
    AbsolutePathBuf::try_from(path).unwrap_or_else(|err| panic!("path should be absolute: {err}"))
}

fn sandbox_context() -> FileSystemSandboxContext {
    FileSystemSandboxContext::from_permission_profile(PermissionProfile::default())
}

#[tokio::test]
async fn read_write_and_inspection_work_for_virtual_file_paths() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let root = path("/virtual/project");
    let file = path("/virtual/project/notes.txt");

    fs.seed_directory(&root)?;
    assert!(fs.exists(&root));

    fs.write_file(&file, b"hello".to_vec(), /*sandbox*/ None)
        .await?;

    assert_eq!(fs.read_file(&file, /*sandbox*/ None).await?, b"hello");
    assert_eq!(fs.file_contents(&file), Some(b"hello".to_vec()));
    assert!(fs.exists(&file));

    Ok(())
}

#[tokio::test]
async fn reads_do_not_fall_back_to_host_files() -> io::Result<()> {
    let tempdir = tempfile::tempdir()?;
    let host_file = tempdir.path().join("host-only.txt");
    std::fs::write(&host_file, "host contents")?;
    let host_file = path_buf(host_file);

    let err = InMemoryFileSystem::new()
        .read_file(&host_file, /*sandbox*/ None)
        .await
        .expect_err("host-only file should be invisible");

    assert_eq!(err.kind(), io::ErrorKind::NotFound);
    Ok(())
}

#[tokio::test]
async fn root_paths_exist_without_explicit_seeding() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let root = path("/");

    assert!(fs.exists(&root));
    assert_eq!(
        fs.get_metadata(&root, /*sandbox*/ None).await?,
        codex_file_system::FileMetadata {
            is_directory: true,
            is_file: false,
            is_symlink: false,
            created_at_ms: 0,
            modified_at_ms: 0,
        }
    );
    assert_eq!(
        fs.read_directory(&root, /*sandbox*/ None).await?,
        Vec::new()
    );

    Ok(())
}

#[tokio::test]
async fn directory_metadata_and_read_directory_are_deterministic() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let dir = path("/virtual/project");

    fs.seed_file(&path("/virtual/project/zeta.txt"), b"z".to_vec())?;
    fs.seed_directory(&path("/virtual/project/alpha"))?;
    fs.seed_file(&path("/virtual/project/middle.txt"), b"m".to_vec())?;

    let metadata = fs.get_metadata(&dir, /*sandbox*/ None).await?;
    assert_eq!(metadata.is_directory, true);
    assert_eq!(metadata.is_file, false);
    assert_eq!(metadata.is_symlink, false);

    let entries = fs.read_directory(&dir, /*sandbox*/ None).await?;
    assert_eq!(
        entries,
        vec![
            codex_file_system::ReadDirectoryEntry {
                file_name: "alpha".to_string(),
                is_directory: true,
                is_file: false,
            },
            codex_file_system::ReadDirectoryEntry {
                file_name: "middle.txt".to_string(),
                is_directory: false,
                is_file: true,
            },
            codex_file_system::ReadDirectoryEntry {
                file_name: "zeta.txt".to_string(),
                is_directory: false,
                is_file: true,
            },
        ]
    );

    Ok(())
}

#[tokio::test]
async fn read_directory_only_returns_immediate_children() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let dir = path("/virtual/project");

    fs.seed_file(&path("/virtual/project/nested/deep.txt"), b"deep".to_vec())?;
    fs.seed_file(&path("/virtual/project/top.txt"), b"top".to_vec())?;

    assert_eq!(
        fs.read_directory(&dir, /*sandbox*/ None).await?,
        vec![
            codex_file_system::ReadDirectoryEntry {
                file_name: "nested".to_string(),
                is_directory: true,
                is_file: false,
            },
            codex_file_system::ReadDirectoryEntry {
                file_name: "top.txt".to_string(),
                is_directory: false,
                is_file: true,
            },
        ]
    );

    Ok(())
}

#[tokio::test]
async fn create_directory_honors_recursive_behavior() {
    let fs = InMemoryFileSystem::new();
    let deep_dir = path("/virtual/project/a/b");

    let err = fs
        .create_directory(
            &deep_dir,
            CreateDirectoryOptions { recursive: false },
            /*sandbox*/ None,
        )
        .await
        .expect_err("non-recursive create should require existing parent");
    assert_eq!(err.kind(), io::ErrorKind::NotFound);

    fs.create_directory(
        &deep_dir,
        CreateDirectoryOptions { recursive: true },
        /*sandbox*/ None,
    )
    .await
    .expect("recursive create should build ancestors");

    assert!(fs.exists(&path("/virtual/project/a")));
    assert!(fs.exists(&deep_dir));
}

#[tokio::test]
async fn create_directory_handles_existing_entries() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let dir = path("/virtual/project");

    fs.seed_directory(&dir)?;
    fs.create_directory(
        &dir,
        CreateDirectoryOptions { recursive: true },
        /*sandbox*/ None,
    )
    .await?;

    let err = fs
        .create_directory(
            &dir,
            CreateDirectoryOptions { recursive: false },
            /*sandbox*/ None,
        )
        .await
        .expect_err("non-recursive create should reject existing directory");
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);

    fs.seed_file(&path("/virtual/project/file.txt"), b"file".to_vec())?;
    let err = fs
        .create_directory(
            &path("/virtual/project/file.txt"),
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await
        .expect_err("directory create should reject existing file");
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);

    Ok(())
}

#[tokio::test]
async fn write_file_requires_existing_parent_directory() {
    let fs = InMemoryFileSystem::new();
    let err = fs
        .write_file(
            &path("/virtual/project/missing/notes.txt"),
            b"hello".to_vec(),
            /*sandbox*/ None,
        )
        .await
        .expect_err("write should not create arbitrary parent directories");
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
}

#[tokio::test]
async fn file_writes_preserve_created_at_and_update_modified_at() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let file = path("/virtual/project/notes.txt");

    fs.seed_directory(&path("/virtual/project"))?;
    fs.write_file(&file, b"first".to_vec(), /*sandbox*/ None)
        .await?;
    let first = fs.get_metadata(&file, /*sandbox*/ None).await?;

    fs.write_file(&file, b"second".to_vec(), /*sandbox*/ None)
        .await?;
    let second = fs.get_metadata(&file, /*sandbox*/ None).await?;

    assert_eq!(second.created_at_ms, first.created_at_ms);
    assert!(second.modified_at_ms > first.modified_at_ms);
    assert_eq!(fs.file_contents(&file), Some(b"second".to_vec()));

    Ok(())
}

#[tokio::test]
async fn write_and_seed_file_reject_directory_targets() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let dir = path("/virtual/project");

    fs.seed_directory(&dir)?;

    let err = fs
        .write_file(&dir, b"contents".to_vec(), /*sandbox*/ None)
        .await
        .expect_err("write should reject directory target");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    let err = fs
        .seed_file(&dir, b"contents".to_vec())
        .expect_err("seed should reject directory target");
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);

    Ok(())
}

#[tokio::test]
async fn read_file_and_read_directory_reject_wrong_entry_kinds() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let dir = path("/virtual/project");
    let file = path("/virtual/project/notes.txt");

    fs.seed_file(&file, b"hello".to_vec())?;

    let err = fs
        .read_file(&dir, /*sandbox*/ None)
        .await
        .expect_err("reading a directory as a file should fail");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    let err = fs
        .read_directory(&file, /*sandbox*/ None)
        .await
        .expect_err("reading a file as a directory should fail");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    Ok(())
}

#[tokio::test]
async fn read_file_text_rejects_invalid_utf8() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let file = path("/virtual/project/bytes.bin");

    fs.seed_file(&file, vec![0xff])?;
    let err = fs
        .read_file_text(&file, /*sandbox*/ None)
        .await
        .expect_err("invalid utf8 should fail");

    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    Ok(())
}

#[tokio::test]
async fn remove_honors_recursive_and_force_behavior() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let dir = path("/virtual/project");

    fs.seed_file(&path("/virtual/project/child.txt"), b"c".to_vec())?;

    let err = fs
        .remove(
            &dir,
            RemoveOptions {
                recursive: false,
                force: false,
            },
            /*sandbox*/ None,
        )
        .await
        .expect_err("non-recursive remove should reject non-empty directories");
    assert_eq!(err.kind(), io::ErrorKind::DirectoryNotEmpty);

    fs.remove(
        &dir,
        RemoveOptions {
            recursive: true,
            force: false,
        },
        /*sandbox*/ None,
    )
    .await?;
    assert!(!fs.exists(&dir));

    fs.remove(
        &path("/virtual/project/missing.txt"),
        RemoveOptions {
            recursive: false,
            force: true,
        },
        /*sandbox*/ None,
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn remove_supports_files_and_rejects_root() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let file = path("/virtual/project/file.txt");

    fs.seed_file(&file, b"payload".to_vec())?;
    fs.remove(
        &file,
        RemoveOptions {
            recursive: false,
            force: false,
        },
        /*sandbox*/ None,
    )
    .await?;
    assert!(!fs.exists(&file));

    let err = fs
        .remove(
            &path("/"),
            RemoveOptions {
                recursive: true,
                force: false,
            },
            /*sandbox*/ None,
        )
        .await
        .expect_err("root removal should fail");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    Ok(())
}

#[tokio::test]
async fn copy_supports_files_and_recursive_directories() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let source_dir = path("/virtual/source");
    let source_file = path("/virtual/source/file.txt");

    fs.seed_file(&source_file, b"payload".to_vec())?;
    fs.seed_file(
        &path("/virtual/source/nested/inner.txt"),
        b"nested".to_vec(),
    )?;

    fs.copy(
        &source_file,
        &path("/virtual/copied.txt"),
        CopyOptions { recursive: false },
        /*sandbox*/ None,
    )
    .await?;
    assert_eq!(
        fs.read_file(&path("/virtual/copied.txt"), /*sandbox*/ None)
            .await?,
        b"payload"
    );

    let err = fs
        .copy(
            &source_dir,
            &path("/virtual/dir-copy"),
            CopyOptions { recursive: false },
            /*sandbox*/ None,
        )
        .await
        .expect_err("directory copy requires recursive");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    fs.copy(
        &source_dir,
        &path("/virtual/dir-copy"),
        CopyOptions { recursive: true },
        /*sandbox*/ None,
    )
    .await?;
    assert_eq!(
        fs.read_file(
            &path("/virtual/dir-copy/nested/inner.txt"),
            /*sandbox*/ None
        )
        .await?,
        b"nested"
    );

    let err = fs
        .copy(
            &source_dir,
            &path("/virtual/source/descendant"),
            CopyOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await
        .expect_err("copying a directory into a descendant should fail");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    Ok(())
}

#[tokio::test]
async fn copy_handles_empty_directories_and_missing_sources() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let source_dir = path("/virtual/source");
    let empty_dir = path("/virtual/source/empty");

    fs.seed_directory(&empty_dir)?;
    fs.copy(
        &source_dir,
        &path("/virtual/destination"),
        CopyOptions { recursive: true },
        /*sandbox*/ None,
    )
    .await?;
    assert!(fs.exists(&path("/virtual/destination/empty")));

    let err = fs
        .copy(
            &path("/virtual/missing"),
            &path("/virtual/destination/missing"),
            CopyOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await
        .expect_err("copy should reject missing source");
    assert_eq!(err.kind(), io::ErrorKind::NotFound);

    Ok(())
}

#[tokio::test]
async fn copy_rejects_file_over_directory() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let source_file = path("/virtual/source/file.txt");
    let destination_dir = path("/virtual/destination");

    fs.seed_file(&source_file, b"payload".to_vec())?;
    fs.seed_directory(&destination_dir)?;

    let err = fs
        .copy(
            &source_file,
            &destination_dir,
            CopyOptions { recursive: false },
            /*sandbox*/ None,
        )
        .await
        .expect_err("copy should reject file over directory");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    Ok(())
}

#[tokio::test]
async fn inspection_helpers_report_missing_paths_and_directories() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let dir = path("/virtual/project");
    let missing = path("/virtual/missing");

    fs.seed_directory(&dir)?;

    assert!(fs.exists(&dir));
    assert!(!fs.exists(&missing));
    assert_eq!(fs.file_contents(&dir), None);
    assert_eq!(fs.file_contents(&missing), None);

    Ok(())
}

#[tokio::test]
async fn sandbox_context_is_rejected_for_all_operations() -> io::Result<()> {
    let fs = InMemoryFileSystem::new();
    let sandbox = sandbox_context();
    let dir = path("/virtual/project");
    let file = path("/virtual/project/file.txt");

    fs.seed_file(&file, b"contents".to_vec())?;

    let err = fs
        .read_file(&file, Some(&sandbox))
        .await
        .expect_err("read_file should reject sandbox");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    let err = fs
        .write_file(&file, b"contents".to_vec(), Some(&sandbox))
        .await
        .expect_err("write_file should reject sandbox");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    let err = fs
        .create_directory(
            &dir,
            CreateDirectoryOptions { recursive: true },
            Some(&sandbox),
        )
        .await
        .expect_err("create_directory should reject sandbox");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    let err = fs
        .get_metadata(&dir, Some(&sandbox))
        .await
        .expect_err("get_metadata should reject sandbox");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    let err = fs
        .read_directory(&dir, Some(&sandbox))
        .await
        .expect_err("read_directory should reject sandbox");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    let err = fs
        .remove(
            &file,
            RemoveOptions {
                recursive: false,
                force: false,
            },
            Some(&sandbox),
        )
        .await
        .expect_err("remove should reject sandbox");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    let err = fs
        .copy(
            &file,
            &path("/virtual/project/copy.txt"),
            CopyOptions { recursive: false },
            Some(&sandbox),
        )
        .await
        .expect_err("copy should reject sandbox");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    Ok(())
}
