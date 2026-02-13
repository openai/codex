use anyhow::Result;
use anyhow::anyhow;
use app_test_support::McpProcess;
use codex_app_server_protocol::FuzzyFileSearchSessionCompletedNotification;
use codex_app_server_protocol::FuzzyFileSearchSessionUpdatedNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const SHORT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);
const STOP_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_millis(250);
const SESSION_UPDATED_METHOD: &str = "fuzzyFileSearch/sessionUpdated";
const SESSION_COMPLETED_METHOD: &str = "fuzzyFileSearch/sessionCompleted";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileExpectation {
    Any,
    Empty,
    NonEmpty,
}

async fn initialized_mcp(codex_home: &TempDir) -> Result<McpProcess> {
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    Ok(mcp)
}

async fn wait_for_session_updated(
    mcp: &mut McpProcess,
    session_id: &str,
    query: &str,
    file_expectation: FileExpectation,
) -> Result<FuzzyFileSearchSessionUpdatedNotification> {
    for _ in 0..20 {
        let notification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message(SESSION_UPDATED_METHOD),
        )
        .await??;
        let params = notification
            .params
            .ok_or_else(|| anyhow!("missing notification params"))?;
        let payload = serde_json::from_value::<FuzzyFileSearchSessionUpdatedNotification>(params)?;
        if payload.session_id != session_id || payload.query != query {
            continue;
        }
        let files_match = match file_expectation {
            FileExpectation::Any => true,
            FileExpectation::Empty => payload.files.is_empty(),
            FileExpectation::NonEmpty => !payload.files.is_empty(),
        };
        if files_match {
            return Ok(payload);
        }
    }
    anyhow::bail!(
        "did not receive expected session update for sessionId={session_id}, query={query}"
    );
}

async fn wait_for_session_completed(
    mcp: &mut McpProcess,
    session_id: &str,
) -> Result<FuzzyFileSearchSessionCompletedNotification> {
    for _ in 0..20 {
        let notification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message(SESSION_COMPLETED_METHOD),
        )
        .await??;
        let params = notification
            .params
            .ok_or_else(|| anyhow!("missing notification params"))?;
        let payload =
            serde_json::from_value::<FuzzyFileSearchSessionCompletedNotification>(params)?;
        if payload.session_id == session_id {
            return Ok(payload);
        }
    }

    anyhow::bail!("did not receive expected session completion for sessionId={session_id}");
}

async fn assert_update_request_fails_for_missing_session(
    mcp: &mut McpProcess,
    session_id: &str,
    query: &str,
) -> Result<()> {
    let request_id = mcp
        .send_fuzzy_file_search_session_update_request(session_id, query)
        .await?;
    let err = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(err.error.code, -32600);
    assert_eq!(
        err.error.message,
        format!("fuzzy file search session not found: {session_id}")
    );
    Ok(())
}

async fn assert_no_session_updates_for(
    mcp: &mut McpProcess,
    session_id: &str,
    grace_period: std::time::Duration,
    duration: std::time::Duration,
) -> Result<()> {
    let grace_deadline = tokio::time::Instant::now() + grace_period;
    loop {
        let now = tokio::time::Instant::now();
        if now >= grace_deadline {
            break;
        }
        let remaining = grace_deadline - now;
        match timeout(
            remaining,
            mcp.read_stream_until_notification_message(SESSION_UPDATED_METHOD),
        )
        .await
        {
            Err(_) => break,
            Ok(Err(err)) => return Err(err),
            Ok(Ok(_)) => {}
        }
    }

    let deadline = tokio::time::Instant::now() + duration;
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ok(());
        }
        let remaining = deadline - now;
        match timeout(
            remaining,
            mcp.read_stream_until_notification_message(SESSION_UPDATED_METHOD),
        )
        .await
        {
            Err(_) => return Ok(()),
            Ok(Err(err)) => return Err(err),
            Ok(Ok(notification)) => {
                let params = notification
                    .params
                    .ok_or_else(|| anyhow!("missing notification params"))?;
                let payload =
                    serde_json::from_value::<FuzzyFileSearchSessionUpdatedNotification>(params)?;
                if payload.session_id == session_id {
                    anyhow::bail!("received unexpected session update after stop: {payload:?}");
                }
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_sorts_and_includes_indices() -> Result<()> {
    // Prepare a temporary Codex home and a separate root with test files.
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;

    // Create files designed to have deterministic ordering for query "abe".
    std::fs::write(root.path().join("abc"), "x")?;
    std::fs::write(root.path().join("abcde"), "x")?;
    std::fs::write(root.path().join("abexy"), "x")?;
    std::fs::write(root.path().join("zzz.txt"), "x")?;
    let sub_dir = root.path().join("sub");
    std::fs::create_dir_all(&sub_dir)?;
    let sub_abce_path = sub_dir.join("abce");
    std::fs::write(&sub_abce_path, "x")?;
    let sub_abce_rel = sub_abce_path
        .strip_prefix(root.path())?
        .to_string_lossy()
        .to_string();

    // Start MCP server and initialize.
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let root_path = root.path().to_string_lossy().to_string();
    // Send fuzzyFileSearch request.
    let request_id = mcp
        .send_fuzzy_file_search_request("abe", vec![root_path.clone()], None)
        .await?;

    // Read response and verify shape and ordering.
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let value = resp.result;
    let expected_score = 72;

    assert_eq!(
        value,
        json!({
            "files": [
                {
                    "root": root_path.clone(),
                    "path": "abexy",
                    "file_name": "abexy",
                    "score": 84,
                    "indices": [0, 1, 2],
                },
                {
                    "root": root_path.clone(),
                    "path": sub_abce_rel,
                    "file_name": "abce",
                    "score": expected_score,
                    "indices": [4, 5, 7],
                },
                {
                    "root": root_path.clone(),
                    "path": "abcde",
                    "file_name": "abcde",
                    "score": 71,
                    "indices": [0, 1, 4],
                },
            ]
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_accepts_cancellation_token() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;

    std::fs::write(root.path().join("alpha.txt"), "contents")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let root_path = root.path().to_string_lossy().to_string();
    let request_id = mcp
        .send_fuzzy_file_search_request("alp", vec![root_path.clone()], None)
        .await?;

    let request_id_2 = mcp
        .send_fuzzy_file_search_request(
            "alp",
            vec![root_path.clone()],
            Some(request_id.to_string()),
        )
        .await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id_2)),
    )
    .await??;

    let files = resp
        .result
        .get("files")
        .ok_or_else(|| anyhow!("files key missing"))?
        .as_array()
        .ok_or_else(|| anyhow!("files not array"))?
        .clone();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["root"], root_path);
    assert_eq!(files[0]["path"], "alpha.txt");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_streams_updates() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    std::fs::write(root.path().join("alpha.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_path = root.path().to_string_lossy().to_string();
    let session_id = "session-1";

    mcp.start_fuzzy_file_search_session(session_id, vec![root_path.clone()])
        .await?;
    mcp.update_fuzzy_file_search_session(session_id, "alp")
        .await?;

    let payload =
        wait_for_session_updated(&mut mcp, session_id, "alp", FileExpectation::NonEmpty).await?;
    assert_eq!(payload.files.len(), 1);
    assert_eq!(payload.files[0].root, root_path);
    assert_eq!(payload.files[0].path, "alpha.txt");
    let completed = wait_for_session_completed(&mut mcp, session_id).await?;
    assert_eq!(completed.session_id, session_id);

    mcp.stop_fuzzy_file_search_session(session_id).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_no_updates_after_complete_until_query_edited() -> Result<()>
{
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    std::fs::write(root.path().join("alpha.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_path = root.path().to_string_lossy().to_string();
    let session_id = "session-complete-invariant";
    mcp.start_fuzzy_file_search_session(session_id, vec![root_path])
        .await?;

    mcp.update_fuzzy_file_search_session(session_id, "alp")
        .await?;
    wait_for_session_updated(&mut mcp, session_id, "alp", FileExpectation::NonEmpty).await?;
    wait_for_session_completed(&mut mcp, session_id).await?;
    assert_no_session_updates_for(&mut mcp, session_id, STOP_GRACE_PERIOD, SHORT_READ_TIMEOUT)
        .await?;

    mcp.update_fuzzy_file_search_session(session_id, "alpha")
        .await?;
    wait_for_session_updated(&mut mcp, session_id, "alpha", FileExpectation::NonEmpty).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_update_before_start_errors() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = initialized_mcp(&codex_home).await?;
    assert_update_request_fails_for_missing_session(&mut mcp, "missing", "alp").await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_update_works_without_waiting_for_start_response()
-> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    std::fs::write(root.path().join("alpha.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_path = root.path().to_string_lossy().to_string();
    let session_id = "session-no-wait";

    let start_request_id = mcp
        .send_fuzzy_file_search_session_start_request(session_id, vec![root_path.clone()])
        .await?;
    let update_request_id = mcp
        .send_fuzzy_file_search_session_update_request(session_id, "alp")
        .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_request_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_request_id)),
    )
    .await??;

    let payload =
        wait_for_session_updated(&mut mcp, session_id, "alp", FileExpectation::NonEmpty).await?;
    assert_eq!(payload.files.len(), 1);
    assert_eq!(payload.files[0].root, root_path);
    assert_eq!(payload.files[0].path, "alpha.txt");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_multiple_query_updates_work() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    std::fs::write(root.path().join("alpha.txt"), "contents")?;
    std::fs::write(root.path().join("alphabet.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_path = root.path().to_string_lossy().to_string();
    let session_id = "session-multi-update";
    mcp.start_fuzzy_file_search_session(session_id, vec![root_path.clone()])
        .await?;

    mcp.update_fuzzy_file_search_session(session_id, "alp")
        .await?;
    let alp_payload =
        wait_for_session_updated(&mut mcp, session_id, "alp", FileExpectation::NonEmpty).await?;
    assert_eq!(
        alp_payload.files.iter().all(|file| file.root == root_path),
        true
    );
    wait_for_session_completed(&mut mcp, session_id).await?;

    mcp.update_fuzzy_file_search_session(session_id, "zzzz")
        .await?;
    let zzzz_payload =
        wait_for_session_updated(&mut mcp, session_id, "zzzz", FileExpectation::Any).await?;
    assert_eq!(zzzz_payload.query, "zzzz");
    assert_eq!(zzzz_payload.files.is_empty(), true);
    wait_for_session_completed(&mut mcp, session_id).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_update_after_stop_fails() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    std::fs::write(root.path().join("alpha.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let session_id = "session-stop-fail";
    let root_path = root.path().to_string_lossy().to_string();
    mcp.start_fuzzy_file_search_session(session_id, vec![root_path])
        .await?;
    mcp.stop_fuzzy_file_search_session(session_id).await?;

    assert_update_request_fails_for_missing_session(&mut mcp, session_id, "alp").await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_stops_sending_updates_after_stop() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    for i in 0..10_000 {
        let file_path = root.path().join(format!("file-{i:04}.txt"));
        std::fs::write(file_path, "contents")?;
    }
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_path = root.path().to_string_lossy().to_string();
    let session_id = "session-stop-no-updates";
    mcp.start_fuzzy_file_search_session(session_id, vec![root_path])
        .await?;
    mcp.update_fuzzy_file_search_session(session_id, "file-")
        .await?;
    wait_for_session_updated(&mut mcp, session_id, "file-", FileExpectation::NonEmpty).await?;

    mcp.stop_fuzzy_file_search_session(session_id).await?;

    assert_no_session_updates_for(&mut mcp, session_id, STOP_GRACE_PERIOD, SHORT_READ_TIMEOUT)
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_two_sessions_are_independent() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root_a = TempDir::new()?;
    let root_b = TempDir::new()?;
    std::fs::write(root_a.path().join("alpha.txt"), "contents")?;
    std::fs::write(root_b.path().join("beta.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_a_path = root_a.path().to_string_lossy().to_string();
    let root_b_path = root_b.path().to_string_lossy().to_string();
    let session_a = "session-a";
    let session_b = "session-b";

    mcp.start_fuzzy_file_search_session(session_a, vec![root_a_path.clone()])
        .await?;
    mcp.start_fuzzy_file_search_session(session_b, vec![root_b_path.clone()])
        .await?;

    mcp.update_fuzzy_file_search_session(session_a, "alp")
        .await?;

    let session_a_update =
        wait_for_session_updated(&mut mcp, session_a, "alp", FileExpectation::NonEmpty).await?;
    assert_eq!(session_a_update.files.len(), 1);
    assert_eq!(session_a_update.files[0].root, root_a_path);
    assert_eq!(session_a_update.files[0].path, "alpha.txt");

    mcp.update_fuzzy_file_search_session(session_b, "bet")
        .await?;
    let session_b_update =
        wait_for_session_updated(&mut mcp, session_b, "bet", FileExpectation::NonEmpty).await?;
    assert_eq!(session_b_update.files.len(), 1);
    assert_eq!(session_b_update.files[0].root, root_b_path);
    assert_eq!(session_b_update.files[0].path, "beta.txt");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_query_cleared_sends_blank_snapshot() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    std::fs::write(root.path().join("alpha.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_path = root.path().to_string_lossy().to_string();
    let session_id = "session-clear-query";
    mcp.start_fuzzy_file_search_session(session_id, vec![root_path])
        .await?;

    mcp.update_fuzzy_file_search_session(session_id, "alp")
        .await?;
    wait_for_session_updated(&mut mcp, session_id, "alp", FileExpectation::NonEmpty).await?;

    mcp.update_fuzzy_file_search_session(session_id, "").await?;
    let payload =
        wait_for_session_updated(&mut mcp, session_id, "", FileExpectation::Empty).await?;
    assert_eq!(payload.files.is_empty(), true);

    Ok(())
}
