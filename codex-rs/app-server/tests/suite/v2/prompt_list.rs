use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PromptListParams;
use codex_app_server_protocol::PromptListResponse;
use codex_app_server_protocol::PromptMetadata;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn prompt_list_reads_prompts_from_codex_home() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let prompts_dir = codex_home.path().join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

    let draft_pr_body = r#"---
description: Prep a branch, commit, and open a draft PR
argument-hint: [FILES=<paths>] [PR_TITLE="<title>"]
---

Create a branch named `dev/<feature_name>` for this work.
If files are specified, stage them first: $FILES.
Commit the staged changes with a clear message.
Open a draft PR on the same branch. Use $PR_TITLE when supplied; otherwise write a concise summary yourself."#;
    std::fs::write(prompts_dir.join("draftpr.md"), draft_pr_body)?;
    std::fs::write(prompts_dir.join("note.txt"), "ignore")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_prompt_list_request(PromptListParams {
            roots: Some(Vec::new()),
            force_reload: Some(false),
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let PromptListResponse { mut data } = to_response::<PromptListResponse>(resp)?;
    assert_eq!(data.len(), 1);

    let entry = data.pop().expect("prompt list entry");
    let expected_root = std::fs::canonicalize(&prompts_dir).unwrap_or_else(|_| prompts_dir.clone());
    assert_eq!(entry.root, expected_root);

    let prompts = entry.prompts;
    let expected_content = "\nCreate a branch named `dev/<feature_name>` for this work.\nIf files are specified, stage them first: $FILES.\nCommit the staged changes with a clear message.\nOpen a draft PR on the same branch. Use $PR_TITLE when supplied; otherwise write a concise summary yourself.".to_string();
    let expected = vec![PromptMetadata {
        id: "draftpr".to_string(),
        path: expected_root.join("draftpr.md"),
        content: expected_content,
        description: Some("Prep a branch, commit, and open a draft PR".to_string()),
        argument_hint: Some("[FILES=<paths>] [PR_TITLE=\"<title>\"]".to_string()),
    }];
    assert_eq!(prompts, expected);

    let req_id = mcp.send_request("prompt/list", Some(json!({}))).await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let PromptListResponse { data: raw_data } = to_response::<PromptListResponse>(resp)?;
    assert_eq!(raw_data.len(), 1);

    Ok(())
}

fn create_config_toml(codex_home: &Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(config_toml, config_contents())
}

fn config_contents() -> &'static str {
    r#"model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
"#
}
