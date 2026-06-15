mod common;

use anyhow::Result;
use base64::Engine as _;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_exec_server::ExecServerClient;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::FsReadFileParams;
use codex_exec_server::InitializeParams;
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
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tempfile::TempDir;
use uuid::Uuid;

use crate::common::exec_server::ExecServerHarness;
use crate::common::exec_server::exec_server;

const BLOCK_SIZE: usize = 1024 * 1024;
const OPEN_FILE_LIMIT: usize = 128;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenFileResponse {
    handle_id: String,
}

#[derive(Debug, Deserialize)]
struct ReadBlockResponse {
    chunk: String,
    eof: bool,
}

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

#[tokio::test]
async fn read_block_supports_non_sequential_offsets_and_lengths() -> Result<()> {
    let mut server = exec_server().await?;
    initialize_exec_server(&mut server).await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("non-sequential.bin");
    std::fs::write(&path, b"0123456789")?;
    let open: OpenFileResponse = rpc_call(
        &mut server,
        "fs/open",
        serde_json::json!({
            "handleId": Uuid::new_v4().to_string(),
            "path": PathUri::from_path(path)?,
            "sandbox": null,
        }),
    )
    .await?;

    assert_eq!(
        read_block(
            &mut server,
            &open.handle_id,
            /*offset*/ 6,
            /*len*/ 3
        )
        .await?,
        (b"678".to_vec(), false)
    );
    assert_eq!(
        read_block(
            &mut server,
            &open.handle_id,
            /*offset*/ 1,
            /*len*/ 2
        )
        .await?,
        (b"12".to_vec(), false)
    );
    assert_eq!(
        read_block(
            &mut server,
            &open.handle_id,
            /*offset*/ 8,
            /*len*/ 4
        )
        .await?,
        (b"89".to_vec(), true)
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn open_enforces_the_per_connection_limit_and_close_releases_capacity() -> Result<()> {
    let mut server = exec_server().await?;
    initialize_exec_server(&mut server).await?;
    let tmp = TempDir::new()?;
    let path = tmp.path().join("limited.bin");
    std::fs::write(&path, b"limited")?;
    let path = PathUri::from_path(path)?;
    let mut handles = Vec::with_capacity(OPEN_FILE_LIMIT);
    for _ in 0..OPEN_FILE_LIMIT {
        let open: OpenFileResponse = rpc_call(
            &mut server,
            "fs/open",
            serde_json::json!({
                "handleId": Uuid::new_v4().to_string(),
                "path": path,
                "sandbox": null,
            }),
        )
        .await?;
        handles.push(open.handle_id);
    }

    let response = rpc_message(
        &mut server,
        "fs/open",
        serde_json::json!({
            "handleId": Uuid::new_v4().to_string(),
            "path": path,
            "sandbox": null,
        }),
    )
    .await?;
    let JSONRPCMessage::Error(JSONRPCError { error, .. }) = response else {
        anyhow::bail!("expected opening beyond the limit to fail, got {response:?}");
    };
    assert_eq!(
        (error.code, error.message),
        (
            -32600,
            format!("at most {OPEN_FILE_LIMIT} file reads may be open per connection"),
        )
    );

    let _: serde_json::Value = rpc_call(
        &mut server,
        "fs/close",
        serde_json::json!({ "handleId": handles[0] }),
    )
    .await?;
    let _: OpenFileResponse = rpc_call(
        &mut server,
        "fs/open",
        serde_json::json!({
            "handleId": Uuid::new_v4().to_string(),
            "path": path,
            "sandbox": null,
        }),
    )
    .await?;
    server.shutdown().await?;
    Ok(())
}

async fn initialize_exec_server(server: &mut ExecServerHarness) -> Result<()> {
    let _: serde_json::Value = rpc_call(
        server,
        "initialize",
        serde_json::to_value(InitializeParams {
            client_name: "file-stream-protocol-test".to_string(),
            resume_session_id: None,
        })?,
    )
    .await?;
    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;
    Ok(())
}

async fn read_block(
    server: &mut ExecServerHarness,
    handle_id: &str,
    offset: u64,
    len: usize,
) -> Result<(Vec<u8>, bool)> {
    let response: ReadBlockResponse = rpc_call(
        server,
        "fs/readBlock",
        serde_json::json!({ "handleId": handle_id, "offset": offset, "len": len }),
    )
    .await?;
    Ok((
        base64::engine::general_purpose::STANDARD.decode(response.chunk)?,
        response.eof,
    ))
}

async fn rpc_call<T>(
    server: &mut ExecServerHarness,
    method: &str,
    params: serde_json::Value,
) -> Result<T>
where
    T: DeserializeOwned,
{
    let response = rpc_message(server, method, params).await?;
    let JSONRPCMessage::Response(JSONRPCResponse { result, .. }) = response else {
        anyhow::bail!("expected successful `{method}` response, got {response:?}");
    };
    Ok(serde_json::from_value(result)?)
}

async fn rpc_message(
    server: &mut ExecServerHarness,
    method: &str,
    params: serde_json::Value,
) -> Result<JSONRPCMessage> {
    let request_id = server.send_request(method, params).await?;
    server
        .wait_for_event(|event| match event {
            JSONRPCMessage::Response(JSONRPCResponse { id, .. })
            | JSONRPCMessage::Error(JSONRPCError { id, .. }) => id == &request_id,
            JSONRPCMessage::Request(_) | JSONRPCMessage::Notification(_) => false,
        })
        .await
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
