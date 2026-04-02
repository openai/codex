use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_fake_rollout;
use app_test_support::rollout_path;
use app_test_support::to_response;
use codex_app_server_protocol::ConversationSummary;
use codex_app_server_protocol::GetConversationSummaryParams;
use codex_app_server_protocol::GetConversationSummaryResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const FILENAME_TS: &str = "2025-01-02T12-00-00";
const META_RFC3339: &str = "2025-01-02T12:00:00Z";
const PREVIEW: &str = "Summarize this conversation";
const MODEL_PROVIDER: &str = "openai";

fn expected_summary(conversation_id: ThreadId, path: PathBuf) -> ConversationSummary {
    ConversationSummary {
        conversation_id,
        path,
        preview: PREVIEW.to_string(),
        timestamp: Some(META_RFC3339.to_string()),
        updated_at: Some(META_RFC3339.to_string()),
        model_provider: MODEL_PROVIDER.to_string(),
        cwd: PathBuf::from("/"),
        cli_version: "0.0.0".to_string(),
        source: SessionSource::Cli,
        git_info: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_conversation_summary_by_thread_id_reads_rollout() -> Result<()> {
    let codex_home = TempDir::new()?;
    let conversation_id = create_fake_rollout(
        codex_home.path(),
        FILENAME_TS,
        META_RFC3339,
        PREVIEW,
        Some(MODEL_PROVIDER),
        /*git_info*/ None,
    )?;
    let thread_id = ThreadId::from_string(&conversation_id)?;
    let expected = expected_summary(
        thread_id,
        std::fs::canonicalize(rollout_path(
            codex_home.path(),
            FILENAME_TS,
            &conversation_id,
        ))?,
    );

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_get_conversation_summary_request(GetConversationSummaryParams::ThreadId {
            conversation_id: thread_id,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: GetConversationSummaryResponse = to_response(response)?;

    assert_eq!(received.summary, expected);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_conversation_summary_by_relative_rollout_path_resolves_from_codex_home() -> Result<()>
{
    let codex_home = TempDir::new()?;
    let conversation_id = create_fake_rollout(
        codex_home.path(),
        FILENAME_TS,
        META_RFC3339,
        PREVIEW,
        Some(MODEL_PROVIDER),
        /*git_info*/ None,
    )?;
    let thread_id = ThreadId::from_string(&conversation_id)?;
    let rollout_path = rollout_path(codex_home.path(), FILENAME_TS, &conversation_id);
    let relative_path = rollout_path.strip_prefix(codex_home.path())?.to_path_buf();
    let expected = expected_summary(thread_id, std::fs::canonicalize(rollout_path)?);

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_get_conversation_summary_request(GetConversationSummaryParams::RolloutPath {
            rollout_path: relative_path,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: GetConversationSummaryResponse = to_response(response)?;

    assert_eq!(received.summary, expected);
    Ok(())
}
