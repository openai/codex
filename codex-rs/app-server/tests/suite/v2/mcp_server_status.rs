use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn write_user_config(codex_home: &TempDir, contents: &str) -> Result<()> {
    Ok(std::fs::write(
        codex_home.path().join("config.toml"),
        contents,
    )?)
}

fn write_project_config(project_root: &TempDir, contents: &str) -> Result<()> {
    let dot_codex = project_root.path().join(".codex");
    std::fs::create_dir_all(&dot_codex)?;
    Ok(std::fs::write(dot_codex.join("config.toml"), contents)?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_server_status_list_honors_cwd_config_layer() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_user_config(
        &codex_home,
        r#"
[mcp_servers.user_only]
command = "true"
enabled = false
"#,
    )?;

    let project_root = TempDir::new()?;
    write_project_config(
        &project_root,
        r#"
[mcp_servers.project_only]
command = "true"
enabled = false
"#,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_list_mcp_server_status_request(ListMcpServerStatusParams {
            cursor: None,
            limit: None,
            cwd: Some(project_root.path().display().to_string()),
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let ListMcpServerStatusResponse { data, next_cursor } = to_response(resp)?;
    assert_eq!(next_cursor, None);

    let names: Vec<&str> = data.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"project_only"));
    assert!(!names.contains(&"user_only"));

    Ok(())
}
