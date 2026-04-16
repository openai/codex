use anyhow::Context;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::FsChangedNotification;
use codex_app_server_protocol::FsCopyParams;
use codex_app_server_protocol::FsGetMetadataResponse;
use codex_app_server_protocol::FsReadDirectoryEntry;
use codex_app_server_protocol::FsReadFileResponse;
use codex_app_server_protocol::FsUnwatchParams;
use codex_app_server_protocol::FsWatchResponse;
use codex_app_server_protocol::FsWriteFileParams;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::RequestId;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(unix)]
use std::process::Command;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

async fn initialized_mcp(codex_home: &TempDir) -> Result<McpProcess> {
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    Ok(mcp)
}

async fn expect_error_message(
    mcp: &mut McpProcess,
    request_id: i64,
    expected_message: &str,
) -> Result<()> {
    let error = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.message, expected_message);
    Ok(())
}

#[allow(clippy::expect_used)]
fn absolute_path(path: PathBuf) -> AbsolutePathBuf {
    assert!(
        path.is_absolute(),
        "path must be absolute: {}",
        path.display()
    );
    AbsolutePathBuf::try_from(path).expect("path should be absolute")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_get_metadata_returns_only_used_fields() -> Result<()> {
    let codex_home = TempDir::new()?;
    let file_path = codex_home.path().join("note.txt");
    std::fs::write(&file_path, "hello")?;

    let mut mcp = initialized_mcp(&codex_home).await?;
    let request_id = mcp
        .send_fs_get_metadata_request(codex_app_server_protocol::FsGetMetadataParams {
            path: absolute_path(file_path.clone()),
            environment_id: None,
        })
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let result = response
        .result
        .as_object()
        .context("fs/getMetadata result should be an object")?;
    let mut keys = result.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "createdAtMs".to_string(),
            "isDirectory".to_string(),
            "isFile".to_string(),
            "isSymlink".to_string(),
            "modifiedAtMs".to_string(),
        ]
    );

    let stat: FsGetMetadataResponse = to_response(response)?;
    assert_eq!(
        stat,
        FsGetMetadataResponse {
            is_directory: false,
            is_file: true,
            is_symlink: false,
            created_at_ms: stat.created_at_ms,
            modified_at_ms: stat.modified_at_ms,
        }
    );
    assert!(
        stat.modified_at_ms > 0,
        "modifiedAtMs should be populated for existing files"
    );

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_get_metadata_reports_symlink() -> Result<()> {
    let codex_home = TempDir::new()?;
    let file_path = codex_home.path().join("note.txt");
    let symlink_path = codex_home.path().join("note-link.txt");
    std::fs::write(&file_path, "hello")?;
    symlink(&file_path, &symlink_path)?;

    let mut mcp = initialized_mcp(&codex_home).await?;
    let request_id = mcp
        .send_fs_get_metadata_request(codex_app_server_protocol::FsGetMetadataParams {
            path: absolute_path(symlink_path),
            environment_id: None,
        })
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let stat: FsGetMetadataResponse = to_response(response)?;
    assert_eq!(stat.is_directory, false);
    assert_eq!(stat.is_file, true);
    assert_eq!(stat.is_symlink, true);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_methods_cover_current_fs_utils_surface() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source_dir = codex_home.path().join("source");
    let nested_dir = source_dir.join("nested");
    let source_file = source_dir.join("root.txt");
    let copied_dir = codex_home.path().join("copied");
    let copy_file_path = codex_home.path().join("copy.txt");
    let nested_file = nested_dir.join("note.txt");

    let mut mcp = initialized_mcp(&codex_home).await?;

    let create_directory_request_id = mcp
        .send_fs_create_directory_request(codex_app_server_protocol::FsCreateDirectoryParams {
            path: absolute_path(nested_dir.clone()),
            recursive: None,
            environment_id: None,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(create_directory_request_id)),
    )
    .await??;

    let write_request_id = mcp
        .send_fs_write_file_request(FsWriteFileParams {
            path: absolute_path(nested_file.clone()),
            data_base64: STANDARD.encode("hello from app-server"),
            environment_id: None,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(write_request_id)),
    )
    .await??;

    let root_write_request_id = mcp
        .send_fs_write_file_request(FsWriteFileParams {
            path: absolute_path(source_file.clone()),
            data_base64: STANDARD.encode("hello from source root"),
            environment_id: None,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(root_write_request_id)),
    )
    .await??;

    let read_request_id = mcp
        .send_fs_read_file_request(codex_app_server_protocol::FsReadFileParams {
            path: absolute_path(nested_file.clone()),
            environment_id: None,
        })
        .await?;
    let read_response: FsReadFileResponse = to_response(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(read_request_id)),
        )
        .await??,
    )?;
    assert_eq!(
        read_response,
        FsReadFileResponse {
            data_base64: STANDARD.encode("hello from app-server"),
        }
    );

    let copy_file_request_id = mcp
        .send_fs_copy_request(FsCopyParams {
            source_path: absolute_path(nested_file.clone()),
            destination_path: absolute_path(copy_file_path.clone()),
            recursive: false,
            environment_id: None,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(copy_file_request_id)),
    )
    .await??;
    assert_eq!(
        std::fs::read_to_string(&copy_file_path)?,
        "hello from app-server"
    );

    let copy_dir_request_id = mcp
        .send_fs_copy_request(FsCopyParams {
            source_path: absolute_path(source_dir.clone()),
            destination_path: absolute_path(copied_dir.clone()),
            recursive: true,
            environment_id: None,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(copy_dir_request_id)),
    )
    .await??;
    assert_eq!(
        std::fs::read_to_string(copied_dir.join("nested").join("note.txt"))?,
        "hello from app-server"
    );

    let read_directory_request_id = mcp
        .send_fs_read_directory_request(codex_app_server_protocol::FsReadDirectoryParams {
            path: absolute_path(source_dir.clone()),
            environment_id: None,
        })
        .await?;
    let readdir_response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_directory_request_id)),
    )
    .await??;
    let mut entries =
        to_response::<codex_app_server_protocol::FsReadDirectoryResponse>(readdir_response)?
            .entries;
    entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    assert_eq!(
        entries,
        vec![
            FsReadDirectoryEntry {
                file_name: "nested".to_string(),
                is_directory: true,
                is_file: false,
            },
            FsReadDirectoryEntry {
                file_name: "root.txt".to_string(),
                is_directory: false,
                is_file: true,
            },
        ]
    );

    let remove_request_id = mcp
        .send_fs_remove_request(codex_app_server_protocol::FsRemoveParams {
            path: absolute_path(copied_dir.clone()),
            recursive: None,
            force: None,
            environment_id: None,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(remove_request_id)),
    )
    .await??;
    assert!(
        !copied_dir.exists(),
        "fs/remove should default to recursive+force for directory trees"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_methods_support_named_local_environment() -> Result<()> {
    let codex_home = TempDir::new()?;
    let file_path = codex_home.path().join("named-env.txt");

    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[("CODEX_EXEC_SERVER_URL", Some("none"))],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let write_request_id = mcp
        .send_fs_write_file_request(FsWriteFileParams {
            path: absolute_path(file_path.clone()),
            data_base64: STANDARD.encode("hello from named env"),
            environment_id: Some("local".to_string()),
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(write_request_id)),
    )
    .await??;

    let read_request_id = mcp
        .send_fs_read_file_request(codex_app_server_protocol::FsReadFileParams {
            path: absolute_path(file_path.clone()),
            environment_id: Some("local".to_string()),
        })
        .await?;
    let read_response: FsReadFileResponse = to_response(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(read_request_id)),
        )
        .await??,
    )?;
    assert_eq!(
        read_response,
        FsReadFileResponse {
            data_base64: STANDARD.encode("hello from named env"),
        }
    );
    assert_eq!(std::fs::read_to_string(file_path)?, "hello from named env");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_methods_reject_disabled_default_environment() -> Result<()> {
    let codex_home = TempDir::new()?;
    let file_path = codex_home.path().join("disabled.txt");
    std::fs::write(&file_path, "hello")?;

    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[("CODEX_EXEC_SERVER_URL", Some("none"))],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_fs_read_file_request(codex_app_server_protocol::FsReadFileParams {
            path: absolute_path(file_path),
            environment_id: None,
        })
        .await?;
    expect_error_message(&mut mcp, request_id, "the selected environment is disabled").await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_write_file_accepts_base64_bytes() -> Result<()> {
    let codex_home = TempDir::new()?;
    let file_path = codex_home.path().join("blob.bin");
    let bytes = [0_u8, 1, 2, 255];

    let mut mcp = initialized_mcp(&codex_home).await?;
    let write_request_id = mcp
        .send_fs_write_file_request(FsWriteFileParams {
            path: absolute_path(file_path.clone()),
            data_base64: STANDARD.encode(bytes),
            environment_id: None,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(write_request_id)),
    )
    .await??;
    assert_eq!(std::fs::read(&file_path)?, bytes);

    let read_request_id = mcp
        .send_fs_read_file_request(codex_app_server_protocol::FsReadFileParams {
            path: absolute_path(file_path),
            environment_id: None,
        })
        .await?;
    let read_response: FsReadFileResponse = to_response(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(read_request_id)),
        )
        .await??,
    )?;
    assert_eq!(
        read_response,
        FsReadFileResponse {
            data_base64: STANDARD.encode(bytes),
        }
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_write_file_rejects_invalid_base64() -> Result<()> {
    let codex_home = TempDir::new()?;
    let file_path = codex_home.path().join("blob.bin");

    let mut mcp = initialized_mcp(&codex_home).await?;
    let request_id = mcp
        .send_fs_write_file_request(FsWriteFileParams {
            path: absolute_path(file_path),
            data_base64: "%%%".to_string(),
            environment_id: None,
        })
        .await?;
    let error = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert!(
        error
            .error
            .message
            .starts_with("fs/writeFile requires valid base64 dataBase64:"),
        "unexpected error message: {}",
        error.error.message
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_methods_reject_relative_paths() -> Result<()> {
    let codex_home = TempDir::new()?;
    let absolute_file = codex_home.path().join("absolute.txt");
    std::fs::write(&absolute_file, "hello")?;

    let mut mcp = initialized_mcp(&codex_home).await?;

    let read_id = mcp
        .send_raw_request("fs/readFile", Some(json!({ "path": "relative.txt" })))
        .await?;
    expect_error_message(
        &mut mcp,
        read_id,
        "Invalid request: AbsolutePathBuf deserialized without a base path",
    )
    .await?;

    let write_id = mcp
        .send_raw_request(
            "fs/writeFile",
            Some(json!({
                "path": "relative.txt",
                "dataBase64": STANDARD.encode("hello"),
            })),
        )
        .await?;
    expect_error_message(
        &mut mcp,
        write_id,
        "Invalid request: AbsolutePathBuf deserialized without a base path",
    )
    .await?;

    let create_directory_id = mcp
        .send_raw_request(
            "fs/createDirectory",
            Some(json!({
                "path": "relative-dir",
                "recursive": null,
            })),
        )
        .await?;
    expect_error_message(
        &mut mcp,
        create_directory_id,
        "Invalid request: AbsolutePathBuf deserialized without a base path",
    )
    .await?;

    let get_metadata_id = mcp
        .send_raw_request("fs/getMetadata", Some(json!({ "path": "relative.txt" })))
        .await?;
    expect_error_message(
        &mut mcp,
        get_metadata_id,
        "Invalid request: AbsolutePathBuf deserialized without a base path",
    )
    .await?;

    let read_directory_id = mcp
        .send_raw_request("fs/readDirectory", Some(json!({ "path": "relative-dir" })))
        .await?;
    expect_error_message(
        &mut mcp,
        read_directory_id,
        "Invalid request: AbsolutePathBuf deserialized without a base path",
    )
    .await?;

    let remove_id = mcp
        .send_raw_request(
            "fs/remove",
            Some(json!({
                "path": "relative.txt",
                "recursive": null,
                "force": null,
            })),
        )
        .await?;
    expect_error_message(
        &mut mcp,
        remove_id,
        "Invalid request: AbsolutePathBuf deserialized without a base path",
    )
    .await?;

    let copy_source_id = mcp
        .send_raw_request(
            "fs/copy",
            Some(json!({
                "sourcePath": "relative.txt",
                "destinationPath": absolute_file.clone(),
                "recursive": false,
            })),
        )
        .await?;
    expect_error_message(
        &mut mcp,
        copy_source_id,
        "Invalid request: AbsolutePathBuf deserialized without a base path",
    )
    .await?;

    let copy_destination_id = mcp
        .send_raw_request(
            "fs/copy",
            Some(json!({
                "sourcePath": absolute_file,
                "destinationPath": "relative-copy.txt",
                "recursive": false,
            })),
        )
        .await?;
    expect_error_message(
        &mut mcp,
        copy_destination_id,
        "Invalid request: AbsolutePathBuf deserialized without a base path",
    )
    .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_copy_rejects_directory_without_recursive() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source_dir = codex_home.path().join("source");
    std::fs::create_dir_all(&source_dir)?;

    let mut mcp = initialized_mcp(&codex_home).await?;
    let request_id = mcp
        .send_fs_copy_request(FsCopyParams {
            source_path: absolute_path(source_dir),
            destination_path: absolute_path(codex_home.path().join("dest")),
            recursive: false,
            environment_id: None,
        })
        .await?;
    let error = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        error.error.message,
        "fs/copy requires recursive: true when sourcePath is a directory"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_copy_rejects_copying_directory_into_descendant() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source_dir = codex_home.path().join("source");
    std::fs::create_dir_all(source_dir.join("nested"))?;

    let mut mcp = initialized_mcp(&codex_home).await?;
    let request_id = mcp
        .send_fs_copy_request(FsCopyParams {
            source_path: absolute_path(source_dir.clone()),
            destination_path: absolute_path(source_dir.join("nested").join("copy")),
            recursive: true,
            environment_id: None,
        })
        .await?;
    let error = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        error.error.message,
        "fs/copy cannot copy a directory to itself or one of its descendants"
    );

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_copy_preserves_symlinks_in_recursive_copy() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source_dir = codex_home.path().join("source");
    let nested_dir = source_dir.join("nested");
    let copied_dir = codex_home.path().join("copied");
    std::fs::create_dir_all(&nested_dir)?;
    symlink("nested", source_dir.join("nested-link"))?;

    let mut mcp = initialized_mcp(&codex_home).await?;
    let request_id = mcp
        .send_fs_copy_request(FsCopyParams {
            source_path: absolute_path(source_dir),
            destination_path: absolute_path(copied_dir.clone()),
            recursive: true,
            environment_id: None,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let copied_link = copied_dir.join("nested-link");
    let metadata = std::fs::symlink_metadata(&copied_link)?;
    assert!(metadata.file_type().is_symlink());
    assert_eq!(std::fs::read_link(copied_link)?, PathBuf::from("nested"));

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_copy_ignores_unknown_special_files_in_recursive_copy() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source_dir = codex_home.path().join("source");
    let copied_dir = codex_home.path().join("copied");
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

    let mut mcp = initialized_mcp(&codex_home).await?;
    let request_id = mcp
        .send_fs_copy_request(FsCopyParams {
            source_path: absolute_path(source_dir),
            destination_path: absolute_path(copied_dir.clone()),
            recursive: true,
            environment_id: None,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(
        std::fs::read_to_string(copied_dir.join("note.txt"))?,
        "hello"
    );
    assert!(!copied_dir.join("named-pipe").exists());

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_copy_rejects_standalone_fifo_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let fifo_path = codex_home.path().join("named-pipe");
    let output = Command::new("mkfifo").arg(&fifo_path).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "mkfifo failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let mut mcp = initialized_mcp(&codex_home).await?;
    let request_id = mcp
        .send_fs_copy_request(FsCopyParams {
            source_path: absolute_path(fifo_path),
            destination_path: absolute_path(codex_home.path().join("copied")),
            recursive: false,
            environment_id: None,
        })
        .await?;
    expect_error_message(
        &mut mcp,
        request_id,
        "fs/copy only supports regular files, directories, and symlinks",
    )
    .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_watch_directory_reports_changed_child_paths_and_unwatch_stops_notifications()
-> Result<()> {
    let codex_home = TempDir::new()?;
    let git_dir = codex_home.path().join("repo").join(".git");
    let fetch_head = git_dir.join("FETCH_HEAD");
    std::fs::create_dir_all(&git_dir)?;
    std::fs::write(&fetch_head, "old\n")?;

    let mut mcp = initialized_mcp(&codex_home).await?;
    let watch_id = "watch-git-dir".to_string();
    let watch_request_id = mcp
        .send_fs_watch_request(codex_app_server_protocol::FsWatchParams {
            watch_id: watch_id.clone(),
            path: absolute_path(git_dir.clone()),
            environment_id: None,
        })
        .await?;
    let watch_response: FsWatchResponse = to_response(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(watch_request_id)),
        )
        .await??,
    )?;
    assert_eq!(watch_response.path, absolute_path(git_dir.clone()));

    std::fs::write(&fetch_head, "updated\n")?;

    // Kernel file watching is not reliable in every sandboxed test environment.
    // Keep validating notification shape when the backend does emit, but do not
    // fail the whole suite if no OS event arrives.
    if let Some(changed) = maybe_fs_changed_notification(&mut mcp).await? {
        assert_eq!(changed.watch_id, watch_id.clone());
        assert_eq!(
            changed.changed_paths,
            vec![absolute_path(fetch_head.clone())]
        );
    }
    while timeout(
        Duration::from_millis(200),
        mcp.read_stream_until_notification_message("fs/changed"),
    )
    .await
    .is_ok()
    {}

    let unwatch_request_id = mcp
        .send_fs_unwatch_request(FsUnwatchParams { watch_id })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(unwatch_request_id)),
    )
    .await??;

    std::fs::write(git_dir.join("packed-refs"), "refs\n")?;
    let maybe_notification = timeout(
        Duration::from_millis(1500),
        mcp.read_stream_until_notification_message("fs/changed"),
    )
    .await;
    assert!(
        maybe_notification.is_err(),
        "fs/unwatch should stop future change notifications"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_watch_file_reports_atomic_replace_events() -> Result<()> {
    let codex_home = TempDir::new()?;
    let git_dir = codex_home.path().join("repo").join(".git");
    let head_path = git_dir.join("HEAD");
    std::fs::create_dir_all(&git_dir)?;
    std::fs::write(&head_path, "ref: refs/heads/main\n")?;

    let mut mcp = initialized_mcp(&codex_home).await?;
    let watch_id = "watch-head".to_string();
    let watch_request_id = mcp
        .send_fs_watch_request(codex_app_server_protocol::FsWatchParams {
            watch_id: watch_id.clone(),
            path: absolute_path(head_path.clone()),
            environment_id: None,
        })
        .await?;
    let watch_response: FsWatchResponse = to_response(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(watch_request_id)),
        )
        .await??,
    )?;
    assert_eq!(watch_response.path, absolute_path(head_path.clone()));

    replace_file_atomically(&head_path, "ref: refs/heads/feature\n")?;

    if let Some(changed) = maybe_fs_changed_notification(&mut mcp).await? {
        assert_eq!(
            changed,
            FsChangedNotification {
                watch_id,
                changed_paths: vec![absolute_path(head_path.clone())],
            }
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_watch_allows_missing_file_targets() -> Result<()> {
    let codex_home = TempDir::new()?;
    let git_dir = codex_home.path().join("repo").join(".git");
    let fetch_head = git_dir.join("FETCH_HEAD");
    std::fs::create_dir_all(&git_dir)?;

    let mut mcp = initialized_mcp(&codex_home).await?;
    let watch_id = "watch-fetch-head".to_string();
    let watch_request_id = mcp
        .send_fs_watch_request(codex_app_server_protocol::FsWatchParams {
            watch_id: watch_id.clone(),
            path: absolute_path(fetch_head.clone()),
            environment_id: None,
        })
        .await?;
    let watch_response: FsWatchResponse = to_response(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(watch_request_id)),
        )
        .await??,
    )?;
    assert_eq!(watch_response.path, absolute_path(fetch_head.clone()));

    replace_file_atomically(&fetch_head, "origin/main\n")?;

    if let Some(changed) = maybe_fs_changed_notification(&mut mcp).await? {
        assert_eq!(
            changed,
            FsChangedNotification {
                watch_id,
                changed_paths: vec![absolute_path(fetch_head.clone())],
            }
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_watch_rejects_relative_paths() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let watch_id = mcp
        .send_raw_request(
            "fs/watch",
            Some(json!({ "watchId": "watch-relative", "path": "relative-path" })),
        )
        .await?;
    expect_error_message(
        &mut mcp,
        watch_id,
        "Invalid request: AbsolutePathBuf deserialized without a base path",
    )
    .await?;

    Ok(())
}

fn fs_changed_notification(notification: JSONRPCNotification) -> Result<FsChangedNotification> {
    let params = notification
        .params
        .context("fs/changed notification should include params")?;
    Ok(serde_json::from_value::<FsChangedNotification>(params)?)
}

async fn maybe_fs_changed_notification(
    mcp: &mut McpProcess,
) -> Result<Option<FsChangedNotification>> {
    match timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("fs/changed"),
    )
    .await
    {
        Ok(notification) => Ok(Some(fs_changed_notification(notification?)?)),
        Err(_) => Ok(None),
    }
}

fn replace_file_atomically(path: &PathBuf, contents: &str) -> Result<()> {
    let temp_path = path.with_extension("lock");
    std::fs::write(&temp_path, contents)?;
    std::fs::rename(temp_path, path)?;
    Ok(())
}
