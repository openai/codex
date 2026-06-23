#![cfg(unix)]

mod common;

use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::FsReadFileParams;
use codex_exec_server::InitializeParams;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use common::exec_server::ExecServerHarness;
use common::exec_server::exec_server;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sandbox_helper_enforces_fs_read_file_max_bytes() -> anyhow::Result<()> {
    let source_dir = tempfile::tempdir()?;
    let source = source_dir.path().join("bounded.txt");
    tokio::fs::write(&source, b"four").await?;
    let mut server = exec_server().await?;
    initialize_exec_server(&mut server).await?;

    let request_id = server
        .send_request(
            "fs/readFile",
            serde_json::to_value(FsReadFileParams {
                path: PathUri::from_path(&source)?,
                sandbox: Some(read_only_sandbox(source_dir.path().to_path_buf())?),
                max_bytes: Some(3),
            })?,
        )
        .await?;
    let error = server.next_event().await?;
    let JSONRPCMessage::Error(error) = error else {
        anyhow::bail!("expected JSON-RPC error response, got {error:?}")
    };
    assert_eq!(error.id, request_id);
    assert_eq!(
        (error.error.code, error.error.message),
        (
            -32600,
            "file is too large to read: limit is 3 bytes".to_string()
        )
    );

    server.shutdown().await?;
    Ok(())
}

async fn initialize_exec_server(server: &mut ExecServerHarness) -> anyhow::Result<()> {
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "bounded-read-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;
    let response = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &initialize_id
            )
        })
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, .. }) = response else {
        anyhow::bail!("expected initialize response")
    };
    assert_eq!(id, initialize_id);
    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;
    Ok(())
}

fn read_only_sandbox(path: std::path::PathBuf) -> anyhow::Result<FileSystemSandboxContext> {
    let path = AbsolutePathBuf::from_absolute_path(&path)?;
    Ok(FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
                path: FileSystemPath::Path { path },
                access: FileSystemAccessMode::Read,
            }]),
            NetworkSandboxPolicy::Restricted,
        ),
    ))
}
