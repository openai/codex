use std::collections::BTreeMap;
use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_shell_command_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::AdditionalContextEntry;
use codex_app_server_protocol::AdditionalContextKind;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetParams;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetResponse;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::QueuedItemProvenance;
use codex_app_server_protocol::QueuedItemStatus;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadQueueAddParams;
use codex_app_server_protocol::ThreadQueueAddResponse;
use codex_app_server_protocol::ThreadQueueChangedNotification;
use codex_app_server_protocol::ThreadQueueDeleteParams;
use codex_app_server_protocol::ThreadQueueDeleteResponse;
use codex_app_server_protocol::ThreadQueueListParams;
use codex_app_server_protocol::ThreadQueueListResponse;
use codex_app_server_protocol::ThreadQueueReorderParams;
use codex_app_server_protocol::ThreadQueueReorderResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnSubmissionParams;
use codex_app_server_protocol::UserInput;
use codex_core::config::set_project_trust_level;
use codex_protocol::config_types::TrustLevel;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn queue_api_round_trips_payload_and_mutations_while_turn_is_active() -> Result<()> {
    let responses = vec![
        create_shell_command_sse_response(
            vec!["printf".to_string(), "active".to_string()],
            /*workdir*/ None,
            Some(5_000),
            "active-call",
        )?,
        create_final_assistant_message_sse_response("done")?,
        create_final_assistant_message_sse_response("queued done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    let codex_home = TempDir::new()?;
    write_config(
        codex_home.path(),
        &server.uri(),
        /*queue_enabled*/ Some(true),
    )?;
    let mut app = TestAppServer::new(codex_home.path()).await?;

    initialize_experimental(&mut app).await?;
    let thread_id = start_thread(&mut app).await?;
    let approval_request_id = start_blocked_turn(&mut app, &thread_id).await?;

    let submission = TurnSubmissionParams {
        input: vec![UserInput::Text {
            text: "queued from webhook".to_string(),
            text_elements: Vec::new(),
        }],
        responsesapi_client_metadata: Some(HashMap::from([(
            "request_id".to_string(),
            "request-42".to_string(),
        )])),
        additional_context: Some(HashMap::from([(
            "ticket".to_string(),
            AdditionalContextEntry {
                value: "INC-42".to_string(),
                kind: AdditionalContextKind::Application,
            },
        )])),
        output_schema: Some(json!({"type": "object"})),
    };
    let provenance = QueuedItemProvenance::ExternalEvent {
        source: "webhook".to_string(),
        metadata: HashMap::from([("delivery".to_string(), json!(42))]),
    };
    let first = queue_item(&mut app, &thread_id, submission.clone(), provenance.clone()).await?;
    assert_eq!(first.submission.input, submission.input);
    assert_eq!(
        first.submission.responsesapi_client_metadata,
        submission.responsesapi_client_metadata
    );
    assert_eq!(
        first.submission.additional_context,
        submission.additional_context
    );
    assert_eq!(first.submission.output_schema, None);
    assert_eq!(first.provenance, provenance);
    assert_eq!(first.status, QueuedItemStatus::Pending);

    let changed: ThreadQueueChangedNotification = serde_json::from_value(
        timeout(
            READ_TIMEOUT,
            app.read_stream_until_notification_message("thread/queue/changed"),
        )
        .await??
        .params
        .expect("queue notification should include params"),
    )?;
    assert_eq!(changed.thread_id, thread_id);

    let second = queue_item(
        &mut app,
        &thread_id,
        TurnSubmissionParams {
            input: vec![UserInput::Text {
                text: "second".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        },
        QueuedItemProvenance::User,
    )
    .await?;
    let page = list_queue(&mut app, &thread_id, /*cursor*/ None, Some(1)).await?;
    assert_eq!(page.data, vec![first.clone()]);
    assert_eq!(page.next_cursor.as_deref(), Some("1"));
    let second_page = list_queue(&mut app, &thread_id, page.next_cursor, Some(1)).await?;
    assert_eq!(second_page.data, vec![second.clone()]);

    let invalid_reorder_id = app
        .send_raw_request(
            "thread/queue/reorder",
            Some(serde_json::to_value(ThreadQueueReorderParams {
                thread_id: thread_id.clone(),
                queued_item_ids: vec![first.id.clone()],
            })?),
        )
        .await?;
    let invalid_reorder: JSONRPCError = timeout(
        READ_TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(invalid_reorder_id)),
    )
    .await??;
    assert_eq!(invalid_reorder.error.code, -32600);

    let reorder_id = app
        .send_raw_request(
            "thread/queue/reorder",
            Some(serde_json::to_value(ThreadQueueReorderParams {
                thread_id: thread_id.clone(),
                queued_item_ids: vec![second.id.clone(), first.id.clone()],
            })?),
        )
        .await?;
    let _: ThreadQueueReorderResponse = read_response(&mut app, reorder_id).await?;

    delete_item(&mut app, &thread_id, &second.id).await?;

    app.send_response(
        approval_request_id,
        serde_json::to_value(CommandExecutionRequestApprovalResponse {
            decision: CommandExecutionApprovalDecision::Decline,
        })?,
    )
    .await?;
    timeout(
        READ_TIMEOUT,
        app.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    timeout(
        READ_TIMEOUT,
        app.read_stream_until_notification_message("turn/started"),
    )
    .await??;
    timeout(
        READ_TIMEOUT,
        app.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    assert!(
        list_queue(
            &mut app, &thread_id, /*cursor*/ None, /*limit*/ None
        )
        .await?
        .data
        .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn queue_api_rejects_missing_threads() -> Result<()> {
    let server = create_mock_responses_server_sequence(Vec::new()).await;
    let codex_home = TempDir::new()?;
    write_config(
        codex_home.path(),
        &server.uri(),
        /*queue_enabled*/ Some(true),
    )?;
    let mut app = TestAppServer::new(codex_home.path()).await?;
    initialize_experimental(&mut app).await?;

    let request_id = app
        .send_raw_request(
            "thread/queue/list",
            Some(serde_json::to_value(ThreadQueueListParams {
                thread_id: codex_protocol::ThreadId::default().to_string(),
                cursor: None,
                limit: None,
            })?),
        )
        .await?;
    let error: JSONRPCError = timeout(
        READ_TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert!(error.error.message.starts_with("thread not found:"));
    Ok(())
}

#[tokio::test]
async fn queue_api_rejects_archived_threads() -> Result<()> {
    let server =
        create_mock_responses_server_sequence(vec![create_final_assistant_message_sse_response(
            "done",
        )?])
        .await;
    let codex_home = TempDir::new()?;
    write_config(
        codex_home.path(),
        &server.uri(),
        /*queue_enabled*/ Some(true),
    )?;
    let mut app = TestAppServer::new(codex_home.path()).await?;
    initialize_experimental(&mut app).await?;
    let thread_id = start_thread(&mut app).await?;

    let turn_id = app
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "materialize".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: codex_app_server_protocol::TurnStartResponse = read_response(&mut app, turn_id).await?;
    timeout(
        READ_TIMEOUT,
        app.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let archive_id = app
        .send_thread_archive_request(ThreadArchiveParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let _: ThreadArchiveResponse = read_response(&mut app, archive_id).await?;
    timeout(
        READ_TIMEOUT,
        app.read_stream_until_notification_message("thread/archived"),
    )
    .await??;

    let request_id = app
        .send_raw_request(
            "thread/queue/add",
            Some(serde_json::to_value(ThreadQueueAddParams {
                thread_id: thread_id.clone(),
                submission: TurnSubmissionParams {
                    input: vec![UserInput::Text {
                        text: "queued".to_string(),
                        text_elements: Vec::new(),
                    }],
                    ..Default::default()
                },
                provenance: QueuedItemProvenance::User,
            })?),
        )
        .await?;
    let error: JSONRPCError = timeout(
        READ_TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert!(error.error.message.contains("is archived"));
    Ok(())
}

#[tokio::test]
async fn disabled_queue_is_not_advertised_or_callable() -> Result<()> {
    let server = create_mock_responses_server_sequence(Vec::new()).await;
    let codex_home = TempDir::new()?;
    write_config(
        codex_home.path(),
        &server.uri(),
        /*queue_enabled*/ Some(false),
    )?;
    let mut app = TestAppServer::new(codex_home.path()).await?;
    initialize_experimental(&mut app).await?;
    let thread_id = start_thread(&mut app).await?;

    let request_id = app
        .send_raw_request(
            "thread/queue/list",
            Some(serde_json::to_value(ThreadQueueListParams {
                thread_id,
                cursor: None,
                limit: None,
            })?),
        )
        .await?;
    let error: JSONRPCError = timeout(
        READ_TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.message, "user message queue is unavailable");
    Ok(())
}

#[tokio::test]
async fn queue_can_be_enabled_at_runtime_without_restarting_app_server() -> Result<()> {
    let server = create_mock_responses_server_sequence(Vec::new()).await;
    let codex_home = TempDir::new()?;
    write_config(
        codex_home.path(),
        &server.uri(),
        /*queue_enabled*/ None,
    )?;
    let mut app = TestAppServer::new(codex_home.path()).await?;
    initialize_experimental(&mut app).await?;
    let thread_id = start_thread(&mut app).await?;
    let enablement = BTreeMap::from([("user_message_queue".to_string(), true)]);

    let request_id = app
        .send_experimental_feature_enablement_set_request(ExperimentalFeatureEnablementSetParams {
            enablement: enablement.clone(),
        })
        .await?;
    let response: ExperimentalFeatureEnablementSetResponse =
        read_response(&mut app, request_id).await?;
    assert_eq!(
        response,
        ExperimentalFeatureEnablementSetResponse { enablement }
    );

    assert_eq!(
        list_queue(
            &mut app, &thread_id, /*cursor*/ None, /*limit*/ None,
        )
        .await?,
        ThreadQueueListResponse {
            data: Vec::new(),
            next_cursor: None,
        }
    );
    Ok(())
}

#[tokio::test]
async fn queue_uses_the_loaded_threads_project_feature_config() -> Result<()> {
    let server =
        create_mock_responses_server_sequence(vec![create_final_assistant_message_sse_response(
            "materialized",
        )?])
        .await;
    let codex_home = TempDir::new()?;
    let workspace = TempDir::new()?;
    write_config(
        codex_home.path(),
        &server.uri(),
        /*queue_enabled*/ None,
    )?;
    let project_config_dir = workspace.path().join(".codex");
    std::fs::create_dir_all(&project_config_dir)?;
    std::fs::write(
        project_config_dir.join("config.toml"),
        "[features]\nuser_message_queue = true\n",
    )?;
    set_project_trust_level(codex_home.path(), workspace.path(), TrustLevel::Trusted)?;

    let mut app = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    initialize_experimental(&mut app).await?;
    let request_id = app
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(workspace.path().display().to_string()),
            ..Default::default()
        })
        .await?;
    let response: ThreadStartResponse = read_response(&mut app, request_id).await?;

    assert_eq!(
        list_queue(
            &mut app,
            &response.thread.id,
            /*cursor*/ None,
            /*limit*/ None,
        )
        .await?,
        ThreadQueueListResponse {
            data: Vec::new(),
            next_cursor: None,
        }
    );
    let thread_id = response.thread.id;
    let turn_id = app
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "materialize".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: codex_app_server_protocol::TurnStartResponse = read_response(&mut app, turn_id).await?;
    timeout(
        READ_TIMEOUT,
        app.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    drop(app);

    let mut app = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    initialize_experimental(&mut app).await?;
    assert_eq!(
        list_queue(
            &mut app, &thread_id, /*cursor*/ None, /*limit*/ None,
        )
        .await?,
        ThreadQueueListResponse {
            data: Vec::new(),
            next_cursor: None,
        }
    );
    Ok(())
}

async fn initialize_experimental(app: &mut TestAppServer) -> Result<InitializeResponse> {
    let message = timeout(
        READ_TIMEOUT,
        app.initialize_with_capabilities(
            ClientInfo {
                name: "queue-api-test".to_string(),
                title: None,
                version: "0.0.0".to_string(),
            },
            Some(InitializeCapabilities {
                experimental_api: true,
                request_attestation: false,
                opt_out_notification_methods: None,
            }),
        ),
    )
    .await??;
    let JSONRPCMessage::Response(response) = message else {
        anyhow::bail!("expected initialize response")
    };
    to_response(response)
}

async fn start_thread(app: &mut TestAppServer) -> Result<String> {
    let request_id = app
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let response: ThreadStartResponse = read_response(app, request_id).await?;
    Ok(response.thread.id)
}

async fn start_blocked_turn(
    app: &mut TestAppServer,
    thread_id: &str,
) -> Result<codex_app_server_protocol::RequestId> {
    let request_id = app
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.to_string(),
            input: vec![UserInput::Text {
                text: "run a command".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: codex_app_server_protocol::TurnStartResponse = read_response(app, request_id).await?;
    let request = timeout(READ_TIMEOUT, app.read_stream_until_request_message()).await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, .. } = request else {
        anyhow::bail!("expected command approval request")
    };
    Ok(request_id)
}

async fn queue_item(
    app: &mut TestAppServer,
    thread_id: &str,
    submission: TurnSubmissionParams,
    provenance: QueuedItemProvenance,
) -> Result<codex_app_server_protocol::QueuedItem> {
    let request_id = app
        .send_raw_request(
            "thread/queue/add",
            Some(serde_json::to_value(ThreadQueueAddParams {
                thread_id: thread_id.to_string(),
                submission,
                provenance,
            })?),
        )
        .await?;
    let response: ThreadQueueAddResponse = read_response(app, request_id).await?;
    Ok(response.queued_item)
}

async fn list_queue(
    app: &mut TestAppServer,
    thread_id: &str,
    cursor: Option<String>,
    limit: Option<u32>,
) -> Result<ThreadQueueListResponse> {
    let request_id = app
        .send_raw_request(
            "thread/queue/list",
            Some(serde_json::to_value(ThreadQueueListParams {
                thread_id: thread_id.to_string(),
                cursor,
                limit,
            })?),
        )
        .await?;
    read_response(app, request_id).await
}

async fn delete_item(app: &mut TestAppServer, thread_id: &str, item_id: &str) -> Result<()> {
    let request_id = app
        .send_raw_request(
            "thread/queue/delete",
            Some(serde_json::to_value(ThreadQueueDeleteParams {
                thread_id: thread_id.to_string(),
                queued_item_id: item_id.to_string(),
            })?),
        )
        .await?;
    let response: ThreadQueueDeleteResponse = read_response(app, request_id).await?;
    assert!(response.deleted);
    Ok(())
}

async fn read_response<T: serde::de::DeserializeOwned>(
    app: &mut TestAppServer,
    request_id: i64,
) -> Result<T> {
    let response: JSONRPCResponse = timeout(
        READ_TIMEOUT,
        app.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

fn write_config(
    codex_home: &std::path::Path,
    server_uri: &str,
    queue_enabled: Option<bool>,
) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "untrusted"
sandbox_mode = "read-only"
model_provider = "mock_provider"

[features]
{}

[model_providers.mock_provider]
name = "Mock provider"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#,
            queue_enabled
                .map(|enabled| format!("user_message_queue = {enabled}"))
                .unwrap_or_default()
        ),
    )
}
