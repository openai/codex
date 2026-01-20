use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::CustomPromptsListParams;
use codex_app_server_protocol::CustomPromptsListResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn write_prompt(dir: &std::path::Path, name: &str, content: &str) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(format!("{name}.md")), content)?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_prompts_list_defaults_to_global_only() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_prompt(&codex_home.path().join("prompts"), "global", "global")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_custom_prompts_list_request(CustomPromptsListParams { cwd: None })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: CustomPromptsListResponse = to_response(resp)?;

    assert_eq!(response.custom_prompts.len(), 1);
    assert_eq!(response.custom_prompts[0].name, "global");
    assert_eq!(response.custom_prompts[0].content, "global");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_prompts_list_respects_layered_project_prompts() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_prompt(&codex_home.path().join("prompts"), "global", "global")?;

    let project_root = TempDir::new()?;
    std::fs::write(project_root.path().join(".git"), "")?;
    write_prompt(
        &project_root.path().join(".codex/prompts"),
        "shared",
        "root shared",
    )?;

    let child = project_root.path().join("child");
    write_prompt(&child.join(".codex/prompts"), "shared", "child shared")?;
    write_prompt(&child.join(".codex/prompts"), "child-only", "child only")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_custom_prompts_list_request(CustomPromptsListParams {
            cwd: Some(child.to_string_lossy().to_string()),
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: CustomPromptsListResponse = to_response(resp)?;

    let mut by_name = response
        .custom_prompts
        .into_iter()
        .map(|prompt| {
            let name = prompt.name.clone();
            (name, prompt)
        })
        .collect::<std::collections::HashMap<_, _>>();

    assert_eq!(
        by_name.remove("shared").expect("shared").content,
        "child shared"
    );
    assert_eq!(
        by_name.remove("child-only").expect("child-only").content,
        "child only"
    );
    assert_eq!(by_name.remove("global").expect("global").content, "global");
    assert_eq!(by_name.len(), 0);

    Ok(())
}
