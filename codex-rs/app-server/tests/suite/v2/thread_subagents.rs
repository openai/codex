use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadSubagent;
use codex_app_server_protocol::ThreadSubagentLifecycleStatus;
use codex_app_server_protocol::ThreadSubagentsListParams;
use codex_app_server_protocol::ThreadSubagentsListResponse;
use codex_app_server_protocol::ThreadSubagentsReadParams;
use codex_app_server_protocol::ThreadSubagentsReadResponse;
use codex_protocol::ThreadId;
use codex_state::DirectionalThreadSpawnEdgeStatus;
use codex_state::StateRuntime;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_subagents_list_and_read_use_persisted_edges_without_rollouts() -> Result<()> {
    let codex_home = TempDir::new()?;
    let parent_thread_id = thread_id(/*value*/ 100)?;
    let open_child_thread_id = thread_id(/*value*/ 101)?;
    let closed_child_thread_id = thread_id(/*value*/ 102)?;
    let grandchild_thread_id = thread_id(/*value*/ 103)?;
    let missing_child_thread_id = thread_id(/*value*/ 104)?;
    let state_db =
        StateRuntime::init(codex_home.path().to_path_buf(), "mock_provider".into()).await?;
    state_db
        .mark_backfill_complete(/*last_watermark*/ None)
        .await?;
    state_db
        .upsert_thread_spawn_edge(
            parent_thread_id,
            open_child_thread_id,
            DirectionalThreadSpawnEdgeStatus::Open,
        )
        .await?;
    state_db
        .upsert_thread_spawn_edge(
            parent_thread_id,
            closed_child_thread_id,
            DirectionalThreadSpawnEdgeStatus::Closed,
        )
        .await?;
    state_db
        .upsert_thread_spawn_edge(
            open_child_thread_id,
            grandchild_thread_id,
            DirectionalThreadSpawnEdgeStatus::Open,
        )
        .await?;
    assert!(!codex_home.path().join("sessions").exists());

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let first_page = list_subagents(
        &mut mcp,
        ThreadSubagentsListParams {
            limit: Some(1),
            ..list_params(parent_thread_id)
        },
    )
    .await?;
    let next_cursor = first_page.next_cursor.expect("first page cursor");
    assert_ne!(next_cursor, open_child_thread_id.to_string());
    assert_eq!(
        first_page.data,
        vec![subagent(
            parent_thread_id,
            open_child_thread_id,
            ThreadSubagentLifecycleStatus::Open,
        )]
    );

    let second_page = list_subagents(
        &mut mcp,
        ThreadSubagentsListParams {
            cursor: Some(next_cursor),
            limit: Some(1),
            ..list_params(parent_thread_id)
        },
    )
    .await?;
    assert_eq!(
        second_page,
        ThreadSubagentsListResponse {
            data: vec![subagent(
                parent_thread_id,
                closed_child_thread_id,
                ThreadSubagentLifecycleStatus::Closed,
            )],
            next_cursor: None,
        }
    );

    assert_eq!(
        list_subagents(
            &mut mcp,
            ThreadSubagentsListParams {
                lifecycle_statuses: Some(vec![ThreadSubagentLifecycleStatus::Open]),
                ..list_params(parent_thread_id)
            },
        )
        .await?,
        ThreadSubagentsListResponse {
            data: vec![subagent(
                parent_thread_id,
                open_child_thread_id,
                ThreadSubagentLifecycleStatus::Open,
            )],
            next_cursor: None,
        }
    );
    assert_eq!(
        list_subagents(
            &mut mcp,
            ThreadSubagentsListParams {
                lifecycle_statuses: Some(vec![ThreadSubagentLifecycleStatus::Closed]),
                ..list_params(parent_thread_id)
            },
        )
        .await?,
        ThreadSubagentsListResponse {
            data: vec![subagent(
                parent_thread_id,
                closed_child_thread_id,
                ThreadSubagentLifecycleStatus::Closed,
            )],
            next_cursor: None,
        }
    );

    for (child_thread_id, expected) in [
        (
            open_child_thread_id,
            Some(subagent(
                parent_thread_id,
                open_child_thread_id,
                ThreadSubagentLifecycleStatus::Open,
            )),
        ),
        (
            closed_child_thread_id,
            Some(subagent(
                parent_thread_id,
                closed_child_thread_id,
                ThreadSubagentLifecycleStatus::Closed,
            )),
        ),
        (missing_child_thread_id, None),
    ] {
        assert_eq!(read_subagent(&mut mcp, child_thread_id).await?, expected);
    }
    assert!(!codex_home.path().join("sessions").exists());
    Ok(())
}

#[tokio::test]
async fn thread_subagents_reject_malformed_ids_cursor_and_limits() -> Result<()> {
    let codex_home = TempDir::new()?;
    let state_db =
        StateRuntime::init(codex_home.path().to_path_buf(), "mock_provider".into()).await?;
    state_db
        .mark_backfill_complete(/*last_watermark*/ None)
        .await?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    for params in [
        ThreadSubagentsListParams {
            parent_thread_id: "not-a-thread-id".to_string(),
            cursor: None,
            limit: None,
            lifecycle_statuses: None,
        },
        ThreadSubagentsListParams {
            cursor: Some("not-a-cursor".to_string()),
            ..list_params(thread_id(/*value*/ 200)?)
        },
        ThreadSubagentsListParams {
            limit: Some(0),
            ..list_params(thread_id(/*value*/ 200)?)
        },
        ThreadSubagentsListParams {
            limit: Some(101),
            ..list_params(thread_id(/*value*/ 200)?)
        },
    ] {
        let request_id = mcp.send_thread_subagents_list_request(params).await?;
        assert_invalid_request(&mut mcp, request_id).await?;
    }

    let request_id = mcp
        .send_thread_subagents_read_request(ThreadSubagentsReadParams {
            child_thread_id: "not-a-thread-id".to_string(),
        })
        .await?;
    assert_invalid_request(&mut mcp, request_id).await?;
    Ok(())
}

async fn list_subagents(
    mcp: &mut McpProcess,
    params: ThreadSubagentsListParams,
) -> Result<ThreadSubagentsListResponse> {
    let request_id = mcp.send_thread_subagents_list_request(params).await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn read_subagent(
    mcp: &mut McpProcess,
    child_thread_id: ThreadId,
) -> Result<Option<ThreadSubagent>> {
    let request_id = mcp
        .send_thread_subagents_read_request(ThreadSubagentsReadParams {
            child_thread_id: child_thread_id.to_string(),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    Ok(to_response::<ThreadSubagentsReadResponse>(response)?.subagent)
}

async fn assert_invalid_request(mcp: &mut McpProcess, request_id: i64) -> Result<()> {
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, -32600);
    Ok(())
}

fn subagent(
    parent_thread_id: ThreadId,
    child_thread_id: ThreadId,
    lifecycle_status: ThreadSubagentLifecycleStatus,
) -> ThreadSubagent {
    ThreadSubagent {
        child_thread_id: child_thread_id.to_string(),
        parent_thread_id: parent_thread_id.to_string(),
        lifecycle_status,
    }
}

fn list_params(parent_thread_id: ThreadId) -> ThreadSubagentsListParams {
    ThreadSubagentsListParams {
        parent_thread_id: parent_thread_id.to_string(),
        cursor: None,
        limit: None,
        lifecycle_statuses: None,
    }
}

fn thread_id(value: u128) -> Result<ThreadId> {
    ThreadId::from_string(&uuid::Uuid::from_u128(value).to_string()).map_err(Into::into)
}
