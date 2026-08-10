use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::rollout_path;
use app_test_support::to_response;
use codex_app_server_protocol::GitInfo;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadMetadataGitInfoUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadSection;
use codex_app_server_protocol::ThreadSectionListParams;
use codex_app_server_protocol::ThreadSectionListResponse;
use codex_app_server_protocol::ThreadSectionMoveParams;
use codex_app_server_protocol::ThreadSectionMoveResponse;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_core::ARCHIVED_SESSIONS_SUBDIR;
use codex_features::Feature;
use codex_git_utils::GitSha;
use codex_protocol::ThreadId;
use codex_protocol::protocol::GitInfo as RolloutGitInfo;
use codex_rollout::state_db::reconcile_rollout;
use codex_state::PINNED_THREAD_SECTION_ID;
use codex_state::PINNED_THREAD_SECTION_NAME;
use codex_state::StateRuntime;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test]
async fn thread_section_move_pins_and_unpins_with_filtered_recency_pagination() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let state_db = init_state_db(codex_home.path()).await?;

    let mut thread_ids = Vec::new();
    for (filename_timestamp, timestamp, preview) in [
        (
            "2025-01-06T08-00-00",
            "2025-01-06T08:00:00Z",
            "Older pinned",
        ),
        ("2025-01-06T09-00-00", "2025-01-06T09:00:00Z", "Unpinned"),
        (
            "2025-01-06T10-00-00",
            "2025-01-06T10:00:00Z",
            "Newer pinned",
        ),
    ] {
        let thread_id = create_fake_rollout(
            codex_home.path(),
            filename_timestamp,
            timestamp,
            preview,
            Some("mock_provider"),
            /*git_info*/ None,
        )?;
        reconcile_rollout(
            Some(&state_db),
            rollout_path(codex_home.path(), filename_timestamp, &thread_id).as_path(),
            "mock_provider",
            /*builder*/ None,
            &[],
            /*archived_only*/ None,
            /*new_thread_memory_mode*/ None,
        )
        .await;
        thread_ids.push(thread_id);
    }
    let [older_pinned, initially_unpinned, newer_pinned] = thread_ids.as_slice() else {
        unreachable!("three fake rollouts were created");
    };

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let pinned_section = ThreadSection {
        id: PINNED_THREAD_SECTION_ID.to_string(),
        name: PINNED_THREAD_SECTION_NAME.to_string(),
        appearance: None,
    };
    let section_list_id = mcp
        .send_raw_request(
            "threadSection/list",
            Some(serde_json::to_value(ThreadSectionListParams {
                cursor: None,
                limit: Some(1),
            })?),
        )
        .await?;
    let empty_membership_sections: ThreadSectionListResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(section_list_id)).await??;
    assert_eq!(empty_membership_sections.data, vec![pinned_section.clone()]);
    assert_eq!(empty_membership_sections.next_cursor, None);

    let unknown_section_id = "01984de2-8f74-7c91-a3b2-5c5e937cf319";
    let unknown_section_request_id = mcp
        .send_thread_section_move_request(ThreadSectionMoveParams {
            thread_id: initially_unpinned.clone(),
            section_id: Some(unknown_section_id.to_string()),
            before_thread_id: None,
        })
        .await?;
    let unknown_section_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(unknown_section_request_id)),
    )
    .await??;
    assert_eq!(unknown_section_error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        unknown_section_error.error.message,
        format!("section {unknown_section_id} does not exist")
    );

    for thread_id in [older_pinned, newer_pinned] {
        let request_id = mcp
            .send_thread_section_move_request(ThreadSectionMoveParams {
                thread_id: thread_id.clone(),
                section_id: Some(PINNED_THREAD_SECTION_ID.to_string()),
                before_thread_id: None,
            })
            .await?;
        let response = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
        )
        .await??;
        assert_eq!(
            to_response::<ThreadSectionMoveResponse>(response)?,
            ThreadSectionMoveResponse {}
        );
        let thread = state_db
            .get_thread(ThreadId::from_string(thread_id)?)
            .await?
            .expect("pinned thread should remain persisted");
        assert_eq!(
            thread.section,
            Some(codex_state::ThreadSection {
                id: pinned_section.id.clone(),
                name: pinned_section.name.clone(),
                appearance: None,
            })
        );
    }

    let list_params = ThreadListParams {
        cursor: None,
        limit: Some(1),
        sort_key: Some(ThreadSortKey::RecencyAt),
        sort_direction: None,
        model_providers: None,
        source_kinds: None,
        archived: None,
        section_id: Some(Some(PINNED_THREAD_SECTION_ID.to_string())),
        cwd: None,
        use_state_db_only: false,
        search_term: None,
        parent_thread_id: None,
        ancestor_thread_id: None,
    };
    let request_id = mcp.send_thread_list_request(list_params.clone()).await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let first_page: ThreadListResponse = to_response(response)?;
    assert_eq!(first_page.data.len(), 1);
    assert_eq!(first_page.data[0].id, *newer_pinned);
    assert_eq!(first_page.data[0].section, Some(pinned_section.clone()));

    let request_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: first_page.next_cursor,
            ..list_params.clone()
        })
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let second_page: ThreadListResponse = to_response(response)?;
    assert_eq!(second_page.data.len(), 1);
    assert_eq!(second_page.data[0].id, *older_pinned);
    assert_eq!(second_page.data[0].section, Some(pinned_section.clone()));

    let request_id = mcp
        .send_thread_section_move_request(ThreadSectionMoveParams {
            thread_id: newer_pinned.clone(),
            section_id: None,
            before_thread_id: None,
        })
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<ThreadSectionMoveResponse>(response)?,
        ThreadSectionMoveResponse {}
    );
    let thread = state_db
        .get_thread(ThreadId::from_string(newer_pinned)?)
        .await?
        .expect("unpinned thread should remain persisted");
    assert_eq!(
        (
            thread.section,
            thread.section_position,
            thread.section_entered_at,
        ),
        (None, None, None)
    );

    let request_id = mcp
        .send_thread_list_request(ThreadListParams {
            limit: Some(10),
            section_id: Some(None),
            ..list_params
        })
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let unsectioned_page: ThreadListResponse = to_response(response)?;
    assert_eq!(
        unsectioned_page
            .data
            .iter()
            .map(|thread| thread.id.as_str())
            .collect::<Vec<_>>(),
        [newer_pinned.as_str(), initially_unpinned.as_str()]
    );
    assert!(
        unsectioned_page
            .data
            .iter()
            .all(|thread| thread.section.is_none())
    );

    let section_list_id = mcp
        .send_raw_request(
            "threadSection/list",
            Some(serde_json::to_value(ThreadSectionListParams {
                cursor: None,
                limit: Some(1),
            })?),
        )
        .await?;
    let sections_after_clear: ThreadSectionListResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(section_list_id)).await??;
    assert_eq!(sections_after_clear.data, vec![pinned_section]);
    assert_eq!(sections_after_clear.next_cursor, None);

    Ok(())
}

#[tokio::test]
async fn thread_sections_preserve_server_owned_manual_order_across_moves_and_restarts() -> Result<()>
{
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let state_db = init_state_db(codex_home.path()).await?;

    let mut thread_ids = Vec::new();
    for (filename_timestamp, timestamp, preview) in [
        (
            "2025-01-06T08-00-00",
            "2025-01-06T08:00:00Z",
            "First pinned",
        ),
        (
            "2025-01-06T09-00-00",
            "2025-01-06T09:00:00Z",
            "Second pinned",
        ),
        (
            "2025-01-06T10-00-00",
            "2025-01-06T10:00:00Z",
            "Third pinned",
        ),
    ] {
        let thread_id = create_fake_rollout(
            codex_home.path(),
            filename_timestamp,
            timestamp,
            preview,
            Some("mock_provider"),
            /*git_info*/ None,
        )?;
        reconcile_rollout(
            Some(&state_db),
            rollout_path(codex_home.path(), filename_timestamp, &thread_id).as_path(),
            "mock_provider",
            /*builder*/ None,
            &[],
            /*archived_only*/ None,
            /*new_thread_memory_mode*/ None,
        )
        .await;
        thread_ids.push(thread_id);
    }
    let [first_pinned, second_pinned, third_pinned] = thread_ids.as_slice() else {
        unreachable!("three fake rollouts were created");
    };

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    for thread_id in [first_pinned, second_pinned, third_pinned] {
        let request_id = mcp
            .send_thread_section_move_request(ThreadSectionMoveParams {
                thread_id: thread_id.clone(),
                section_id: Some(PINNED_THREAD_SECTION_ID.to_string()),
                before_thread_id: None,
            })
            .await?;
        let response = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
        )
        .await??;
        assert_eq!(
            to_response::<ThreadSectionMoveResponse>(response)?,
            ThreadSectionMoveResponse {}
        );
    }

    let list_params = ThreadListParams {
        cursor: None,
        limit: Some(10),
        sort_key: Some(ThreadSortKey::SectionPosition),
        sort_direction: None,
        model_providers: None,
        source_kinds: None,
        archived: None,
        section_id: Some(Some(PINNED_THREAD_SECTION_ID.to_string())),
        cwd: None,
        use_state_db_only: false,
        search_term: None,
        parent_thread_id: None,
        ancestor_thread_id: None,
    };
    let request_id = mcp.send_thread_list_request(list_params.clone()).await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let initial: ThreadListResponse = to_response(response)?;
    assert_eq!(
        initial
            .data
            .iter()
            .map(|thread| thread.id.as_str())
            .collect::<Vec<_>>(),
        [
            first_pinned.as_str(),
            second_pinned.as_str(),
            third_pinned.as_str(),
        ]
    );
    let third_entered_at = initial.data[2].section_entered_at;
    assert!(third_entered_at.is_some());

    let request_id = mcp
        .send_thread_section_move_request(ThreadSectionMoveParams {
            thread_id: third_pinned.clone(),
            section_id: Some(PINNED_THREAD_SECTION_ID.to_string()),
            before_thread_id: Some(first_pinned.clone()),
        })
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<ThreadSectionMoveResponse>(response)?,
        ThreadSectionMoveResponse {}
    );

    let request_id = mcp.send_thread_list_request(list_params.clone()).await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let reordered: ThreadListResponse = to_response(response)?;
    assert_eq!(
        reordered
            .data
            .iter()
            .map(|thread| thread.id.as_str())
            .collect::<Vec<_>>(),
        [
            third_pinned.as_str(),
            first_pinned.as_str(),
            second_pinned.as_str()
        ]
    );
    assert_eq!(reordered.data[0].section_entered_at, third_entered_at);

    drop(mcp);
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let request_id = mcp.send_thread_list_request(list_params).await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let persisted: ThreadListResponse = to_response(response)?;
    assert_eq!(
        persisted
            .data
            .iter()
            .map(|thread| thread.id.as_str())
            .collect::<Vec<_>>(),
        [
            third_pinned.as_str(),
            first_pinned.as_str(),
            second_pinned.as_str()
        ]
    );
    assert_eq!(persisted.data[0].section_entered_at, third_entered_at);

    Ok(())
}

#[tokio::test]
async fn thread_metadata_update_patches_git_branch_and_returns_updated_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
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

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread.id.clone(),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: None,
                branch: Some(Some("feature/sidebar-pr".to_string())),
                origin_url: None,
            }),
        })
        .await?;
    let update_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_id)),
    )
    .await??;
    let update_result = update_resp.result.clone();
    let ThreadMetadataUpdateResponse { thread: updated } =
        to_response::<ThreadMetadataUpdateResponse>(update_resp)?;

    assert_eq!(updated.id, thread.id);
    assert_eq!(updated.session_id, thread.session_id);
    assert_eq!(
        updated.git_info,
        Some(GitInfo {
            sha: None,
            branch: Some("feature/sidebar-pr".to_string()),
            origin_url: None,
        })
    );
    assert_eq!(updated.status, ThreadStatus::Idle);
    let updated_thread_json = update_result
        .get("thread")
        .and_then(Value::as_object)
        .expect("thread/metadata/update result.thread must be an object");
    assert_eq!(
        updated_thread_json.get("sessionId").and_then(Value::as_str),
        Some(thread.session_id.as_str())
    );
    let updated_git_info_json = updated_thread_json
        .get("gitInfo")
        .and_then(Value::as_object)
        .expect("thread/metadata/update must serialize `thread.gitInfo` on the wire");
    assert_eq!(
        updated_git_info_json.get("branch").and_then(Value::as_str),
        Some("feature/sidebar-pr")
    );

    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id,
            include_turns: false,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let ThreadReadResponse { thread: read, .. } = to_response::<ThreadReadResponse>(read_resp)?;

    assert_eq!(
        read.git_info,
        Some(GitInfo {
            sha: None,
            branch: Some("feature/sidebar-pr".to_string()),
            origin_url: None,
        })
    );
    assert_eq!(read.status, ThreadStatus::Idle);

    Ok(())
}

#[tokio::test]
async fn thread_metadata_update_rejects_empty_git_info_patch() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
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

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread.id,
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: None,
                branch: None,
                origin_url: None,
            }),
        })
        .await?;
    let update_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(update_id)),
    )
    .await??;

    assert_eq!(
        update_err.error.message,
        "gitInfo must include at least one field"
    );

    Ok(())
}

#[tokio::test]
async fn thread_metadata_update_rejects_ephemeral_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread.id.clone(),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: None,
                branch: Some(Some("feature/ephemeral".to_string())),
                origin_url: None,
            }),
        })
        .await?;
    let update_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(update_id)),
    )
    .await??;

    assert_eq!(update_err.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        update_err.error.message,
        format!(
            "ephemeral thread does not support metadata updates: {}",
            thread.id
        )
    );

    let clear_section_id = mcp
        .send_thread_section_move_request(ThreadSectionMoveParams {
            thread_id: thread.id.clone(),
            section_id: None,
            before_thread_id: None,
        })
        .await?;
    let clear_section_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(clear_section_id)),
    )
    .await??;

    assert_eq!(clear_section_err.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        clear_section_err.error.message,
        format!(
            "ephemeral thread does not support section moves: {}",
            thread.id
        )
    );

    Ok(())
}

#[tokio::test]
async fn thread_metadata_update_repairs_missing_sqlite_row_for_stored_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let _state_db = init_state_db(codex_home.path()).await?;

    let preview = "Stored thread preview";
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        preview,
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread_id.clone(),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: None,
                branch: Some(Some("feature/stored-thread".to_string())),
                origin_url: None,
            }),
        })
        .await?;
    let update_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_id)),
    )
    .await??;
    let ThreadMetadataUpdateResponse { thread: updated } =
        to_response::<ThreadMetadataUpdateResponse>(update_resp)?;

    assert_eq!(updated.id, thread_id);
    assert_eq!(updated.preview, preview);
    assert_eq!(updated.created_at, 1736078400);
    assert_eq!(
        updated.git_info,
        Some(GitInfo {
            sha: None,
            branch: Some("feature/stored-thread".to_string()),
            origin_url: None,
        })
    );

    Ok(())
}

#[tokio::test]
async fn thread_metadata_update_repairs_loaded_thread_without_resetting_summary() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let state_db = init_state_db(codex_home.path()).await?;

    let preview = "Loaded thread preview";
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-06T08-30-00",
        "2025-01-06T08:30:00Z",
        preview,
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let thread_uuid = ThreadId::from_string(&thread_id)?;
    let rollout_path = rollout_path(codex_home.path(), "2025-01-06T08-30-00", &thread_id);
    reconcile_rollout(
        Some(&state_db),
        rollout_path.as_path(),
        "mock_provider",
        /*builder*/ None,
        &[],
        /*archived_only*/ None,
        /*new_thread_memory_mode*/ None,
    )
    .await;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let _: ThreadResumeResponse = to_response::<ThreadResumeResponse>(resume_resp)?;

    assert_eq!(state_db.delete_thread(thread_uuid).await?, 1);

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread_id.clone(),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: None,
                branch: Some(Some("feature/loaded-thread".to_string())),
                origin_url: None,
            }),
        })
        .await?;
    let update_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_id)),
    )
    .await??;
    let ThreadMetadataUpdateResponse { thread: updated } =
        to_response::<ThreadMetadataUpdateResponse>(update_resp)?;

    assert_eq!(updated.id, thread_id);
    assert_eq!(updated.preview, preview);
    assert_eq!(updated.created_at, 1736152200);
    assert_eq!(
        updated.git_info,
        Some(GitInfo {
            sha: None,
            branch: Some("feature/loaded-thread".to_string()),
            origin_url: None,
        })
    );

    Ok(())
}

#[tokio::test]
async fn thread_metadata_update_repairs_missing_sqlite_row_for_archived_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let _state_db = init_state_db(codex_home.path()).await?;

    let preview = "Archived thread preview";
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-06T08-30-00",
        "2025-01-06T08:30:00Z",
        preview,
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let archived_dir = codex_home.path().join(ARCHIVED_SESSIONS_SUBDIR);
    fs::create_dir_all(&archived_dir)?;
    let archived_source = rollout_path(codex_home.path(), "2025-01-06T08-30-00", &thread_id);
    let archived_dest = archived_dir.join(
        archived_source
            .file_name()
            .expect("archived rollout should have a file name"),
    );
    fs::rename(&archived_source, &archived_dest)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread_id.clone(),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: None,
                branch: Some(Some("feature/archived-thread".to_string())),
                origin_url: None,
            }),
        })
        .await?;
    let update_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_id)),
    )
    .await??;
    let ThreadMetadataUpdateResponse { thread: updated } =
        to_response::<ThreadMetadataUpdateResponse>(update_resp)?;

    assert_eq!(updated.id, thread_id);
    assert_eq!(updated.preview, preview);
    assert_eq!(updated.created_at, 1736152200);
    assert_eq!(
        updated.git_info,
        Some(GitInfo {
            sha: None,
            branch: Some("feature/archived-thread".to_string()),
            origin_url: None,
        })
    );

    Ok(())
}

#[tokio::test]
async fn thread_metadata_update_can_clear_stored_git_fields() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-07T09-15-00",
        "2025-01-07T09:15:00Z",
        "Thread preview",
        Some("mock_provider"),
        Some(RolloutGitInfo {
            commit_hash: Some(GitSha::new("abc123")),
            branch: Some("feature/sidebar-pr".to_string()),
            repository_url: Some("git@example.com:openai/codex.git".to_string()),
        }),
    )?;
    let _state_db = init_state_db(codex_home.path()).await?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread_id.clone(),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: Some(None),
                branch: Some(None),
                origin_url: Some(None),
            }),
        })
        .await?;
    let update_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_id)),
    )
    .await??;
    let ThreadMetadataUpdateResponse { thread: updated } =
        to_response::<ThreadMetadataUpdateResponse>(update_resp)?;

    assert_eq!(updated.id, thread_id.clone());
    assert_eq!(updated.git_info, None);

    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id,
            include_turns: false,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let ThreadReadResponse { thread: read, .. } = to_response::<ThreadReadResponse>(read_resp)?;

    assert_eq!(read.git_info, None);

    Ok(())
}

async fn init_state_db(codex_home: &Path) -> Result<Arc<StateRuntime>> {
    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.abs()),
        "mock_provider".into(),
    )
    .await?;
    state_db
        .mark_backfill_complete(/*last_watermark*/ None)
        .await?;
    Ok(state_db)
}

fn mock_responses_config(server_uri: &str) -> MockResponsesConfig {
    MockResponsesConfig::new(server_uri)
        .with_root_config("suppress_unstable_features_warning = true")
        .enable_feature(Feature::Sqlite)
}
