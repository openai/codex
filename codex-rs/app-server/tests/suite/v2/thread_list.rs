use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_fake_rollout;
use app_test_support::to_response;
use codex_app_server_protocol::GitInfo as ApiGitInfo;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_protocol::protocol::GitInfo as CoreGitInfo;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_list_basic_empty() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // List threads in an empty CODEX_HOME; should return an empty page with nextCursor: null.
    let list_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: Some(10),
            model_providers: Some(vec!["mock_provider".to_string()]),
        })
        .await?;
    let list_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(list_id)),
    )
    .await??;
    let ThreadListResponse { data, next_cursor } = to_response::<ThreadListResponse>(list_resp)?;
    assert!(data.is_empty());
    assert_eq!(next_cursor, None);

    Ok(())
}

// Minimal config.toml for listing.
fn create_minimal_config(codex_home: &std::path::Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        r#"
model = "mock-model"
approval_policy = "never"
"#,
    )
}

#[tokio::test]
async fn thread_list_pagination_next_cursor_none_on_last_page() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    // Create three rollouts so we can paginate with limit=2.
    let _a = create_fake_rollout(
        codex_home.path(),
        "2025-01-02T12-00-00",
        "2025-01-02T12:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;
    let _b = create_fake_rollout(
        codex_home.path(),
        "2025-01-01T13-00-00",
        "2025-01-01T13:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;
    let _c = create_fake_rollout(
        codex_home.path(),
        "2025-01-01T12-00-00",
        "2025-01-01T12:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Page 1: limit 2 → expect next_cursor Some.
    let page1_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: Some(2),
            model_providers: Some(vec!["mock_provider".to_string()]),
        })
        .await?;
    let page1_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(page1_id)),
    )
    .await??;
    let ThreadListResponse {
        data: data1,
        next_cursor: cursor1,
    } = to_response::<ThreadListResponse>(page1_resp)?;
    assert_eq!(data1.len(), 2);
    for thread in &data1 {
        assert_eq!(thread.preview, "Hello");
        assert_eq!(thread.model_provider, "mock_provider");
        assert!(thread.created_at > 0);
        assert_eq!(thread.cwd, PathBuf::from("/"));
        assert_eq!(thread.cli_version, "0.0.0");
        assert_eq!(thread.source, SessionSource::Cli);
        assert_eq!(thread.git_info, None);
    }
    let cursor1 = cursor1.expect("expected nextCursor on first page");

    // Page 2: with cursor → expect next_cursor None when no more results.
    let page2_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: Some(cursor1),
            limit: Some(2),
            model_providers: Some(vec!["mock_provider".to_string()]),
        })
        .await?;
    let page2_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(page2_id)),
    )
    .await??;
    let ThreadListResponse {
        data: data2,
        next_cursor: cursor2,
    } = to_response::<ThreadListResponse>(page2_resp)?;
    assert!(data2.len() <= 2);
    for thread in &data2 {
        assert_eq!(thread.preview, "Hello");
        assert_eq!(thread.model_provider, "mock_provider");
        assert!(thread.created_at > 0);
        assert_eq!(thread.cwd, PathBuf::from("/"));
        assert_eq!(thread.cli_version, "0.0.0");
        assert_eq!(thread.source, SessionSource::Cli);
        assert_eq!(thread.git_info, None);
    }
    assert_eq!(cursor2, None, "expected nextCursor to be null on last page");

    Ok(())
}

#[tokio::test]
async fn thread_list_respects_provider_filter() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    // Create rollouts under two providers.
    let _a = create_fake_rollout(
        codex_home.path(),
        "2025-01-02T10-00-00",
        "2025-01-02T10:00:00Z",
        "X",
        Some("mock_provider"),
        None,
    )?; // mock_provider
    let _b = create_fake_rollout(
        codex_home.path(),
        "2025-01-02T11-00-00",
        "2025-01-02T11:00:00Z",
        "X",
        Some("other_provider"),
        None,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Filter to only other_provider; expect 1 item, nextCursor None.
    let list_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: Some(10),
            model_providers: Some(vec!["other_provider".to_string()]),
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(list_id)),
    )
    .await??;
    let ThreadListResponse { data, next_cursor } = to_response::<ThreadListResponse>(resp)?;
    assert_eq!(data.len(), 1);
    assert_eq!(next_cursor, None);
    let thread = &data[0];
    assert_eq!(thread.preview, "X");
    assert_eq!(thread.model_provider, "other_provider");
    let expected_ts = chrono::DateTime::parse_from_rfc3339("2025-01-02T11:00:00Z")?.timestamp();
    assert_eq!(thread.created_at, expected_ts);
    assert_eq!(thread.cwd, PathBuf::from("/"));
    assert_eq!(thread.cli_version, "0.0.0");
    assert_eq!(thread.source, SessionSource::Cli);
    assert_eq!(thread.git_info, None);

    Ok(())
}

#[tokio::test]
async fn thread_list_includes_git_info() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let git_info = CoreGitInfo {
        commit_hash: Some("abc123".to_string()),
        branch: Some("main".to_string()),
        repository_url: Some("https://example.com/repo.git".to_string()),
    };
    let conversation_id = create_fake_rollout(
        codex_home.path(),
        "2025-02-01T09-00-00",
        "2025-02-01T09:00:00Z",
        "Git info preview",
        Some("mock_provider"),
        Some(git_info),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let list_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: Some(10),
            model_providers: Some(vec!["mock_provider".to_string()]),
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(list_id)),
    )
    .await??;
    let ThreadListResponse { data, .. } = to_response::<ThreadListResponse>(resp)?;
    let thread = data
        .iter()
        .find(|t| t.id == conversation_id)
        .expect("expected thread for created rollout");

    let expected_git = ApiGitInfo {
        sha: Some("abc123".to_string()),
        branch: Some("main".to_string()),
        origin_url: Some("https://example.com/repo.git".to_string()),
    };
    assert_eq!(thread.git_info, Some(expected_git));
    assert_eq!(thread.source, SessionSource::Cli);
    assert_eq!(thread.cwd, PathBuf::from("/"));
    assert_eq!(thread.cli_version, "0.0.0");

    Ok(())
}
