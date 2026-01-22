use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_fake_rollout;
use app_test_support::to_response;
use codex_app_server_protocol::GitInfo as ApiGitInfo;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadListResponse;
use codex_protocol::protocol::GitInfo as CoreGitInfo;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

async fn init_mcp(codex_home: &Path) -> Result<McpProcess> {
    let mut mcp = McpProcess::new(codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    Ok(mcp)
}

async fn list_threads(
    mcp: &mut McpProcess,
    cursor: Option<String>,
    limit: Option<u32>,
    providers: Option<Vec<String>>,
) -> Result<ThreadListResponse> {
    let request_id = mcp
        .send_thread_list_request(codex_app_server_protocol::ThreadListParams {
            cursor,
            limit,
            model_providers: providers,
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response::<ThreadListResponse>(resp)
}

fn create_fake_rollouts<F, G>(
    codex_home: &Path,
    count: usize,
    provider_for_index: F,
    timestamp_for_index: G,
    preview: &str,
) -> Result<Vec<String>>
where
    F: Fn(usize) -> &'static str,
    G: Fn(usize) -> (String, String),
{
    let mut ids = Vec::with_capacity(count);
    for i in 0..count {
        let (ts_file, ts_rfc) = timestamp_for_index(i);
        ids.push(create_fake_rollout(
            codex_home,
            &ts_file,
            &ts_rfc,
            preview,
            Some(provider_for_index(i)),
            None,
        )?);
    }
    Ok(ids)
}

fn timestamp_at(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> (String, String) {
    (
        format!("{year:04}-{month:02}-{day:02}T{hour:02}-{minute:02}-{second:02}"),
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"),
    )
}

#[tokio::test]
async fn thread_list_basic_empty() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse { data, next_cursor } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
    )
    .await?;
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

    let mut mcp = init_mcp(codex_home.path()).await?;

    // Page 1: limit 2 → expect next_cursor Some.
    let ThreadListResponse {
        data: data1,
        next_cursor: cursor1,
    } = list_threads(
        &mut mcp,
        None,
        Some(2),
        Some(vec!["mock_provider".to_string()]),
    )
    .await?;
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
    let ThreadListResponse {
        data: data2,
        next_cursor: cursor2,
    } = list_threads(
        &mut mcp,
        Some(cursor1),
        Some(2),
        Some(vec!["mock_provider".to_string()]),
    )
    .await?;
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

    let mut mcp = init_mcp(codex_home.path()).await?;

    // Filter to only other_provider; expect 1 item, nextCursor None.
    let ThreadListResponse { data, next_cursor } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["other_provider".to_string()]),
    )
    .await?;
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
async fn thread_list_fetches_until_limit_or_exhausted() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    // Newest 16 conversations belong to a different provider; the older 8 are the
    // only ones that match the filter. We request 8 so the server must keep
    // paging past the first two pages to reach the desired count.
    create_fake_rollouts(
        codex_home.path(),
        24,
        |i| {
            if i < 16 {
                "skip_provider"
            } else {
                "target_provider"
            }
        },
        |i| timestamp_at(2025, 3, 30 - i as u32, 12, 0, 0),
        "Hello",
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    // Request 8 threads for the target provider; the matches only start on the
    // third page so we rely on pagination to reach the limit.
    let ThreadListResponse { data, next_cursor } = list_threads(
        &mut mcp,
        None,
        Some(8),
        Some(vec!["target_provider".to_string()]),
    )
    .await?;
    assert_eq!(
        data.len(),
        8,
        "should keep paging until the requested count is filled"
    );
    assert!(
        data.iter()
            .all(|thread| thread.model_provider == "target_provider"),
        "all returned threads must match the requested provider"
    );
    assert_eq!(
        next_cursor, None,
        "once the requested count is satisfied on the final page, nextCursor should be None"
    );

    Ok(())
}

#[tokio::test]
async fn thread_list_enforces_max_limit() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    create_fake_rollouts(
        codex_home.path(),
        105,
        |_| "mock_provider",
        |i| {
            let month = 5 + (i / 28);
            let day = (i % 28) + 1;
            timestamp_at(2025, month as u32, day as u32, 0, 0, 0)
        },
        "Hello",
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse { data, next_cursor } = list_threads(
        &mut mcp,
        None,
        Some(200),
        Some(vec!["mock_provider".to_string()]),
    )
    .await?;
    assert_eq!(
        data.len(),
        100,
        "limit should be clamped to the maximum page size"
    );
    assert!(
        next_cursor.is_some(),
        "when more than the maximum exist, nextCursor should continue pagination"
    );

    Ok(())
}

#[tokio::test]
async fn thread_list_stops_when_not_enough_filtered_results_exist() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    // Only the last 7 conversations match the provider filter; we ask for 10 to
    // ensure the server exhausts pagination without looping forever.
    create_fake_rollouts(
        codex_home.path(),
        22,
        |i| {
            if i < 15 {
                "skip_provider"
            } else {
                "target_provider"
            }
        },
        |i| timestamp_at(2025, 4, 28 - i as u32, 8, 0, 0),
        "Hello",
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    // Request more threads than exist after filtering; expect all matches to be
    // returned with nextCursor None.
    let ThreadListResponse { data, next_cursor } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["target_provider".to_string()]),
    )
    .await?;
    assert_eq!(
        data.len(),
        7,
        "all available filtered threads should be returned"
    );
    assert!(
        data.iter()
            .all(|thread| thread.model_provider == "target_provider"),
        "results should still respect the provider filter"
    );
    assert_eq!(
        next_cursor, None,
        "when results are exhausted before reaching the limit, nextCursor should be None"
    );

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

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse { data, .. } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
    )
    .await?;
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
