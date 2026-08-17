use std::collections::BTreeMap;

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::ProjectChangeType;
use codex_app_server_protocol::ProjectChangedNotification;
use codex_app_server_protocol::ProjectCreateParams;
use codex_app_server_protocol::ProjectCreateResponse;
use codex_app_server_protocol::ProjectDeleteParams;
use codex_app_server_protocol::ProjectDeleteResponse;
use codex_app_server_protocol::ProjectImportParams;
use codex_app_server_protocol::ProjectImportResponse;
use codex_app_server_protocol::ProjectListParams;
use codex_app_server_protocol::ProjectListResponse;
use codex_app_server_protocol::ProjectMoveParams;
use codex_app_server_protocol::ProjectMoveResponse;
use codex_app_server_protocol::ProjectReadParams;
use codex_app_server_protocol::ProjectReadResponse;
use codex_app_server_protocol::ProjectRoot;
use codex_app_server_protocol::ProjectUpdateParams;
use codex_app_server_protocol::ProjectUpdateResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadMetadataGitInfoUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateResponse;
use codex_app_server_protocol::ThreadProjectUpdatedNotification;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn projects_persist_and_assign_threads() -> Result<()> {
    let responses = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses.uri())
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;
    let root = AbsolutePathBuf::from_absolute_path(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let created: ProjectCreateResponse = server
        .request(|request_id| ClientRequest::ProjectCreate {
            request_id,
            params: ProjectCreateParams {
                name: "  Work  ".to_string(),
                roots: vec![ProjectRoot { path: root.clone() }],
                metadata: Some(BTreeMap::from([("color".to_string(), "blue".to_string())])),
                idempotency_key: "projects-persist-primary".to_string(),
            },
        })
        .await?;
    assert_eq!(created.project.name, "Work");
    assert_eq!(Uuid::parse_str(&created.project.id)?.get_version_num(), 7);

    let read: ProjectReadResponse = server
        .request(|request_id| ClientRequest::ProjectRead {
            request_id,
            params: ProjectReadParams {
                project_id: created.project.id.clone(),
            },
        })
        .await?;
    assert_eq!(read.project, created.project);

    server.clear_message_buffer();
    let started_id = server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            project_id: Some(created.project.id.clone()),
            ..Default::default()
        })
        .await?;
    let JSONRPCMessage::Response(response) = server.read_next_message().await? else {
        panic!("thread/start must respond before lifecycle notifications");
    };
    assert_eq!(response.id, RequestId::Integer(started_id));
    let started: ThreadStartResponse = serde_json::from_value(response.result)?;
    let JSONRPCMessage::Notification(thread_started) = server.read_next_message().await? else {
        panic!("thread/start must emit thread/started");
    };
    assert_eq!(thread_started.method, "thread/started");
    assert_eq!(started.thread.project_id, Some(created.project.id.clone()));

    server.clear_message_buffer();
    let ephemeral_id = server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            project_id: Some(created.project.id.clone()),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;
    let JSONRPCMessage::Response(response) = server.read_next_message().await? else {
        panic!("ephemeral thread/start must respond before lifecycle notifications");
    };
    assert_eq!(response.id, RequestId::Integer(ephemeral_id));
    let ephemeral: ThreadStartResponse = serde_json::from_value(response.result)?;
    let JSONRPCMessage::Notification(thread_started) = server.read_next_message().await? else {
        panic!("ephemeral thread/start must emit thread/started");
    };
    assert_eq!(thread_started.method, "thread/started");
    assert_eq!(
        ephemeral.thread.project_id,
        Some(created.project.id.clone())
    );
    assert!(ephemeral.thread.ephemeral);
    assert!(ephemeral.thread.path.is_none());
    server
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: started.thread.id.clone(),
            input: vec![UserInput::Text {
                text: "persist this thread".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;

    drop(server);
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let resumed: ThreadResumeResponse = server
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: started.thread.id.clone(),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(resumed.thread.project_id, Some(created.project.id.clone()));
    let read_after_resume: ThreadReadResponse = server
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: started.thread.id.clone(),
                include_turns: false,
            },
        })
        .await?;
    assert_eq!(
        read_after_resume.thread.project_id,
        Some(created.project.id.clone())
    );

    let listed: ThreadListResponse = server
        .request(|request_id| ClientRequest::ThreadList {
            request_id,
            params: ThreadListParams {
                cursor: None,
                limit: Some(10),
                sort_key: None,
                sort_direction: None,
                model_providers: Some(Vec::new()),
                source_kinds: Some(Vec::new()),
                archived: None,
                section_id: None,
                project_id: Some(Some(created.project.id.clone())),
                cwd: None,
                use_state_db_only: true,
                search_term: None,
                parent_thread_id: None,
                ancestor_thread_id: None,
            },
        })
        .await?;
    assert_eq!(listed.data.len(), 1);
    assert_eq!(listed.data[0].id, started.thread.id);

    server.clear_message_buffer();
    let cleared: ThreadMetadataUpdateResponse = server
        .request(|request_id| ClientRequest::ThreadMetadataUpdate {
            request_id,
            params: ThreadMetadataUpdateParams {
                thread_id: started.thread.id.clone(),
                project_id: Some(String::new()),
                git_info: None,
            },
        })
        .await?;
    assert_eq!(cleared.thread.project_id, None);
    let cleared_notification: ThreadProjectUpdatedNotification =
        server.read_notification("thread/project/updated").await?;
    assert_eq!(
        cleared_notification,
        ThreadProjectUpdatedNotification {
            thread_id: started.thread.id.clone(),
            project_id: None,
        }
    );
    let unassigned: ThreadListResponse = server
        .request(|request_id| ClientRequest::ThreadList {
            request_id,
            params: ThreadListParams {
                cursor: None,
                limit: Some(10),
                sort_key: None,
                sort_direction: None,
                model_providers: Some(Vec::new()),
                source_kinds: Some(Vec::new()),
                archived: None,
                section_id: None,
                project_id: Some(None),
                cwd: None,
                use_state_db_only: true,
                search_term: None,
                parent_thread_id: None,
                ancestor_thread_id: None,
            },
        })
        .await?;
    assert_eq!(unassigned.data.len(), 1);
    assert_eq!(unassigned.data[0].id, started.thread.id);

    server.clear_message_buffer();
    let reassigned: ThreadMetadataUpdateResponse = server
        .request(|request_id| ClientRequest::ThreadMetadataUpdate {
            request_id,
            params: ThreadMetadataUpdateParams {
                thread_id: started.thread.id.clone(),
                project_id: Some(created.project.id.clone()),
                git_info: None,
            },
        })
        .await?;
    assert_eq!(
        reassigned.thread.project_id,
        Some(created.project.id.clone())
    );
    let reassigned_notification: ThreadProjectUpdatedNotification =
        server.read_notification("thread/project/updated").await?;
    assert_eq!(
        reassigned_notification,
        ThreadProjectUpdatedNotification {
            thread_id: started.thread.id.clone(),
            project_id: Some(created.project.id.clone()),
        }
    );
    let reassigned_list: ThreadListResponse = server
        .request(|request_id| ClientRequest::ThreadList {
            request_id,
            params: ThreadListParams {
                cursor: None,
                limit: Some(10),
                sort_key: None,
                sort_direction: None,
                model_providers: Some(Vec::new()),
                source_kinds: Some(Vec::new()),
                archived: None,
                section_id: None,
                project_id: Some(Some(created.project.id.clone())),
                cwd: None,
                use_state_db_only: true,
                search_term: None,
                parent_thread_id: None,
                ancestor_thread_id: None,
            },
        })
        .await?;
    assert_eq!(reassigned_list.data.len(), 1);
    assert_eq!(reassigned_list.data[0].id, started.thread.id);
    server.clear_message_buffer();
    let unchanged: ThreadMetadataUpdateResponse = server
        .request(|request_id| ClientRequest::ThreadMetadataUpdate {
            request_id,
            params: ThreadMetadataUpdateParams {
                thread_id: started.thread.id.clone(),
                project_id: Some(created.project.id.clone()),
                git_info: None,
            },
        })
        .await?;
    assert_eq!(
        unchanged.thread.project_id,
        Some(created.project.id.clone())
    );
    assert!(
        !server
            .pending_notification_methods()
            .iter()
            .any(|method| method == "thread/project/updated")
    );

    let updated: ProjectUpdateResponse = server
        .request(|request_id| ClientRequest::ProjectUpdate {
            request_id,
            params: ProjectUpdateParams {
                project_id: created.project.id.clone(),
                name: Some("Renamed".to_string()),
                roots: Some(Vec::new()),
                metadata: Some(BTreeMap::new()),
            },
        })
        .await?;
    assert_eq!(updated.project.name, "Renamed");
    assert!(updated.project.roots.is_empty());
    assert!(updated.project.metadata.is_empty());
    let updated_notification: ProjectChangedNotification =
        server.read_notification("project/changed").await?;
    assert_eq!(
        updated_notification,
        ProjectChangedNotification {
            project_id: updated.project.id.clone(),
            change_type: ProjectChangeType::Updated,
        }
    );

    let unchanged_project: ProjectUpdateResponse = server
        .request(|request_id| ClientRequest::ProjectUpdate {
            request_id,
            params: ProjectUpdateParams {
                project_id: updated.project.id.clone(),
                name: Some(updated.project.name.clone()),
                roots: Some(updated.project.roots.clone()),
                metadata: Some(updated.project.metadata.clone()),
            },
        })
        .await?;
    assert_eq!(unchanged_project.project, updated.project);
    assert!(
        !server
            .pending_notification_methods()
            .iter()
            .any(|method| method == "project/changed")
    );

    let listed_projects: ProjectListResponse = server
        .request(|request_id| ClientRequest::ProjectList {
            request_id,
            params: ProjectListParams {
                cursor: None,
                limit: Some(10),
            },
        })
        .await?;
    assert_eq!(listed_projects.data, vec![updated.project.clone()]);

    let second: ProjectCreateResponse = server
        .request(|request_id| ClientRequest::ProjectCreate {
            request_id,
            params: ProjectCreateParams {
                name: "Second".to_string(),
                roots: Vec::new(),
                metadata: None,
                idempotency_key: "projects-persist-second".to_string(),
            },
        })
        .await?;
    server.clear_message_buffer();
    let _: ProjectMoveResponse = server
        .request(|request_id| ClientRequest::ProjectMove {
            request_id,
            params: ProjectMoveParams {
                project_id: second.project.id.clone(),
                before_project_id: Some(updated.project.id.clone()),
            },
        })
        .await?;
    let changed: ProjectChangedNotification = server.read_notification("project/changed").await?;
    assert_eq!(changed.project_id, second.project.id);
    let reordered: ProjectListResponse = server
        .request(|request_id| ClientRequest::ProjectList {
            request_id,
            params: ProjectListParams {
                cursor: None,
                limit: Some(10),
            },
        })
        .await?;
    assert_eq!(
        reordered
            .data
            .iter()
            .map(|project| project.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Second", "Renamed"]
    );
    let moved_second = reordered.data[0].clone();

    server.clear_message_buffer();
    let deleted_project_id = created.project.id.clone();
    let _: ProjectDeleteResponse = server
        .request(|request_id| ClientRequest::ProjectDelete {
            request_id,
            params: ProjectDeleteParams {
                project_id: deleted_project_id.clone(),
            },
        })
        .await?;
    let deleted_notification: ProjectChangedNotification =
        server.read_notification("project/changed").await?;
    assert_eq!(deleted_notification.project_id, deleted_project_id);
    assert_eq!(deleted_notification.change_type, ProjectChangeType::Deleted);
    let unassigned_notification: ThreadProjectUpdatedNotification =
        server.read_notification("thread/project/updated").await?;
    assert_eq!(
        unassigned_notification,
        ThreadProjectUpdatedNotification {
            thread_id: started.thread.id.clone(),
            project_id: None,
        }
    );
    let read_id = server
        .send_project_read_request(ProjectReadParams {
            project_id: deleted_project_id,
        })
        .await?;
    let read_error = server
        .read_stream_until_error_message(RequestId::Integer(read_id))
        .await?;
    assert_eq!(read_error.error.code, -32602);
    let remaining_projects: ProjectListResponse = server
        .request(|request_id| ClientRequest::ProjectList {
            request_id,
            params: ProjectListParams {
                cursor: None,
                limit: Some(10),
            },
        })
        .await?;
    assert_eq!(remaining_projects.data, vec![moved_second]);
    let unassigned_after_delete: ThreadListResponse = server
        .request(|request_id| ClientRequest::ThreadList {
            request_id,
            params: ThreadListParams {
                cursor: None,
                limit: Some(10),
                sort_key: None,
                sort_direction: None,
                model_providers: Some(Vec::new()),
                source_kinds: Some(Vec::new()),
                archived: None,
                section_id: None,
                project_id: Some(None),
                cwd: None,
                use_state_db_only: true,
                search_term: None,
                parent_thread_id: None,
                ancestor_thread_id: None,
            },
        })
        .await?;
    assert_eq!(unassigned_after_delete.data.len(), 1);
    assert_eq!(unassigned_after_delete.data[0].id, started.thread.id);
    assert!(codex_home.path().exists());
    Ok(())
}

#[tokio::test]
async fn deleted_project_is_dropped_before_first_durable_thread_persistence() -> Result<()> {
    let responses = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses.uri())
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let project: ProjectCreateResponse = server
        .request(|request_id| ClientRequest::ProjectCreate {
            request_id,
            params: ProjectCreateParams {
                name: "Pending".to_string(),
                roots: Vec::new(),
                metadata: None,
                idempotency_key: "deleted-before-persistence".to_string(),
            },
        })
        .await?;
    server.clear_message_buffer();
    let started = server
        .start_thread(ThreadStartParams {
            project_id: Some(project.project.id.clone()),
            ..Default::default()
        })
        .await?;
    let _: serde_json::Value = server.read_notification("thread/started").await?;

    server.clear_message_buffer();
    let _: ProjectDeleteResponse = server
        .request(|request_id| ClientRequest::ProjectDelete {
            request_id,
            params: ProjectDeleteParams {
                project_id: project.project.id.clone(),
            },
        })
        .await?;
    let _: ProjectChangedNotification = server.read_notification("project/changed").await?;
    let read_after_delete: ThreadReadResponse = server
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: started.thread.id.clone(),
                include_turns: false,
            },
        })
        .await?;
    assert_eq!(read_after_delete.thread.project_id, None);
    server
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: started.thread.id.clone(),
            input: vec![UserInput::Text {
                text: "persist after project deletion".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;

    let unassigned: ThreadListResponse = server
        .request(|request_id| ClientRequest::ThreadList {
            request_id,
            params: ThreadListParams {
                cursor: None,
                limit: Some(10),
                sort_key: None,
                sort_direction: None,
                model_providers: Some(Vec::new()),
                source_kinds: Some(Vec::new()),
                archived: None,
                section_id: None,
                project_id: Some(None),
                cwd: None,
                use_state_db_only: true,
                search_term: None,
                parent_thread_id: None,
                ancestor_thread_id: None,
            },
        })
        .await?;
    assert_eq!(unassigned.data.len(), 1);
    assert_eq!(unassigned.data[0].id, started.thread.id);
    Ok(())
}

#[tokio::test]
async fn project_import_is_atomic_and_notifies_after_commit_in_order() -> Result<()> {
    let responses = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses.uri())
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let started = server.start_thread(ThreadStartParams::default()).await?;
    let _: serde_json::Value = server.read_notification("thread/started").await?;
    server
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: started.thread.id.clone(),
            input: vec![UserInput::Text {
                text: "persist this thread".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    server.clear_message_buffer();
    let import_id = server
        .send_project_import_request(ProjectImportParams {
            name: "Imported".to_string(),
            roots: Vec::new(),
            metadata: None,
            threads: Some(vec![started.thread.id.clone()]),
            idempotency_key: "desktop:legacy-project".to_string(),
        })
        .await?;

    let JSONRPCMessage::Notification(project_changed) = server.read_next_message().await? else {
        panic!("project/import must emit project/changed before its response");
    };
    assert_eq!(project_changed.method, "project/changed");
    let project_changed: ProjectChangedNotification =
        serde_json::from_value(project_changed.params.expect("notification params"))?;
    let JSONRPCMessage::Notification(thread_updated) = server.read_next_message().await? else {
        panic!("project/import must emit thread/project/updated after project/changed");
    };
    assert_eq!(thread_updated.method, "thread/project/updated");
    let thread_updated: ThreadProjectUpdatedNotification =
        serde_json::from_value(thread_updated.params.expect("notification params"))?;
    assert_eq!(thread_updated.thread_id, started.thread.id);
    assert_eq!(
        thread_updated.project_id,
        Some(project_changed.project_id.clone())
    );
    let JSONRPCMessage::Response(response) = server.read_next_message().await? else {
        panic!("project/import must respond after commit notifications");
    };
    assert_eq!(response.id, RequestId::Integer(import_id));
    let imported: ProjectImportResponse = serde_json::from_value(response.result)?;

    server.clear_message_buffer();
    let replayed: ProjectImportResponse = server
        .request(|request_id| ClientRequest::ProjectImport {
            request_id,
            params: ProjectImportParams {
                name: "Changed payload".to_string(),
                roots: Vec::new(),
                metadata: None,
                threads: Some(vec![started.thread.id.clone()]),
                idempotency_key: "desktop:legacy-project".to_string(),
            },
        })
        .await?;
    assert_eq!(replayed.project, imported.project);
    assert!(
        !server
            .pending_notification_methods()
            .iter()
            .any(|method| method == "project/changed" || method == "thread/project/updated")
    );

    let listed: ProjectListResponse = server
        .request(|request_id| ClientRequest::ProjectList {
            request_id,
            params: ProjectListParams {
                cursor: None,
                limit: Some(10),
            },
        })
        .await?;
    assert_eq!(listed.data.len(), 1);

    let duplicate_id = server
        .send_project_import_request(ProjectImportParams {
            name: "Duplicate".to_string(),
            roots: Vec::new(),
            metadata: None,
            threads: Some(vec![started.thread.id.clone(), started.thread.id.clone()]),
            idempotency_key: "duplicate-thread-import".to_string(),
        })
        .await?;
    let duplicate = server
        .read_stream_until_error_message(RequestId::Integer(duplicate_id))
        .await?;
    assert_eq!(duplicate.error.code, -32602);
    let listed: ProjectListResponse = server
        .request(|request_id| ClientRequest::ProjectList {
            request_id,
            params: ProjectListParams {
                cursor: None,
                limit: Some(10),
            },
        })
        .await?;
    assert_eq!(listed.data.len(), 1);

    let ephemeral = server
        .start_thread(ThreadStartParams {
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;
    let ephemeral_id = server
        .send_project_import_request(ProjectImportParams {
            name: "Ephemeral".to_string(),
            roots: Vec::new(),
            metadata: None,
            threads: Some(vec![ephemeral.thread.id]),
            idempotency_key: "ephemeral-thread-import".to_string(),
        })
        .await?;
    let ephemeral_error = server
        .read_stream_until_error_message(RequestId::Integer(ephemeral_id))
        .await?;
    assert_eq!(ephemeral_error.error.code, -32602);
    let listed: ProjectListResponse = server
        .request(|request_id| ClientRequest::ProjectList {
            request_id,
            params: ProjectListParams {
                cursor: None,
                limit: Some(10),
            },
        })
        .await?;
    assert_eq!(listed.data.len(), 1);
    Ok(())
}

#[tokio::test]
async fn projects_validate_filters_cursors_and_sqlite_less_assignment() -> Result<()> {
    let responses = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses.uri())
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    for project_id in [String::new(), Uuid::now_v7().to_string()] {
        let request_id = server
            .send_thread_list_request(ThreadListParams {
                cursor: None,
                limit: Some(10),
                sort_key: None,
                sort_direction: None,
                model_providers: Some(Vec::new()),
                source_kinds: Some(Vec::new()),
                archived: None,
                section_id: None,
                project_id: Some(Some(project_id)),
                cwd: None,
                use_state_db_only: true,
                search_term: None,
                parent_thread_id: None,
                ancestor_thread_id: None,
            })
            .await?;
        let error = server
            .read_stream_until_error_message(RequestId::Integer(request_id))
            .await?;
        assert_eq!(error.error.code, -32602);
    }
    let cursor_id = server
        .send_project_list_request(ProjectListParams {
            cursor: Some("123|not-a-uuid|extra".to_string()),
            limit: Some(10),
        })
        .await?;
    let cursor_error = server
        .read_stream_until_error_message(RequestId::Integer(cursor_id))
        .await?;
    assert_eq!(cursor_error.error.code, -32602);

    let started = server.start_thread(ThreadStartParams::default()).await?;
    let metadata_id = server
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: started.thread.id.clone(),
            project_id: Some(Uuid::now_v7().to_string()),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: Some(Some("abc123".to_string())),
                branch: None,
                origin_url: None,
            }),
        })
        .await?;
    let metadata_error = server
        .read_stream_until_error_message(RequestId::Integer(metadata_id))
        .await?;
    assert_eq!(metadata_error.error.code, -32600);
    let unchanged = server
        .request::<ThreadReadResponse>(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: started.thread.id.clone(),
                include_turns: false,
            },
        })
        .await?;
    assert_eq!(unchanged.thread.git_info, None);

    let unsupported_projects_home = TempDir::new()?;
    MockResponsesConfig::new(&responses.uri()).write(unsupported_projects_home.path())?;
    let store_id = Uuid::now_v7();
    std::fs::write(
        unsupported_projects_home.path().join("config.toml"),
        format!("experimental_thread_store = {{ type = \"in_memory\", id = \"{store_id}\" }}"),
    )?;
    let mut unsupported_projects = TestAppServer::builder()
        .with_codex_home(unsupported_projects_home.path())
        .build_initialized()
        .await?;
    let start_id = unsupported_projects
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            project_id: Some(Uuid::now_v7().to_string()),
            ..Default::default()
        })
        .await?;
    let error: JSONRPCError = unsupported_projects
        .read_stream_until_error_message(RequestId::Integer(start_id))
        .await?;
    assert_eq!(error.error.code, -32601);
    let list_id = unsupported_projects
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: Some(10),
            sort_key: None,
            sort_direction: None,
            model_providers: Some(Vec::new()),
            source_kinds: Some(Vec::new()),
            archived: None,
            section_id: None,
            project_id: Some(None),
            cwd: None,
            use_state_db_only: true,
            search_term: None,
            parent_thread_id: None,
            ancestor_thread_id: None,
        })
        .await?;
    let list_error = unsupported_projects
        .read_stream_until_error_message(RequestId::Integer(list_id))
        .await?;
    assert_eq!(list_error.error.code, -32601);
    let unassigned = unsupported_projects
        .start_thread(ThreadStartParams::default())
        .await?;
    let _: serde_json::Value = unsupported_projects
        .read_notification("thread/started")
        .await?;
    assert_eq!(unassigned.thread.project_id, None);
    Ok(())
}

#[tokio::test]
async fn assigned_forks_inherit_projects_for_persistent_and_ephemeral_children() -> Result<()> {
    let responses = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses.uri())
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let project: ProjectCreateResponse = server
        .request(|request_id| ClientRequest::ProjectCreate {
            request_id,
            params: ProjectCreateParams {
                name: "Work".to_string(),
                roots: Vec::new(),
                metadata: None,
                idempotency_key: "fork-inheritance".to_string(),
            },
        })
        .await?;
    let started = server
        .start_thread(ThreadStartParams {
            project_id: Some(project.project.id.clone()),
            ..Default::default()
        })
        .await?;
    let _: serde_json::Value = server.read_notification("thread/started").await?;
    server
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: started.thread.id.clone(),
            input: vec![UserInput::Text {
                text: "persist this thread".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;

    server.clear_message_buffer();
    let fork_id = server
        .send_thread_fork_request(ThreadForkParams {
            thread_id: started.thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let JSONRPCMessage::Response(response) = server.read_next_message().await? else {
        panic!("thread/fork must respond before lifecycle notifications");
    };
    assert_eq!(response.id, RequestId::Integer(fork_id));
    let forked: ThreadForkResponse = serde_json::from_value(response.result)?;
    let _: serde_json::Value = server.read_notification("thread/started").await?;
    assert_eq!(forked.thread.project_id, Some(project.project.id.clone()));

    server.clear_message_buffer();
    let ephemeral_id = server
        .send_thread_fork_request(ThreadForkParams {
            thread_id: started.thread.id,
            ephemeral: true,
            ..Default::default()
        })
        .await?;
    let JSONRPCMessage::Response(response) = server.read_next_message().await? else {
        panic!("ephemeral thread/fork must respond before lifecycle notifications");
    };
    assert_eq!(response.id, RequestId::Integer(ephemeral_id));
    let ephemeral_fork: ThreadForkResponse = serde_json::from_value(response.result)?;
    let _: serde_json::Value = server.read_notification("thread/started").await?;
    assert_eq!(ephemeral_fork.thread.project_id, Some(project.project.id));
    assert!(ephemeral_fork.thread.ephemeral);
    assert!(ephemeral_fork.thread.path.is_none());
    Ok(())
}
