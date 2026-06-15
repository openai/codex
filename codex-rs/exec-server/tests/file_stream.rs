mod common;

use anyhow::Result;
use codex_exec_server::ExecServerClient;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::FsReadFileParams;
use codex_exec_server::RemoteExecServerConnectArgs;
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
use tempfile::TempDir;

use crate::common::exec_server::exec_server;

const BLOCK_SIZE: usize = 1024 * 1024;

#[tokio::test]
async fn stream_reads_file_in_one_mib_blocks() -> Result<()> {
    let server = exec_server().await?;
    let client = connect_client(server.websocket_url()).await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("blocks.bin");
    let contents = (0..BLOCK_SIZE * 2 + 17)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    std::fs::write(&path, &contents)?;

    let chunks = client
        .stream(FsReadFileParams {
            path: PathUri::from_path(path)?,
            sandbox: None,
        })
        .await?
        .try_collect::<Vec<_>>()
        .await?;

    assert_eq!(
        chunks.iter().map(bytes::Bytes::len).collect::<Vec<_>>(),
        vec![BLOCK_SIZE, BLOCK_SIZE, 17]
    );
    assert_eq!(
        chunks
            .iter()
            .flat_map(|chunk| chunk.iter().copied())
            .collect::<Vec<_>>(),
        contents
    );
    Ok(())
}

#[tokio::test]
async fn stream_stops_after_an_exact_block_boundary() -> Result<()> {
    let server = exec_server().await?;
    let client = connect_client(server.websocket_url()).await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("exact-blocks.bin");
    std::fs::write(&path, vec![b'x'; BLOCK_SIZE * 2])?;

    let chunks = client
        .stream(FsReadFileParams {
            path: PathUri::from_path(path)?,
            sandbox: None,
        })
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
async fn stream_rejects_platform_sandbox() -> Result<()> {
    let server = exec_server().await?;
    let client = connect_client(server.websocket_url()).await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("sandboxed.txt");
    std::fs::write(&path, "sandboxed hello")?;

    let result = client
        .stream(FsReadFileParams {
            path: PathUri::from_path(&path)?,
            sandbox: Some(read_only_sandbox(tmp.path().to_path_buf())),
        })
        .await;

    let Err(error) = result else {
        panic!("sandboxed stream should be rejected");
    };
    assert_eq!(
        error.to_string(),
        "exec-server rejected request (-32600): streaming file reads do not support platform sandboxing"
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn stream_keeps_reading_the_open_file_after_path_replacement() -> Result<()> {
    let server = exec_server().await?;
    let client = connect_client(server.websocket_url()).await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("replaceable.bin");
    std::fs::write(&path, vec![b'a'; BLOCK_SIZE + 1])?;
    let mut stream = client
        .stream(FsReadFileParams {
            path: PathUri::from_path(&path)?,
            sandbox: None,
        })
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

async fn connect_client(websocket_url: &str) -> Result<ExecServerClient> {
    Ok(
        ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
            websocket_url.to_string(),
            "file-stream-test".to_string(),
        ))
        .await?,
    )
}

fn read_only_sandbox(path: std::path::PathBuf) -> FileSystemSandboxContext {
    let path = AbsolutePathBuf::from_absolute_path(&path)
        .unwrap_or_else(|err| panic!("sandbox path should be absolute: {err}"));
    FileSystemSandboxContext::from_permission_profile(PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path { path },
            access: FileSystemAccessMode::Read,
        }]),
        NetworkSandboxPolicy::Restricted,
    ))
}
