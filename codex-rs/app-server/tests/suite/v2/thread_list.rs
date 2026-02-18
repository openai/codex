use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_fake_rollout;
use app_test_support::create_fake_rollout_with_source;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::rollout_path;
use app_test_support::to_response;
use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::GitInfo as ApiGitInfo;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_core::ARCHIVED_SESSIONS_SUBDIR;
use codex_protocol::ThreadId;
use codex_protocol::protocol::GitInfo as CoreGitInfo;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionSource as CoreSessionSource;
use codex_protocol::protocol::SubAgentSource;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use std::cmp::Reverse;
use std::fs;
use std::fs::FileTimes;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::timeout;
use uuid::Uuid;

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
    source_kinds: Option<Vec<ThreadSourceKind>>,
    archived: Option<bool>,
) -> Result<ThreadListResponse> {
    list_threads_with_sort(mcp, cursor, limit, providers, source_kinds, None, archived).await
}

async fn list_threads_with_sort(
    mcp: &mut McpProcess,
    cursor: Option<String>,
    limit: Option<u32>,
    providers: Option<Vec<String>>,
    source_kinds: Option<Vec<ThreadSourceKind>>,
    sort_key: Option<ThreadSortKey>,
    archived: Option<bool>,
) -> Result<ThreadListResponse> {
    let request_id = mcp
        .send_thread_list_request(codex_app_server_protocol::ThreadListParams {
            cursor,
            limit,
            sort_key,
            model_providers: providers,
            source_kinds,
            archived,
            cwd: None,
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

#[allow(dead_code)]
fn set_rollout_mtime(path: &Path, updated_at_rfc3339: &str) -> Result<()> {
    let parsed = DateTime::parse_from_rfc3339(updated_at_rfc3339)?.with_timezone(&Utc);
    let times = FileTimes::new().set_modified(parsed.into());
    OpenOptions::new()
        .append(true)
        .open(path)?
        .set_times(times)?;
    Ok(())
}

fn set_rollout_cwd(path: &Path, cwd: &Path) -> Result<()> {
    let content = fs::read_to_string(path)?;
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    let first_line = lines
        .first_mut()
        .ok_or_else(|| anyhow::anyhow!("rollout at {} is empty", path.display()))?;
    let mut rollout_line: RolloutLine = serde_json::from_str(first_line)?;
    let RolloutItem::SessionMeta(mut session_meta_line) = rollout_line.item else {
        return Err(anyhow::anyhow!(
            "rollout at {} does not start with session metadata",
            path.display()
        ));
    };
    session_meta_line.meta.cwd = cwd.to_path_buf();
    rollout_line.item = RolloutItem::SessionMeta(session_meta_line);
    *first_line = serde_json::to_string(&rollout_line)?;
    fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

#[tokio::test]
async fn thread_list_basic_empty() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse {
        data, next_cursor, ..
    } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        None,
        None,
    )
    .await?;
    assert!(data.is_empty());
    assert_eq!(next_cursor, None);

    Ok(())
}

#[tokio::test]
async fn thread_list_reports_system_error_idle_flag_after_failed_turn() -> Result<()> {
    let responses = vec![
        create_final_assistant_message_sse_response("seeded")?,
        responses::sse_failed("resp-2", "server_error", "simulated failure"),
    ];
    let server = create_mock_responses_server_sequence(responses).await;

    let codex_home = TempDir::new()?;
    create_runtime_config(codex_home.path(), &server.uri())?;
    let mut mcp = init_mcp(codex_home.path()).await?;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let seed_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "seed history".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let seed_turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(seed_turn_id)),
    )
    .await??;
    let _: TurnStartResponse = to_response::<TurnStartResponse>(seed_turn_resp)?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let failed_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "fail turn".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let failed_turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(failed_turn_id)),
    )
    .await??;
    let _: TurnStartResponse = to_response::<TurnStartResponse>(failed_turn_resp)?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("error"),
    )
    .await??;

    let ThreadListResponse { data, .. } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        Some(vec![
            ThreadSourceKind::AppServer,
            ThreadSourceKind::Cli,
            ThreadSourceKind::VsCode,
        ]),
        None,
    )
    .await?;
    let listed = data
        .iter()
        .find(|candidate| candidate.id == thread.id)
        .expect("expected started thread to be listed");
    assert_eq!(listed.status, ThreadStatus::SystemError,);

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

fn create_runtime_config(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
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
        None,
        None,
    )
    .await?;
    assert_eq!(data1.len(), 2);
    for thread in &data1 {
        assert_eq!(thread.preview, "Hello");
        assert_eq!(thread.model_provider, "mock_provider");
        assert!(thread.created_at > 0);
        assert_eq!(thread.updated_at, thread.created_at);
        assert_eq!(thread.cwd, PathBuf::from("/"));
        assert_eq!(thread.cli_version, "0.0.0");
        assert_eq!(thread.source, SessionSource::Cli);
        assert_eq!(thread.git_info, None);
        assert_eq!(thread.status, ThreadStatus::NotLoaded);
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
        None,
        None,
    )
    .await?;
    assert!(data2.len() <= 2);
    for thread in &data2 {
        assert_eq!(thread.preview, "Hello");
        assert_eq!(thread.model_provider, "mock_provider");
        assert!(thread.created_at > 0);
        assert_eq!(thread.updated_at, thread.created_at);
        assert_eq!(thread.cwd, PathBuf::from("/"));
        assert_eq!(thread.cli_version, "0.0.0");
        assert_eq!(thread.source, SessionSource::Cli);
        assert_eq!(thread.git_info, None);
        assert_eq!(thread.status, ThreadStatus::NotLoaded);
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
    let ThreadListResponse {
        data, next_cursor, ..
    } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["other_provider".to_string()]),
        None,
        None,
    )
    .await?;
    assert_eq!(data.len(), 1);
    assert_eq!(next_cursor, None);
    let thread = &data[0];
    assert_eq!(thread.preview, "X");
    assert_eq!(thread.model_provider, "other_provider");
    let expected_ts = chrono::DateTime::parse_from_rfc3339("2025-01-02T11:00:00Z")?.timestamp();
    assert_eq!(thread.created_at, expected_ts);
    assert_eq!(thread.updated_at, expected_ts);
    assert_eq!(thread.cwd, PathBuf::from("/"));
    assert_eq!(thread.cli_version, "0.0.0");
    assert_eq!(thread.source, SessionSource::Cli);
    assert_eq!(thread.git_info, None);

    Ok(())
}

#[tokio::test]
async fn thread_list_respects_cwd_filter() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let filtered_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-02T10-00-00",
        "2025-01-02T10:00:00Z",
        "filtered",
        Some("mock_provider"),
        None,
    )?;
    let unfiltered_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-02T11-00-00",
        "2025-01-02T11:00:00Z",
        "unfiltered",
        Some("mock_provider"),
        None,
    )?;

    let target_cwd = codex_home.path().join("target-cwd");
    fs::create_dir_all(&target_cwd)?;
    set_rollout_cwd(
        rollout_path(codex_home.path(), "2025-01-02T10-00-00", &filtered_id).as_path(),
        &target_cwd,
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;
    let request_id = mcp
        .send_thread_list_request(codex_app_server_protocol::ThreadListParams {
            cursor: None,
            limit: Some(10),
            sort_key: None,
            model_providers: Some(vec!["mock_provider".to_string()]),
            source_kinds: None,
            archived: None,
            cwd: Some(target_cwd.to_string_lossy().into_owned()),
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadListResponse {
        data, next_cursor, ..
    } = to_response::<ThreadListResponse>(resp)?;

    assert_eq!(next_cursor, None);
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].id, filtered_id);
    assert_ne!(data[0].id, unfiltered_id);
    assert_eq!(data[0].cwd, target_cwd);

    Ok(())
}

#[tokio::test]
async fn thread_list_empty_source_kinds_defaults_to_interactive_only() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let cli_id = create_fake_rollout(
        codex_home.path(),
        "2025-02-01T10-00-00",
        "2025-02-01T10:00:00Z",
        "CLI",
        Some("mock_provider"),
        None,
    )?;
    let exec_id = create_fake_rollout_with_source(
        codex_home.path(),
        "2025-02-01T11-00-00",
        "2025-02-01T11:00:00Z",
        "Exec",
        Some("mock_provider"),
        None,
        CoreSessionSource::Exec,
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse {
        data, next_cursor, ..
    } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        Some(Vec::new()),
        None,
    )
    .await?;

    assert_eq!(next_cursor, None);
    let ids: Vec<_> = data.iter().map(|thread| thread.id.as_str()).collect();
    assert_eq!(ids, vec![cli_id.as_str()]);
    assert_ne!(cli_id, exec_id);
    assert_eq!(data[0].source, SessionSource::Cli);

    Ok(())
}

#[tokio::test]
async fn thread_list_filters_by_source_kind_subagent_thread_spawn() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let cli_id = create_fake_rollout(
        codex_home.path(),
        "2025-02-01T10-00-00",
        "2025-02-01T10:00:00Z",
        "CLI",
        Some("mock_provider"),
        None,
    )?;

    let parent_thread_id = ThreadId::from_string(&Uuid::new_v4().to_string())?;
    let subagent_id = create_fake_rollout_with_source(
        codex_home.path(),
        "2025-02-01T11-00-00",
        "2025-02-01T11:00:00Z",
        "SubAgent",
        Some("mock_provider"),
        None,
        CoreSessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth: 1,
        }),
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse {
        data, next_cursor, ..
    } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        Some(vec![ThreadSourceKind::SubAgentThreadSpawn]),
        None,
    )
    .await?;

    assert_eq!(next_cursor, None);
    let ids: Vec<_> = data.iter().map(|thread| thread.id.as_str()).collect();
    assert_eq!(ids, vec![subagent_id.as_str()]);
    assert_ne!(cli_id, subagent_id);
    assert!(matches!(data[0].source, SessionSource::SubAgent(_)));

    Ok(())
}

#[tokio::test]
async fn thread_list_filters_by_subagent_variant() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let parent_thread_id = ThreadId::from_string(&Uuid::new_v4().to_string())?;

    let review_id = create_fake_rollout_with_source(
        codex_home.path(),
        "2025-02-02T09-00-00",
        "2025-02-02T09:00:00Z",
        "Review",
        Some("mock_provider"),
        None,
        CoreSessionSource::SubAgent(SubAgentSource::Review),
    )?;
    let compact_id = create_fake_rollout_with_source(
        codex_home.path(),
        "2025-02-02T10-00-00",
        "2025-02-02T10:00:00Z",
        "Compact",
        Some("mock_provider"),
        None,
        CoreSessionSource::SubAgent(SubAgentSource::Compact),
    )?;
    let spawn_id = create_fake_rollout_with_source(
        codex_home.path(),
        "2025-02-02T11-00-00",
        "2025-02-02T11:00:00Z",
        "Spawn",
        Some("mock_provider"),
        None,
        CoreSessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth: 1,
        }),
    )?;
    let other_id = create_fake_rollout_with_source(
        codex_home.path(),
        "2025-02-02T12-00-00",
        "2025-02-02T12:00:00Z",
        "Other",
        Some("mock_provider"),
        None,
        CoreSessionSource::SubAgent(SubAgentSource::Other("custom".to_string())),
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let review = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        Some(vec![ThreadSourceKind::SubAgentReview]),
        None,
    )
    .await?;
    let review_ids: Vec<_> = review
        .data
        .iter()
        .map(|thread| thread.id.as_str())
        .collect();
    assert_eq!(review_ids, vec![review_id.as_str()]);

    let compact = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        Some(vec![ThreadSourceKind::SubAgentCompact]),
        None,
    )
    .await?;
    let compact_ids: Vec<_> = compact
        .data
        .iter()
        .map(|thread| thread.id.as_str())
        .collect();
    assert_eq!(compact_ids, vec![compact_id.as_str()]);

    let spawn = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        Some(vec![ThreadSourceKind::SubAgentThreadSpawn]),
        None,
    )
    .await?;
    let spawn_ids: Vec<_> = spawn.data.iter().map(|thread| thread.id.as_str()).collect();
    assert_eq!(spawn_ids, vec![spawn_id.as_str()]);

    let other = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        Some(vec![ThreadSourceKind::SubAgentOther]),
        None,
    )
    .await?;
    let other_ids: Vec<_> = other.data.iter().map(|thread| thread.id.as_str()).collect();
    assert_eq!(other_ids, vec![other_id.as_str()]);

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
    let ThreadListResponse {
        data, next_cursor, ..
    } = list_threads(
        &mut mcp,
        None,
        Some(8),
        Some(vec!["target_provider".to_string()]),
        None,
        None,
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

    let ThreadListResponse {
        data, next_cursor, ..
    } = list_threads(
        &mut mcp,
        None,
        Some(200),
        Some(vec!["mock_provider".to_string()]),
        None,
        None,
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
    let ThreadListResponse {
        data, next_cursor, ..
    } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["target_provider".to_string()]),
        None,
        None,
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
        None,
        None,
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

#[tokio::test]
async fn thread_list_default_sorts_by_created_at() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let id_a = create_fake_rollout(
        codex_home.path(),
        "2025-01-02T12-00-00",
        "2025-01-02T12:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;
    let id_b = create_fake_rollout(
        codex_home.path(),
        "2025-01-01T13-00-00",
        "2025-01-01T13:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;
    let id_c = create_fake_rollout(
        codex_home.path(),
        "2025-01-01T12-00-00",
        "2025-01-01T12:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse { data, .. } = list_threads_with_sort(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        None,
        None,
        None,
    )
    .await?;

    let ids: Vec<_> = data.iter().map(|thread| thread.id.as_str()).collect();
    assert_eq!(ids, vec![id_a.as_str(), id_b.as_str(), id_c.as_str()]);

    Ok(())
}

#[tokio::test]
async fn thread_list_sort_updated_at_orders_by_mtime() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let id_old = create_fake_rollout(
        codex_home.path(),
        "2025-01-01T10-00-00",
        "2025-01-01T10:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;
    let id_mid = create_fake_rollout(
        codex_home.path(),
        "2025-01-01T11-00-00",
        "2025-01-01T11:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;
    let id_new = create_fake_rollout(
        codex_home.path(),
        "2025-01-01T12-00-00",
        "2025-01-01T12:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;

    set_rollout_mtime(
        rollout_path(codex_home.path(), "2025-01-01T10-00-00", &id_old).as_path(),
        "2025-01-03T00:00:00Z",
    )?;
    set_rollout_mtime(
        rollout_path(codex_home.path(), "2025-01-01T11-00-00", &id_mid).as_path(),
        "2025-01-02T00:00:00Z",
    )?;
    set_rollout_mtime(
        rollout_path(codex_home.path(), "2025-01-01T12-00-00", &id_new).as_path(),
        "2025-01-01T00:00:00Z",
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse { data, .. } = list_threads_with_sort(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        None,
        Some(ThreadSortKey::UpdatedAt),
        None,
    )
    .await?;

    let ids: Vec<_> = data.iter().map(|thread| thread.id.as_str()).collect();
    assert_eq!(ids, vec![id_old.as_str(), id_mid.as_str(), id_new.as_str()]);

    Ok(())
}

#[tokio::test]
async fn thread_list_updated_at_paginates_with_cursor() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let id_a = create_fake_rollout(
        codex_home.path(),
        "2025-02-01T10-00-00",
        "2025-02-01T10:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;
    let id_b = create_fake_rollout(
        codex_home.path(),
        "2025-02-01T11-00-00",
        "2025-02-01T11:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;
    let id_c = create_fake_rollout(
        codex_home.path(),
        "2025-02-01T12-00-00",
        "2025-02-01T12:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;

    set_rollout_mtime(
        rollout_path(codex_home.path(), "2025-02-01T10-00-00", &id_a).as_path(),
        "2025-02-03T00:00:00Z",
    )?;
    set_rollout_mtime(
        rollout_path(codex_home.path(), "2025-02-01T11-00-00", &id_b).as_path(),
        "2025-02-02T00:00:00Z",
    )?;
    set_rollout_mtime(
        rollout_path(codex_home.path(), "2025-02-01T12-00-00", &id_c).as_path(),
        "2025-02-01T00:00:00Z",
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse {
        data: page1,
        next_cursor: cursor1,
        ..
    } = list_threads_with_sort(
        &mut mcp,
        None,
        Some(2),
        Some(vec!["mock_provider".to_string()]),
        None,
        Some(ThreadSortKey::UpdatedAt),
        None,
    )
    .await?;
    let ids_page1: Vec<_> = page1.iter().map(|thread| thread.id.as_str()).collect();
    assert_eq!(ids_page1, vec![id_a.as_str(), id_b.as_str()]);
    let cursor1 = cursor1.expect("expected nextCursor on first page");

    let ThreadListResponse {
        data: page2,
        next_cursor: cursor2,
        ..
    } = list_threads_with_sort(
        &mut mcp,
        Some(cursor1),
        Some(2),
        Some(vec!["mock_provider".to_string()]),
        None,
        Some(ThreadSortKey::UpdatedAt),
        None,
    )
    .await?;
    let ids_page2: Vec<_> = page2.iter().map(|thread| thread.id.as_str()).collect();
    assert_eq!(ids_page2, vec![id_c.as_str()]);
    assert_eq!(cursor2, None);

    Ok(())
}

#[tokio::test]
async fn thread_list_created_at_tie_breaks_by_uuid() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let id_a = create_fake_rollout(
        codex_home.path(),
        "2025-02-01T10-00-00",
        "2025-02-01T10:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;
    let id_b = create_fake_rollout(
        codex_home.path(),
        "2025-02-01T10-00-00",
        "2025-02-01T10:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse { data, .. } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        None,
        None,
    )
    .await?;

    let ids: Vec<_> = data.iter().map(|thread| thread.id.as_str()).collect();
    let mut expected = [id_a, id_b];
    expected.sort_by_key(|id| Reverse(Uuid::parse_str(id).expect("uuid should parse")));
    let expected: Vec<_> = expected.iter().map(String::as_str).collect();
    assert_eq!(ids, expected);

    Ok(())
}

#[tokio::test]
async fn thread_list_updated_at_tie_breaks_by_uuid() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let id_a = create_fake_rollout(
        codex_home.path(),
        "2025-02-01T10-00-00",
        "2025-02-01T10:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;
    let id_b = create_fake_rollout(
        codex_home.path(),
        "2025-02-01T11-00-00",
        "2025-02-01T11:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;

    let updated_at = "2025-02-03T00:00:00Z";
    set_rollout_mtime(
        rollout_path(codex_home.path(), "2025-02-01T10-00-00", &id_a).as_path(),
        updated_at,
    )?;
    set_rollout_mtime(
        rollout_path(codex_home.path(), "2025-02-01T11-00-00", &id_b).as_path(),
        updated_at,
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse { data, .. } = list_threads_with_sort(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        None,
        Some(ThreadSortKey::UpdatedAt),
        None,
    )
    .await?;

    let ids: Vec<_> = data.iter().map(|thread| thread.id.as_str()).collect();
    let mut expected = [id_a, id_b];
    expected.sort_by_key(|id| Reverse(Uuid::parse_str(id).expect("uuid should parse")));
    let expected: Vec<_> = expected.iter().map(String::as_str).collect();
    assert_eq!(ids, expected);

    Ok(())
}

#[tokio::test]
async fn thread_list_updated_at_uses_mtime() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-02-01T10-00-00",
        "2025-02-01T10:00:00Z",
        "Hello",
        Some("mock_provider"),
        None,
    )?;

    set_rollout_mtime(
        rollout_path(codex_home.path(), "2025-02-01T10-00-00", &thread_id).as_path(),
        "2025-02-05T00:00:00Z",
    )?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse { data, .. } = list_threads_with_sort(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        None,
        Some(ThreadSortKey::UpdatedAt),
        None,
    )
    .await?;

    let thread = data
        .iter()
        .find(|item| item.id == thread_id)
        .expect("expected thread for created rollout");
    let expected_created =
        chrono::DateTime::parse_from_rfc3339("2025-02-01T10:00:00Z")?.timestamp();
    let expected_updated =
        chrono::DateTime::parse_from_rfc3339("2025-02-05T00:00:00Z")?.timestamp();
    assert_eq!(thread.created_at, expected_created);
    assert_eq!(thread.updated_at, expected_updated);

    Ok(())
}

#[tokio::test]
async fn thread_list_archived_filter() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let active_id = create_fake_rollout(
        codex_home.path(),
        "2025-03-01T10-00-00",
        "2025-03-01T10:00:00Z",
        "Active",
        Some("mock_provider"),
        None,
    )?;
    let archived_id = create_fake_rollout(
        codex_home.path(),
        "2025-03-01T09-00-00",
        "2025-03-01T09:00:00Z",
        "Archived",
        Some("mock_provider"),
        None,
    )?;

    let archived_dir = codex_home.path().join(ARCHIVED_SESSIONS_SUBDIR);
    fs::create_dir_all(&archived_dir)?;
    let archived_source = rollout_path(codex_home.path(), "2025-03-01T09-00-00", &archived_id);
    let archived_dest = archived_dir.join(
        archived_source
            .file_name()
            .expect("archived rollout should have a file name"),
    );
    fs::rename(&archived_source, &archived_dest)?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let ThreadListResponse { data, .. } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        None,
        None,
    )
    .await?;
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].id, active_id);

    let ThreadListResponse { data, .. } = list_threads(
        &mut mcp,
        None,
        Some(10),
        Some(vec!["mock_provider".to_string()]),
        None,
        Some(true),
    )
    .await?;
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].id, archived_id);

    Ok(())
}

#[tokio::test]
async fn thread_list_invalid_cursor_returns_error() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let mut mcp = init_mcp(codex_home.path()).await?;

    let request_id = mcp
        .send_thread_list_request(codex_app_server_protocol::ThreadListParams {
            cursor: Some("not-a-cursor".to_string()),
            limit: Some(2),
            sort_key: None,
            model_providers: Some(vec!["mock_provider".to_string()]),
            source_kinds: None,
            archived: None,
            cwd: None,
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, -32600);
    assert_eq!(error.error.message, "invalid cursor: not-a-cursor");

    Ok(())
}
