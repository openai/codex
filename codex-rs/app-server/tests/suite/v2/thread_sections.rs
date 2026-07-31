use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server::INVALID_PARAMS_ERROR_CODE;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadSection;
use codex_app_server_protocol::ThreadSectionCreateParams;
use codex_app_server_protocol::ThreadSectionCreateResponse;
use codex_app_server_protocol::ThreadSectionDeleteParams;
use codex_app_server_protocol::ThreadSectionDeleteResponse;
use codex_app_server_protocol::ThreadSectionListParams;
use codex_app_server_protocol::ThreadSectionListResponse;
use codex_app_server_protocol::ThreadSectionMoveParams;
use codex_app_server_protocol::ThreadSectionMoveResponse;
use codex_app_server_protocol::ThreadSectionUpdateParams;
use codex_app_server_protocol::ThreadSectionUpdateResponse;
use codex_app_server_protocol::ThreadUnarchiveParams;
use codex_app_server_protocol::ThreadUnarchiveResponse;
use codex_features::Feature;
use codex_state::PINNED_THREAD_SECTION_ID;
use codex_state::PINNED_THREAD_SECTION_NAME;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use uuid::Uuid;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

async fn section_request_error(
    server: &mut TestAppServer,
    method: &str,
    params: Value,
) -> Result<JSONRPCError> {
    let request_id = server.send_raw_request(method, Some(params)).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await?
}

#[tokio::test]
async fn custom_sections_remain_discoverable_across_ordered_updates_and_restart() -> Result<()> {
    let responses = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses.uri())
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let created: ThreadSectionCreateResponse = server
        .request(|request_id| ClientRequest::ThreadSectionCreate {
            request_id,
            params: ThreadSectionCreateParams {
                name: "  Work  ".to_string(),
            },
        })
        .await?;
    assert_eq!(created.section.name, "Work");
    assert_eq!(Uuid::parse_str(&created.section.id)?.get_version_num(), 7);

    let retained: ThreadSectionCreateResponse = server
        .request(|request_id| ClientRequest::ThreadSectionCreate {
            request_id,
            params: ThreadSectionCreateParams {
                name: "Personal".to_string(),
            },
        })
        .await?;
    let discovered: ThreadSectionListResponse = server
        .request(|request_id| ClientRequest::ThreadSectionList {
            request_id,
            params: ThreadSectionListParams {
                cursor: None,
                limit: Some(20),
            },
        })
        .await?;
    assert_eq!(discovered.data.len(), 3);
    assert!(discovered.data.contains(&created.section));
    assert!(discovered.data.contains(&retained.section));

    let first_rename = server
        .send_raw_request(
            "threadSection/update",
            Some(json!({ "sectionId": created.section.id, "name": "Queued work" })),
        )
        .await?;
    let second_rename = server
        .send_raw_request(
            "threadSection/update",
            Some(json!({ "sectionId": created.section.id, "name": "  Projects  " })),
        )
        .await?;
    let listed_after_renames = server
        .send_raw_request("threadSection/list", Some(json!({ "limit": 20 })))
        .await?;
    let _: ThreadSectionUpdateResponse =
        timeout(DEFAULT_READ_TIMEOUT, server.read_response(first_rename)).await??;
    let renamed: ThreadSectionUpdateResponse =
        timeout(DEFAULT_READ_TIMEOUT, server.read_response(second_rename)).await??;
    let observed: ThreadSectionListResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_response(listed_after_renames),
    )
    .await??;
    assert_eq!(
        renamed.section,
        ThreadSection {
            id: created.section.id,
            name: "Projects".to_string(),
        }
    );
    assert!(observed.data.contains(&renamed.section));

    drop(server);
    let mut restarted = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let persisted: ThreadSectionListResponse = restarted
        .request(|request_id| ClientRequest::ThreadSectionList {
            request_id,
            params: ThreadSectionListParams {
                cursor: None,
                limit: Some(20),
            },
        })
        .await?;
    assert_eq!(
        persisted.data,
        vec![
            ThreadSection {
                id: PINNED_THREAD_SECTION_ID.to_string(),
                name: PINNED_THREAD_SECTION_NAME.to_string(),
            },
            renamed.section,
            retained.section,
        ]
    );

    Ok(())
}

#[tokio::test]
async fn deleting_custom_sections_unassigns_active_and_archived_members() -> Result<()> {
    let responses = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses.uri())
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;
    let first_thread = create_fake_rollout(
        codex_home.path(),
        "2025-01-06T08-00-00",
        "2025-01-06T08:00:00Z",
        "First thread",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let archived_thread = create_fake_rollout(
        codex_home.path(),
        "2025-01-06T09-00-00",
        "2025-01-06T09:00:00Z",
        "Archived thread",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let request_id = server
        .send_raw_request("thread/list", Some(json!({ "limit": 20 })))
        .await?;
    let _: ThreadListResponse =
        timeout(DEFAULT_READ_TIMEOUT, server.read_response(request_id)).await??;

    let created: ThreadSectionCreateResponse = server
        .request(|request_id| ClientRequest::ThreadSectionCreate {
            request_id,
            params: ThreadSectionCreateParams {
                name: "Work".to_string(),
            },
        })
        .await?;
    for thread_id in [&first_thread, &archived_thread] {
        let _: ThreadSectionMoveResponse = server
            .request(|request_id| ClientRequest::ThreadSectionMove {
                request_id,
                params: ThreadSectionMoveParams {
                    thread_id: thread_id.clone(),
                    section_id: Some(created.section.id.clone()),
                    before_thread_id: None,
                },
            })
            .await?;
    }
    let _: ThreadArchiveResponse = server
        .request(|request_id| ClientRequest::ThreadArchive {
            request_id,
            params: ThreadArchiveParams {
                thread_id: archived_thread.clone(),
            },
        })
        .await?;

    let renamed: ThreadSectionUpdateResponse = server
        .request(|request_id| ClientRequest::ThreadSectionUpdate {
            request_id,
            params: ThreadSectionUpdateParams {
                section_id: created.section.id.clone(),
                name: "Projects".to_string(),
            },
        })
        .await?;
    let renamed_member: ThreadReadResponse = server
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: first_thread.clone(),
                include_turns: false,
            },
        })
        .await?;
    assert_eq!(renamed_member.thread.section, Some(renamed.section));

    let _: ThreadSectionDeleteResponse = server
        .request(|request_id| ClientRequest::ThreadSectionDelete {
            request_id,
            params: ThreadSectionDeleteParams {
                section_id: created.section.id,
            },
        })
        .await?;
    let unsectioned: ThreadReadResponse = server
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: first_thread,
                include_turns: false,
            },
        })
        .await?;
    assert_eq!(
        (
            unsectioned.thread.section,
            unsectioned.thread.section_entered_at
        ),
        (None, None)
    );
    let restored: ThreadUnarchiveResponse = server
        .request(|request_id| ClientRequest::ThreadUnarchive {
            request_id,
            params: ThreadUnarchiveParams {
                thread_id: archived_thread.clone(),
            },
        })
        .await?;
    assert_eq!(
        (restored.thread.section, restored.thread.section_entered_at),
        (None, None)
    );

    drop(server);
    let mut restarted = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let restored_after_restart: ThreadReadResponse = restarted
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: archived_thread,
                include_turns: false,
            },
        })
        .await?;
    assert_eq!(
        (
            restored_after_restart.thread.section,
            restored_after_restart.thread.section_entered_at
        ),
        (None, None)
    );

    Ok(())
}

#[tokio::test]
async fn custom_section_management_rejects_empty_names_missing_ids_and_pinned_mutations()
-> Result<()> {
    let responses = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses.uri())
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let missing_id = Uuid::now_v7().to_string();

    for (method, params, message) in [
        (
            "threadSection/create",
            json!({ "name": "  " }),
            "section name must not be empty",
        ),
        (
            "threadSection/update",
            json!({ "sectionId": " ", "name": "Work" }),
            "sectionId must not be empty",
        ),
        (
            "threadSection/update",
            json!({ "sectionId": PINNED_THREAD_SECTION_ID, "name": "Pinned again" }),
            "the built-in pinned section cannot be renamed",
        ),
        (
            "threadSection/delete",
            json!({ "sectionId": PINNED_THREAD_SECTION_ID }),
            "the built-in pinned section cannot be deleted",
        ),
    ] {
        let error = section_request_error(&mut server, method, params).await?;
        assert_eq!(error.error.code, INVALID_PARAMS_ERROR_CODE);
        assert_eq!(error.error.message, message);
    }

    for (method, params) in [
        (
            "threadSection/update",
            json!({ "sectionId": missing_id, "name": "Work" }),
        ),
        ("threadSection/delete", json!({ "sectionId": missing_id })),
    ] {
        let error = section_request_error(&mut server, method, params).await?;
        assert_eq!(error.error.code, INVALID_PARAMS_ERROR_CODE);
        assert_eq!(
            error.error.message,
            format!("thread section not found: {missing_id}")
        );
    }

    Ok(())
}
