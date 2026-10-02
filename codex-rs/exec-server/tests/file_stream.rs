mod common;

#[cfg(unix)]
use anyhow::Context;
use anyhow::Result;
use codex_exec_server::Environment;
use codex_exec_server::ExecServerClient;
use codex_exec_server::ExecServerError;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FsCloseParams;
use codex_exec_server::FsOpenMode;
use codex_exec_server::FsOpenParams;
use codex_exec_server::FsReadBlockParams;
use codex_exec_server::FsReadBlockResponse;
use codex_exec_server::FsWriteBlockParams;
use codex_exec_server::ReadFileOptions;
use codex_exec_server::RemoteExecServerConnectArgs;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_utils_path_uri::PathUri;
use futures::TryStreamExt;
use pretty_assertions::assert_eq;
use std::sync::Arc;
#[cfg(any(unix, windows))]
use std::time::Duration;
use tempfile::TempDir;
#[cfg(windows)]
use tokio::net::windows::named_pipe::ServerOptions;
#[cfg(any(unix, windows))]
use tokio::time::timeout;
use uuid::Uuid;

use crate::common::exec_server::exec_server;

const BLOCK_SIZE: usize = 1024 * 1024;
const OPEN_FILE_LIMIT: usize = 128;

#[tokio::test]
async fn stream_stops_after_an_exact_block_boundary() -> Result<()> {
    let server = exec_server().await?;
    let file_system = connect_file_system(server.websocket_url())?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("exact-blocks.bin");
    std::fs::write(&path, vec![b'x'; BLOCK_SIZE * 2])?;

    let chunks = file_system
        .read_file_stream(
            &PathUri::from_host_native_path(path)?,
            /*sandbox*/ None,
        )
        .await?
        .try_collect::<Vec<_>>()
        .await?;

    assert_eq!(
        chunks.iter().map(bytes::Bytes::len).collect::<Vec<_>>(),
        vec![BLOCK_SIZE, BLOCK_SIZE]
    );
    Ok(())
}

#[tokio::test]
async fn completed_streams_release_handle_capacity() -> Result<()> {
    let server = exec_server().await?;
    let file_system = connect_file_system(server.websocket_url())?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("repeated.txt");
    std::fs::write(&path, b"repeated")?;
    let path = PathUri::from_host_native_path(path)?;

    for _ in 0..=OPEN_FILE_LIMIT {
        let chunks = file_system
            .read_file_stream(&path, /*sandbox*/ None)
            .await?
            .try_collect::<Vec<_>>()
            .await?;
        assert_eq!(chunks, vec![bytes::Bytes::from_static(b"repeated")]);
    }

    Ok(())
}

#[cfg(unix)]
#[test_case::test_case(true ; "follow")]
#[test_case::test_case(false ; "no_follow")]
#[tokio::test]
async fn file_reads_reject_fifo_without_waiting_for_a_writer(follow_symlinks: bool) -> Result<()> {
    let server = exec_server().await?;
    let file_system = connect_file_system(server.websocket_url())?;
    let tmp = TempDir::new()?;
    let path = tmp.path().canonicalize()?.join("named-pipe");
    let output = std::process::Command::new("mkfifo").arg(&path).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "mkfifo failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let path_uri = PathUri::from_host_native_path(&path)?;
    let read_result = timeout(
        Duration::from_secs(1),
        file_system.read_file(
            &path_uri,
            ReadFileOptions { follow_symlinks },
            /*sandbox*/ None,
        ),
    )
    .await
    .context("reading a FIFO should not wait for a writer")?;
    let Err(read_error) = read_result else {
        panic!("reading a FIFO should be rejected");
    };
    let stream_result = timeout(
        Duration::from_secs(1),
        file_system.read_file_stream(&path_uri, /*sandbox*/ None),
    )
    .await
    .context("streaming a FIFO should not wait for a writer")?;
    let Err(stream_error) = stream_result else {
        panic!("streaming a FIFO should be rejected");
    };
    let expected = format!("path `{}` is not a file", path.display());
    let expected_read = if follow_symlinks {
        expected.clone()
    } else {
        "path is not a regular file".to_string()
    };
    assert_eq!(
        (read_error.to_string(), stream_error.to_string()),
        (expected_read, expected)
    );
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn file_reads_reject_named_pipes() -> Result<()> {
    let server = exec_server().await?;
    let file_system = connect_file_system(server.websocket_url())?;

    let read_path = format!(r"\\.\pipe\codex-fs-read-{}", Uuid::new_v4());
    let _read_pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&read_path)?;
    let read_error = timeout(
        Duration::from_secs(1),
        file_system.read_file(
            &PathUri::from_host_native_path(std::path::Path::new(&read_path))?,
            ReadFileOptions::default(),
            /*sandbox*/ None,
        ),
    )
    .await
    .expect("reading a named pipe should not hang")
    .expect_err("reading a named pipe should be rejected");

    let stream_path = format!(r"\\.\pipe\codex-fs-stream-{}", Uuid::new_v4());
    let _stream_pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&stream_path)?;
    let stream_result = timeout(
        Duration::from_secs(1),
        file_system.read_file_stream(
            &PathUri::from_host_native_path(std::path::Path::new(&stream_path))?,
            /*sandbox*/ None,
        ),
    )
    .await
    .expect("streaming a named pipe should not hang");
    let Err(stream_error) = stream_result else {
        panic!("streaming a named pipe should be rejected");
    };

    assert_eq!(
        (read_error.kind(), stream_error.kind()),
        (
            std::io::ErrorKind::InvalidInput,
            std::io::ErrorKind::InvalidInput,
        )
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn stream_keeps_reading_the_open_file_after_path_replacement() -> Result<()> {
    let server = exec_server().await?;
    let file_system = connect_file_system(server.websocket_url())?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("replaceable.bin");
    std::fs::write(&path, vec![b'a'; BLOCK_SIZE + 1])?;
    let sandbox = read_only_sandbox(tmp.path().to_path_buf());
    let mut stream = file_system
        .read_file_stream(&PathUri::from_host_native_path(&path)?, Some(&sandbox))
        .await?;

    assert_eq!(
        stream.try_next().await?,
        Some(bytes::Bytes::from(vec![b'a'; BLOCK_SIZE]))
    );
    let replacement = tmp.path().join("replacement.bin");
    std::fs::write(&replacement, vec![b'b'; BLOCK_SIZE + 1])?;
    std::fs::remove_file(&path)?;
    std::fs::rename(replacement, &path)?;

    assert_eq!(
        stream.try_next().await?,
        Some(bytes::Bytes::from_static(b"a"))
    );
    assert_eq!(stream.try_next().await?, None);
    Ok(())
}

#[tokio::test]
async fn read_block_supports_non_sequential_offsets_and_lengths() -> Result<()> {
    let mut server = exec_server().await?;
    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
        server.websocket_url().to_string(),
        "file-stream-protocol-test".to_string(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    ))
    .await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("non-sequential.bin");
    std::fs::write(&path, b"0123456789")?;
    let open = client
        .fs_open(FsOpenParams {
            handle_id: Uuid::new_v4().simple().to_string(),
            path: PathUri::from_host_native_path(path)?,
            mode: FsOpenMode::Read,
            sandbox: None,
        })
        .await?;

    client
        .fs_write_block(FsWriteBlockParams {
            handle_id: open.handle_id.clone(),
            offset: 0,
            chunk: b"x".to_vec().into(),
        })
        .await
        .expect_err("default read-only handles must reject writes");

    let mut blocks = Vec::new();
    for (offset, len) in [(6, 3), (1, 2), (8, 4), (0, 2)] {
        blocks.push(
            client
                .fs_read_block(FsReadBlockParams {
                    handle_id: open.handle_id.clone(),
                    offset,
                    len,
                })
                .await?,
        );
    }
    assert_eq!(
        blocks,
        vec![
            FsReadBlockResponse {
                chunk: b"678".to_vec().into(),
                eof: false,
            },
            FsReadBlockResponse {
                chunk: b"12".to_vec().into(),
                eof: false,
            },
            FsReadBlockResponse {
                chunk: b"89".to_vec().into(),
                eof: true,
            },
            FsReadBlockResponse {
                chunk: b"01".to_vec().into(),
                eof: false,
            },
        ]
    );
    client
        .fs_close(FsCloseParams {
            handle_id: open.handle_id,
        })
        .await?;
    drop(client);
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn open_enforces_the_per_connection_limit_and_close_releases_capacity() -> Result<()> {
    let mut server = exec_server().await?;
    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
        server.websocket_url().to_string(),
        "file-stream-protocol-test".to_string(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    ))
    .await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("limited.bin");
    std::fs::write(&path, b"limited")?;
    let path = PathUri::from_host_native_path(path)?;
    let mut handles = Vec::with_capacity(OPEN_FILE_LIMIT);
    for _ in 0..OPEN_FILE_LIMIT {
        let open = client
            .fs_open(FsOpenParams {
                handle_id: Uuid::new_v4().simple().to_string(),
                path: path.clone(),
                mode: FsOpenMode::Read,
                sandbox: None,
            })
            .await?;
        handles.push(open.handle_id);
    }

    let error = client
        .fs_open(FsOpenParams {
            handle_id: Uuid::new_v4().simple().to_string(),
            path: path.clone(),
            mode: FsOpenMode::Read,
            sandbox: None,
        })
        .await
        .expect_err("opening beyond the limit should fail");
    let ExecServerError::Server { code, message } = error else {
        anyhow::bail!("expected server error, got {error:?}");
    };
    assert_eq!(
        (code, message),
        (
            -32600,
            format!("at most {OPEN_FILE_LIMIT} file handles may be open per connection"),
        )
    );

    client
        .fs_close(FsCloseParams {
            handle_id: handles.remove(0),
        })
        .await?;
    client
        .fs_open(FsOpenParams {
            handle_id: Uuid::new_v4().simple().to_string(),
            path,
            mode: FsOpenMode::Read,
            sandbox: None,
        })
        .await?;
    drop(client);
    server.shutdown().await?;
    Ok(())
}

/// Rejected replacement opens must neither truncate existing files nor create missing files.
#[test_case::test_case(1, "0", "file handle `0` already exists"; "duplicate_id")]
#[test_case::test_case(OPEN_FILE_LIMIT, "overflow", "at most 128 file handles may be open per connection"; "capacity")]
#[tokio::test]
async fn replace_open_rejects_unavailable_handles_before_touching_files(
    handle_count: usize,
    handle_id: &str,
    expected_message: &str,
) -> Result<()> {
    let server = exec_server().await?;
    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
        server.websocket_url().to_string(),
        "file-admission-test".to_string(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    ))
    .await?;
    let tmp = TempDir::new()?;
    let existing = tmp.path().join("existing.bin");
    let missing = tmp.path().join("missing.bin");
    std::fs::write(&existing, b"original")?;
    for index in 0..handle_count {
        client
            .fs_open(FsOpenParams {
                handle_id: index.to_string(),
                path: PathUri::from_host_native_path(&existing)?,
                mode: FsOpenMode::Read,
                sandbox: None,
            })
            .await?;
    }

    for path in [&existing, &missing] {
        let error = client
            .fs_open(FsOpenParams {
                handle_id: handle_id.to_string(),
                path: PathUri::from_host_native_path(path)?,
                mode: FsOpenMode::Replace,
                sandbox: None,
            })
            .await;
        let Err(ExecServerError::Server { code, message }) = error else {
            anyhow::bail!("expected server error, got {error:?}");
        };
        assert_eq!((code, message), (-32600, expected_message.to_string()));
    }
    assert_eq!(std::fs::read(&existing)?, b"original");
    assert!(!missing.exists());
    Ok(())
}

#[tokio::test]
async fn open_rejects_handle_ids_longer_than_32_bytes() -> Result<()> {
    let server = exec_server().await?;
    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
        server.websocket_url().to_string(),
        "file-stream-protocol-test".to_string(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    ))
    .await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("handle-id-limit.bin");
    std::fs::write(&path, b"limited")?;

    let error = client
        .fs_open(FsOpenParams {
            handle_id: "x".repeat(33),
            path: PathUri::from_host_native_path(path)?,
            mode: FsOpenMode::Read,
            sandbox: None,
        })
        .await
        .expect_err("oversized handle ID should fail");

    let ExecServerError::Server { code, message } = error else {
        anyhow::bail!("expected server error, got {error:?}");
    };
    assert_eq!(
        (code, message),
        (
            -32600,
            "file handle ID must not exceed 32 bytes".to_string(),
        )
    );
    Ok(())
}

/// Writes use explicit offsets and preserve untouched bytes across multiple blocks.
#[tokio::test]
async fn write_blocks_support_non_sequential_offsets_and_enforce_bounds() -> Result<()> {
    let server = exec_server().await?;
    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
        server.websocket_url().to_string(),
        "file-write-test".to_string(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    ))
    .await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("blocks.bin");
    let open = client
        .fs_open(FsOpenParams {
            handle_id: "writer".to_string(),
            path: PathUri::from_host_native_path(&path)?,
            mode: FsOpenMode::Replace,
            sandbox: None,
        })
        .await?;
    for (offset, chunk) in [
        (BLOCK_SIZE as u64, vec![b'z'; 3]),
        (0, vec![b'a'; BLOCK_SIZE]),
        (1, b"bc".to_vec()),
    ] {
        client
            .fs_write_block(FsWriteBlockParams {
                handle_id: open.handle_id.clone(),
                offset,
                chunk: chunk.into(),
            })
            .await?;
    }
    for (offset, chunk) in [
        (0, Vec::new()),
        (0, vec![b'x'; BLOCK_SIZE + 1]),
        (u64::MAX, vec![b'x']),
    ] {
        client
            .fs_write_block(FsWriteBlockParams {
                handle_id: open.handle_id.clone(),
                offset,
                chunk: chunk.into(),
            })
            .await
            .expect_err("empty, oversized, and overflowing writes must be rejected");
    }
    let read = client
        .fs_read_block(FsReadBlockParams {
            handle_id: open.handle_id.clone(),
            offset: 0,
            len: 4,
        })
        .await;
    read.expect_err("write-only handles must reject reads");
    let mut expected = vec![b'a'; BLOCK_SIZE];
    expected[1..3].copy_from_slice(b"bc");
    expected.extend_from_slice(b"zzz");
    assert_eq!(std::fs::read(&path)?, expected);
    client
        .fs_write_block(FsWriteBlockParams {
            handle_id: "writer".to_string(),
            offset: 0,
            chunk: b"x".to_vec().into(),
        })
        .await
        .expect_err("read failures must close the handle");
    Ok(())
}

/// Invalid signed offsets must not reach Windows' current-position sentinel or mutate the file.
#[test_case::test_case(u64::MAX - 1, 1; "windows_current_position_sentinel")]
#[test_case::test_case(u64::MAX, 1; "unsigned_overflow")]
#[test_case::test_case(i64::MAX as u64, 1; "signed_end_overflow")]
#[test_case::test_case(i64::MAX as u64 - 1, 2; "block_crosses_signed_limit")]
#[tokio::test]
async fn write_blocks_reject_ranges_outside_signed_file_offsets(
    offset: u64,
    len: usize,
) -> Result<()> {
    let server = exec_server().await?;
    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
        server.websocket_url().to_string(),
        "file-write-offset-test".to_string(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    ))
    .await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("offsets.bin");
    let open = client
        .fs_open(FsOpenParams {
            handle_id: "writer".to_string(),
            path: PathUri::from_host_native_path(&path)?,
            mode: FsOpenMode::Replace,
            sandbox: None,
        })
        .await?;
    client
        .fs_write_block(FsWriteBlockParams {
            handle_id: open.handle_id.clone(),
            offset: 0,
            chunk: b"original".to_vec().into(),
        })
        .await?;

    let error = client
        .fs_write_block(FsWriteBlockParams {
            handle_id: open.handle_id.clone(),
            offset,
            chunk: vec![b'x'; len].into(),
        })
        .await;
    let Err(ExecServerError::Server { code, message }) = error else {
        anyhow::bail!("expected server error, got {error:?}");
    };
    assert_eq!(
        (code, message),
        (
            -32600,
            "file write range exceeds the signed 64-bit file offset limit".to_string(),
        )
    );
    client
        .fs_close(FsCloseParams {
            handle_id: open.handle_id,
        })
        .await?;
    assert_eq!(std::fs::read(&path)?, b"original");
    Ok(())
}

/// Replacement discards the previous contents before any streamed blocks are written.
#[tokio::test]
async fn replace_open_truncates_existing_file() -> Result<()> {
    let server = exec_server().await?;
    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
        server.websocket_url().to_string(),
        "file-replace-test".to_string(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    ))
    .await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("replace.bin");
    std::fs::write(&path, b"original")?;
    let open = client
        .fs_open(FsOpenParams {
            handle_id: "replacement".to_string(),
            path: PathUri::from_host_native_path(&path)?,
            mode: FsOpenMode::Replace,
            sandbox: None,
        })
        .await?;
    assert_eq!(std::fs::read(&path)?, b"");
    client
        .fs_close(FsCloseParams {
            handle_id: open.handle_id,
        })
        .await?;
    Ok(())
}

/// Both restricted reads and full-disk reads must retain write sandbox enforcement.
#[cfg(unix)]
#[tokio::test]
async fn writable_open_obeys_sandbox_write_permissions() -> Result<()> {
    use codex_exec_server::FileSystemSandboxContext;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::FileSystemSpecialPath;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;

    let server = exec_server().await?;
    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
        server.websocket_url().to_string(),
        "file-write-sandbox-test".to_string(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    ))
    .await?;
    let workspace = TempDir::new()?;
    let outside = TempDir::new()?;
    let allowed = workspace.path().join("allowed.bin");
    let denied = outside.path().join("denied.bin");
    std::fs::write(&denied, b"original")?;
    for full_disk_read in [false, true] {
        let mut entries = vec![FileSystemSandboxEntry::new(
            AbsolutePathBuf::from_absolute_path(workspace.path())?.into(),
            FileSystemAccessMode::Write,
        )];
        if full_disk_read {
            entries.push(FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Read,
            ));
        }
        let sandbox = FileSystemSandboxContext::from_permission_profile(
            PermissionProfile::from_runtime_permissions(
                &FileSystemSandboxPolicy::restricted(entries),
                NetworkSandboxPolicy::Restricted,
            ),
            PathUri::from_host_native_path(workspace.path())?,
        );
        client
            .fs_open(FsOpenParams {
                handle_id: "denied".to_string(),
                path: PathUri::from_host_native_path(&denied)?,
                mode: FsOpenMode::Replace,
                sandbox: Some(sandbox.clone()),
            })
            .await
            .expect_err("writable opens must not escape the write sandbox");
        let open = client
            .fs_open(FsOpenParams {
                handle_id: "allowed".to_string(),
                path: PathUri::from_host_native_path(&allowed)?,
                mode: FsOpenMode::Replace,
                sandbox: Some(sandbox.clone()),
            })
            .await?;
        client
            .fs_write_block(FsWriteBlockParams {
                handle_id: open.handle_id.clone(),
                offset: 0,
                chunk: b"allowed".to_vec().into(),
            })
            .await?;
        client
            .fs_close(FsCloseParams {
                handle_id: open.handle_id,
            })
            .await?;
        assert_eq!(std::fs::read(&allowed)?, b"allowed");
        assert_eq!(std::fs::read(&denied)?, b"original");
    }
    Ok(())
}

fn connect_file_system(websocket_url: &str) -> Result<Arc<dyn ExecutorFileSystem>> {
    let environment = Environment::create_for_tests(Some(websocket_url.to_string()))?;
    Ok(environment.get_filesystem())
}

// Only the Unix stream tests above need this sandbox builder.
#[cfg(unix)]
fn read_only_sandbox(path: std::path::PathBuf) -> codex_exec_server::FileSystemSandboxContext {
    use codex_exec_server::FileSystemSandboxContext;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;

    let path = AbsolutePathBuf::from_absolute_path(&path)
        .unwrap_or_else(|err| panic!("sandbox path should be absolute: {err}"));
    let cwd = PathUri::from_abs_path(&path);
    let permissions = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: path.into(),
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        }]),
        NetworkSandboxPolicy::Restricted,
    );
    FileSystemSandboxContext::from_permission_profile(permissions, cwd)
}
