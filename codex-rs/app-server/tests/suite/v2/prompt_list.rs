use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PromptListParams;
use codex_app_server_protocol::PromptListResponse;
use codex_app_server_protocol::PromptMetadata;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
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

    let review_body = r#"---
description: "Quick review command"
argument-hint: "[file]"
---
Review the following changes..."#;
    std::fs::write(prompts_dir.join("review.md"), review_body)?;
    std::fs::write(prompts_dir.join("note.txt"), "ignore")?;
    std::fs::write(prompts_dir.join("quick.md"), "Quick check")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_prompt_list_request(PromptListParams {
            roots: Vec::new(),
            force_reload: false,
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
    assert_eq!(entry.errors, Vec::new());

    let mut prompts = entry.prompts;
    prompts.sort_by(|left, right| left.id.cmp(&right.id));
    let expected = vec![
        PromptMetadata {
            id: "quick".to_string(),
            path: expected_root.join("quick.md"),
            content: "Quick check".to_string(),
            description: None,
            argument_hint: None,
        },
        PromptMetadata {
            id: "review".to_string(),
            path: expected_root.join("review.md"),
            content: "Review the following changes...".to_string(),
            description: Some("Quick review command".to_string()),
            argument_hint: Some("[file]".to_string()),
        },
    ];
    assert_eq!(prompts, expected);

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
