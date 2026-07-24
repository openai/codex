mod common;

use anyhow::Result;
use codex_exec_server::Environment;
use codex_exec_server::ExecServerClient;
use codex_exec_server::ExecServerError;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::FsCloseParams;
use codex_exec_server::FsOpenParams;
use codex_exec_server::FsReadBlockParams;
use codex_exec_server::FsReadBlockResponse;
use codex_exec_server::RemoteExecServerConnectArgs;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
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

#[tokio::test]
async fn stream_rejects_platform_sandbox() -> Result<()> {
    let server = exec_server().await?;
    let file_system = connect_file_system(server.websocket_url())?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("sandboxed.txt");
    std::fs::write(&path, "sandboxed hello")?;

    let result = file_system
        .read_file_stream(
            &PathUri::from_host_native_path(&path)?,
            Some(&read_only_sandbox(tmp.path().to_path_buf())),
        )
        .await;

    let Err(error) = result else {
        panic!("sandboxed stream should be rejected");
    };
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    assert_eq!(
        error.to_string(),
        "streaming file reads do not support platform sandboxing"
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn file_reads_reject_fifo_without_waiting_for_a_writer() -> Result<()> {
    let server = exec_server().await?;
    let file_system = connect_file_system(server.websocket_url())?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("named-pipe");
    let output = std::process::Command::new("mkfifo").arg(&path).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "mkfifo failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let path_uri = PathUri::from_host_native_path(&path)?;
    let read_error = timeout(
        Duration::from_secs(1),
        file_system.read_file(&path_uri, /*sandbox*/ None),
    )
    .await
    .expect("reading a FIFO should not wait for a writer")
    .expect_err("reading a FIFO should be rejected");
    let stream_result = timeout(
        Duration::from_secs(1),
        file_system.read_file_stream(&path_uri, /*sandbox*/ None),
    )
    .await
    .expect("streaming a FIFO should not wait for a writer");
    let Err(stream_error) = stream_result else {
        panic!("streaming a FIFO should be rejected");
    };
    let expected = format!("path `{}` is not a file", path.display());
    assert_eq!(
        (read_error.to_string(), stream_error.to_string()),
        (expected.clone(), expected)
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
    let mut stream = file_system
        .read_file_stream(
            &PathUri::from_host_native_path(&path)?,
            /*sandbox*/ None,
        )
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
            sandbox: None,
        })
        .await?;

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
                sandbox: None,
            })
            .await?;
        handles.push(open.handle_id);
    }

    let error = client
        .fs_open(FsOpenParams {
            handle_id: Uuid::new_v4().simple().to_string(),
            path: path.clone(),
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
            format!("at most {OPEN_FILE_LIMIT} file reads may be open per connection"),
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
            sandbox: None,
        })
        .await?;
    drop(client);
    server.shutdown().await?;
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
            "file read handle ID must not exceed 32 bytes".to_string(),
        )
    );
    Ok(())
}

fn connect_file_system(websocket_url: &str) -> Result<Arc<dyn ExecutorFileSystem>> {
    let environment = Environment::create_for_tests(Some(websocket_url.to_string()))?;
    Ok(environment.get_filesystem())
}

fn read_only_sandbox(path: std::path::PathBuf) -> FileSystemSandboxContext {
    let path = AbsolutePathBuf::from_absolute_path(&path)
        .unwrap_or_else(|err| panic!("sandbox path should be absolute: {err}"));
    FileSystemSandboxContext::from_permission_profile(PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path { path },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        }]),
        NetworkSandboxPolicy::Restricted,
    ))
}
