use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use app_test_support::write_models_cache_with_models;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallParams;
use codex_app_server_protocol::DynamicToolCallResponse;
use codex_app_server_protocol::DynamicToolCallStatus;
use codex_app_server_protocol::DynamicToolFunctionSpec;
use codex_app_server_protocol::DynamicToolNamespaceSpec;
use codex_app_server_protocol::DynamicToolNamespaceTool;
use codex_app_server_protocol::DynamicToolSpec;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::openai_models::InputModality;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::MockServer;

const TINY_PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
const INLINE_AUDIO_DATA_URL: &str = "data:audio/wav;base64,YXVkaW8=";
const INVALID_AUDIO_URL_ERROR: &str = "audio URLs must use an inline data URL";
const REMOTE_IMAGE_URL_ERROR: &str =
    "remote image URLs are not supported; use an inline data URL instead";

// macOS and Windows Bazel CI can spend tens of seconds starting app-server
// subprocesses or processing test RPCs under load.
#[cfg(any(target_os = "macos", windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(not(any(target_os = "macos", windows)))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn thread_start_normalizes_legacy_dynamic_tools_into_model_request() -> Result<()> {
    let responses = vec![create_final_assistant_message_sse_response("Done")?];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let visible_schema = json!({
        "type": "object",
        "properties": {
            "ticket_id": { "type": "string" }
        },
        "required": ["ticket_id"],
        "additionalProperties": false,
    });
    let thread_req = mcp
        .send_raw_request(
            "thread/start",
            Some(json!({
                "dynamicTools": [
                    {
                        "name": "lookup_ticket",
                        "description": "Look up a ticket",
                        "inputSchema": visible_schema,
                    },
                    {
                        "namespace": "legacy_app",
                        "name": "lookup_status",
                        "description": "Look up a ticket status",
                        "inputSchema": visible_schema,
                        "exposeToContext": true
                    },
                    {
                        "namespace": "legacy_app",
                        "name": "update_ticket",
                        "description": "Update a ticket",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false
                        },
                        "exposeToContext": false
                    }
                ]
            })),
        )
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Look up the ticket".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _turn: TurnStartResponse = to_response::<TurnStartResponse>(turn_resp)?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let bodies = responses_bodies(&server).await?;
    let function =
        find_tool(&bodies[0], "lookup_ticket").context("expected normalized legacy function")?;
    assert_eq!(
        function,
        &json!({
            "type": "function",
            "name": "lookup_ticket",
            "description": "Look up a ticket",
            "strict": false,
            "parameters": visible_schema,
        })
    );
    let namespace =
        find_tool(&bodies[0], "legacy_app").context("expected normalized legacy namespace")?;
    assert_eq!(
        namespace,
        &json!({
            "type": "namespace",
            "name": "legacy_app",
            "description": "Tools in the legacy_app namespace.",
            "tools": [{
                "type": "function",
                "name": "lookup_status",
                "description": "Look up a ticket status",
                "strict": false,
                "parameters": visible_schema,
            }],
        })
    );

    Ok(())
}

#[tokio::test]
async fn thread_start_rejects_hidden_dynamic_tools_without_namespace() -> Result<()> {
    let server = MockServer::start().await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let dynamic_tool = DynamicToolSpec::Function(DynamicToolFunctionSpec {
        name: "hidden_tool".to_string(),
        description: "Hidden dynamic tool".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false,
        }),
        defer_loading: true,
    });

    let thread_req = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            dynamic_tools: Some(vec![dynamic_tool]),
            ..Default::default()
        })
        .await?;
    let error = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(thread_req)),
    )
    .await??;
    assert_eq!(error.error.code, -32600);
    assert!(error.error.message.contains("hidden_tool"));
    assert!(error.error.message.contains("namespace"));

    Ok(())
}

#[tokio::test]
async fn thread_start_rejects_invalid_dynamic_tool_inputs() -> Result<()> {
    let server = MockServer::start().await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    for (dynamic_tools, expected_error) in [
        (
            json!([
                {
                    "type": "function",
                    "name": "canonical_tool",
                    "description": "Canonical tool",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "namespace": "legacy_app",
                    "name": "legacy_tool",
                    "description": "Legacy tool",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                }
            ]),
            "either canonical or legacy format",
        ),
        (
            json!([{
                "type": "namespace",
                "name": "canonical_namespace",
                "description": "Canonical namespace",
                "tools": [{
                    "type": "function",
                    "name": "legacy_visibility_tool",
                    "description": "Uses a legacy visibility field",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    },
                    "exposeToContext": false
                }]
            }]),
            "either canonical or legacy format",
        ),
        (
            json!([{
                "type": "namespace",
                "name": "empty_namespace",
                "description": "Contains no tools",
                "tools": []
            }]),
            "must contain at least one tool",
        ),
        (
            json!([
                {
                    "type": "namespace",
                    "name": "duplicate_namespace",
                    "description": "First namespace",
                    "tools": [{
                        "type": "function",
                        "name": "first_tool",
                        "description": "First tool",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    }]
                },
                {
                    "type": "namespace",
                    "name": "duplicate_namespace",
                    "description": "Second namespace",
                    "tools": [{
                        "type": "function",
                        "name": "second_tool",
                        "description": "Second tool",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    }]
                }
            ]),
            "duplicate dynamic tool namespace",
        ),
    ] {
        let thread_req = mcp
            .send_raw_request(
                "thread/start",
                Some(json!({ "dynamicTools": dynamic_tools })),
            )
            .await?;
        let error = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_error_message(RequestId::Integer(thread_req)),
        )
        .await??;
        assert_eq!(error.error.code, -32600);
        assert!(
            error.error.message.contains(expected_error),
            "unexpected error: {}",
            error.error.message
        );
    }

    Ok(())
}

/// Exercises the full dynamic tool call path (server request, client response, model output).
#[tokio::test]
async fn dynamic_tool_call_round_trip_sends_text_content_items_to_model() -> Result<()> {
    let call_id = "dyn-call-1";
    let tool_namespace = "codex_app";
    let tool_name = "demo_tool";
    let tool_args = json!({ "city": "Paris" });
    let tool_call_arguments = serde_json::to_string(&tool_args)?;

    // First response triggers a dynamic tool call, second closes the turn.
    let responses = vec![
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "call_id": call_id,
                    "namespace": tool_namespace,
                    "name": tool_name,
                    "arguments": tool_call_arguments,
                }
            }),
            responses::ev_completed("resp-1"),
        ]),
        create_final_assistant_message_sse_response("Done")?,
    ];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let input_schema = json!({
        "type": "object",
        "properties": {
            "city": { "type": "string" }
        },
        "required": ["city"],
        "additionalProperties": false,
    });
    let status_schema = json!({
        "type": "object",
        "properties": {
            "ticket_id": { "type": "string" }
        },
        "required": ["ticket_id"],
        "additionalProperties": false,
    });
    let namespace_description = "Demo namespace tools";
    let dynamic_tool = DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
        name: tool_namespace.to_string(),
        description: namespace_description.to_string(),
        tools: vec![
            DynamicToolNamespaceTool::Function(DynamicToolFunctionSpec {
                name: tool_name.to_string(),
                description: "Demo dynamic tool".to_string(),
                input_schema: input_schema.clone(),
                defer_loading: false,
            }),
            DynamicToolNamespaceTool::Function(DynamicToolFunctionSpec {
                name: "lookup_status".to_string(),
                description: "Look up ticket status".to_string(),
                input_schema: status_schema.clone(),
                defer_loading: false,
            }),
        ],
    });

    let thread_req = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            dynamic_tools: Some(vec![dynamic_tool]),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;
    let thread_id = thread.id.clone();

    // Start a turn so the tool call is emitted.
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Run the tool".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;
    let turn_id = turn.id.clone();

    let started = wait_for_dynamic_tool_started(&mut mcp, call_id).await?;
    assert_eq!(started.thread_id, thread_id);
    assert_eq!(started.turn_id, turn_id.clone());
    let ThreadItem::DynamicToolCall {
        id,
        namespace,
        tool,
        arguments,
        status,
        content_items,
        success,
        duration_ms,
    } = started.item
    else {
        panic!("expected dynamic tool call item");
    };
    assert_eq!(id, call_id);
    assert_eq!(namespace.as_deref(), Some(tool_namespace));
    assert_eq!(tool, tool_name);
    assert_eq!(arguments, tool_args);
    assert_eq!(status, DynamicToolCallStatus::InProgress);
    assert_eq!(content_items, None);
    assert_eq!(success, None);
    assert_eq!(duration_ms, None);

    // Read the tool call request from the app server.
    let request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let (request_id, params) = match request {
        ServerRequest::DynamicToolCall { request_id, params } => (request_id, params),
        other => panic!("expected DynamicToolCall request, got {other:?}"),
    };

    let expected = DynamicToolCallParams {
        thread_id: thread_id.clone(),
        turn_id: turn_id.clone(),
        call_id: call_id.to_string(),
        namespace: Some(tool_namespace.to_string()),
        tool: tool_name.to_string(),
        arguments: tool_args.clone(),
    };
    assert_eq!(params, expected);

    // Respond to the tool call so the model receives a function_call_output.
    let response = DynamicToolCallResponse {
        content_items: vec![DynamicToolCallOutputContentItem::InputText {
            text: "dynamic-ok".to_string(),
        }],
        success: true,
    };
    mcp.send_response(request_id, serde_json::to_value(response)?)
        .await?;

    let completed = wait_for_dynamic_tool_completed(&mut mcp, call_id).await?;
    assert_eq!(completed.thread_id, thread_id);
    assert_eq!(completed.turn_id, turn_id);
    let ThreadItem::DynamicToolCall {
        id,
        namespace,
        tool,
        arguments,
        status,
        content_items,
        success,
        duration_ms,
    } = completed.item
    else {
        panic!("expected dynamic tool call item");
    };
    assert_eq!(id, call_id);
    assert_eq!(namespace.as_deref(), Some(tool_namespace));
    assert_eq!(tool, tool_name);
    assert_eq!(arguments, tool_args);
    assert_eq!(status, DynamicToolCallStatus::Completed);
    assert_eq!(
        content_items,
        Some(vec![DynamicToolCallOutputContentItem::InputText {
            text: "dynamic-ok".to_string(),
        }])
    );
    assert_eq!(success, Some(true));
    assert!(duration_ms.is_some());

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let bodies = responses_bodies(&server).await?;
    let namespace = find_tool(&bodies[0], tool_namespace)
        .context("expected explicit dynamic tool namespace in first request")?;
    assert_eq!(
        namespace,
        &json!({
            "type": "namespace",
            "name": tool_namespace,
            "description": namespace_description,
            "tools": [
                {
                    "type": "function",
                    "name": tool_name,
                    "description": "Demo dynamic tool",
                    "strict": false,
                    "parameters": input_schema,
                },
                {
                    "type": "function",
                    "name": "lookup_status",
                    "description": "Look up ticket status",
                    "strict": false,
                    "parameters": status_schema,
                },
            ],
        })
    );
    let payload = bodies
        .iter()
        .find_map(|body| function_call_output_payload(body, call_id))
        .context("expected function_call_output in follow-up request")?;
    let expected_payload = FunctionCallOutputPayload::from_text("dynamic-ok".to_string());
    assert_eq!(payload, expected_payload);

    Ok(())
}

struct PendingDynamicToolCall {
    mcp: TestAppServer,
    server: MockServer,
    request_id: RequestId,
    params: DynamicToolCallParams,
}

async fn start_function_dynamic_tool_call(call_id: &str) -> Result<PendingDynamicToolCall> {
    let tool_name = "demo_tool";
    let tool_args = json!({ "city": "Paris" });
    let tool_call_arguments = serde_json::to_string(&tool_args)?;

    let response_sequence = vec![
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, tool_name, &tool_call_arguments),
            responses::ev_completed("resp-1"),
        ]),
        create_final_assistant_message_sse_response("Done")?,
    ];
    let server = create_mock_responses_server_sequence_unchecked(response_sequence).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    let config = load_default_config_for_test(&codex_home).await;
    let mut model_info =
        codex_core::test_support::construct_model_info_offline("mock-model", &config);
    model_info.input_modalities.push(InputModality::Audio);
    write_models_cache_with_models(codex_home.path(), vec![model_info])?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let dynamic_tool = DynamicToolSpec::Function(DynamicToolFunctionSpec {
        name: tool_name.to_string(),
        description: "Demo dynamic tool".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "city": { "type": "string" }
            },
            "required": ["city"],
            "additionalProperties": false,
        }),
        defer_loading: false,
    });

    let thread_req = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            dynamic_tools: Some(vec![dynamic_tool]),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;
    let thread_id = thread.id.clone();

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Run the tool".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;
    let turn_id = turn.id.clone();

    let started = wait_for_dynamic_tool_started(&mut mcp, call_id).await?;
    assert_eq!(started.thread_id, thread_id.clone());
    assert_eq!(started.turn_id, turn_id.clone());

    let request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let (request_id, actual_params) = match request {
        ServerRequest::DynamicToolCall { request_id, params } => (request_id, params),
        other => panic!("expected DynamicToolCall request, got {other:?}"),
    };

    let params = DynamicToolCallParams {
        thread_id,
        turn_id,
        call_id: call_id.to_string(),
        namespace: None,
        tool: tool_name.to_string(),
        arguments: tool_args,
    };
    assert_eq!(actual_params, params);

    Ok(PendingDynamicToolCall {
        mcp,
        server,
        request_id,
        params,
    })
}

/// Ensures dynamic tool call responses can include structured content items.
#[tokio::test]
async fn dynamic_tool_call_round_trip_handles_content_items() -> Result<()> {
    let call_id = "dyn-call-items-1";
    let PendingDynamicToolCall {
        mut mcp,
        server,
        request_id,
        params,
    } = start_function_dynamic_tool_call(call_id).await?;

    let response_content_items = vec![
        DynamicToolCallOutputContentItem::InputText {
            text: "dynamic-ok".to_string(),
        },
        DynamicToolCallOutputContentItem::InputImage {
            image_url: TINY_PNG_DATA_URL.to_string(),
        },
        DynamicToolCallOutputContentItem::InputAudio {
            audio_url: INLINE_AUDIO_DATA_URL.to_string(),
        },
    ];
    let model_content_items = vec![
        FunctionCallOutputContentItem::InputText {
            text: "dynamic-ok".to_string(),
        },
        FunctionCallOutputContentItem::InputImage {
            image_url: TINY_PNG_DATA_URL.to_string(),
            detail: Some(DEFAULT_IMAGE_DETAIL),
        },
        FunctionCallOutputContentItem::InputAudio {
            audio_url: INLINE_AUDIO_DATA_URL.to_string(),
        },
    ];
    let response = DynamicToolCallResponse {
        content_items: response_content_items,
        success: true,
    };
    mcp.send_response(request_id, serde_json::to_value(response)?)
        .await?;

    let completed = wait_for_dynamic_tool_completed(&mut mcp, call_id).await?;
    assert_eq!(completed.thread_id, params.thread_id);
    assert_eq!(completed.turn_id, params.turn_id);
    let ThreadItem::DynamicToolCall {
        status,
        content_items: completed_content_items,
        success,
        ..
    } = completed.item
    else {
        panic!("expected dynamic tool call item");
    };
    assert_eq!(status, DynamicToolCallStatus::Completed);
    assert_eq!(
        completed_content_items,
        Some(vec![
            DynamicToolCallOutputContentItem::InputText {
                text: "dynamic-ok".to_string(),
            },
            DynamicToolCallOutputContentItem::InputImage {
                image_url: TINY_PNG_DATA_URL.to_string(),
            },
            DynamicToolCallOutputContentItem::InputAudio {
                audio_url: INLINE_AUDIO_DATA_URL.to_string(),
            },
        ])
    );
    assert_eq!(success, Some(true));

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let bodies = responses_bodies(&server).await?;
    let output_value = bodies
        .iter()
        .find_map(|body| function_call_output_raw_output(body, call_id))
        .context("expected function_call_output output in follow-up request")?;
    assert_eq!(
        output_value,
        json!([
            {
                "type": "input_text",
                "text": "dynamic-ok"
            },
            {
                "type": "input_image",
                "image_url": TINY_PNG_DATA_URL,
                "detail": "high"
            },
            {
                "type": "input_audio",
                "audio_url": INLINE_AUDIO_DATA_URL
            }
        ])
    );

    let payload = bodies
        .iter()
        .find_map(|body| function_call_output_payload(body, call_id))
        .context("expected function_call_output in follow-up request")?;
    assert_eq!(
        payload.body,
        FunctionCallOutputBody::ContentItems(model_content_items.clone())
    );
    assert_eq!(payload.success, None);
    assert_eq!(
        serde_json::to_string(&payload)?,
        serde_json::to_string(&model_content_items)?
    );

    Ok(())
}

#[tokio::test]
async fn dynamic_tool_remote_image_response_becomes_model_visible_error() -> Result<()> {
    let call_id = "dyn-call-remote-image";
    let PendingDynamicToolCall {
        mut mcp,
        server,
        request_id,
        params,
    } = start_function_dynamic_tool_call(call_id).await?;

    let response = DynamicToolCallResponse {
        content_items: vec![DynamicToolCallOutputContentItem::InputImage {
            image_url: "https://example.com/tool.png".to_string(),
        }],
        success: true,
    };
    mcp.send_response(request_id, serde_json::to_value(response)?)
        .await?;

    let completed = wait_for_dynamic_tool_completed(&mut mcp, call_id).await?;
    assert_eq!(completed.thread_id, params.thread_id);
    assert_eq!(completed.turn_id, params.turn_id);
    let ThreadItem::DynamicToolCall {
        status,
        content_items,
        success,
        ..
    } = completed.item
    else {
        panic!("expected dynamic tool call item");
    };
    assert_eq!(status, DynamicToolCallStatus::Failed);
    assert_eq!(
        content_items,
        Some(vec![DynamicToolCallOutputContentItem::InputText {
            text: REMOTE_IMAGE_URL_ERROR.to_string(),
        }])
    );
    assert_eq!(success, Some(false));

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let output = responses_bodies(&server)
        .await?
        .iter()
        .find_map(|body| function_call_output_raw_output(body, call_id))
        .context("expected function_call_output output in follow-up request")?;
    assert_eq!(output, json!(REMOTE_IMAGE_URL_ERROR));

    Ok(())
}

#[tokio::test]
async fn dynamic_tool_remote_audio_response_becomes_model_visible_error() -> Result<()> {
    let call_id = "dyn-call-remote-audio";
    let PendingDynamicToolCall {
        mut mcp,
        server,
        request_id,
        params,
    } = start_function_dynamic_tool_call(call_id).await?;

    let response = DynamicToolCallResponse {
        content_items: vec![DynamicToolCallOutputContentItem::InputAudio {
            audio_url: "https://example.com/tool.wav".to_string(),
        }],
        success: true,
    };
    mcp.send_response(request_id, serde_json::to_value(response)?)
        .await?;

    let completed = wait_for_dynamic_tool_completed(&mut mcp, call_id).await?;
    assert_eq!(completed.thread_id, params.thread_id);
    assert_eq!(completed.turn_id, params.turn_id);
    let ThreadItem::DynamicToolCall {
        status,
        content_items,
        success,
        ..
    } = completed.item
    else {
        panic!("expected dynamic tool call item");
    };
    assert_eq!(status, DynamicToolCallStatus::Failed);
    assert_eq!(
        content_items,
        Some(vec![DynamicToolCallOutputContentItem::InputText {
            text: INVALID_AUDIO_URL_ERROR.to_string(),
        }])
    );
    assert_eq!(success, Some(false));

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let output = responses_bodies(&server)
        .await?
        .iter()
        .find_map(|body| function_call_output_raw_output(body, call_id))
        .context("expected function_call_output output in follow-up request")?;
    assert_eq!(output, json!(INVALID_AUDIO_URL_ERROR));

    Ok(())
}

async fn responses_bodies(server: &MockServer) -> Result<Vec<Value>> {
    let requests = server
        .received_requests()
        .await
        .context("failed to fetch received requests")?;

    requests
        .into_iter()
        .filter(|req| req.url.path().ends_with("/responses"))
        .map(|req| {
            req.body_json::<Value>()
                .context("request body should be JSON")
        })
        .collect()
}

fn find_tool<'a>(body: &'a Value, name: &str) -> Option<&'a Value> {
    body.get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools
                .iter()
                .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
        })
}

fn function_call_output_payload(body: &Value, call_id: &str) -> Option<FunctionCallOutputPayload> {
    function_call_output_raw_output(body, call_id)
        .and_then(|output| serde_json::from_value(output).ok())
}

fn function_call_output_raw_output(body: &Value, call_id: &str) -> Option<Value> {
    body.get("input")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    && item.get("call_id").and_then(Value::as_str) == Some(call_id)
            })
        })
        .and_then(|item| item.get("output"))
        .cloned()
}

async fn wait_for_dynamic_tool_started(
    mcp: &mut TestAppServer,
    call_id: &str,
) -> Result<ItemStartedNotification> {
    loop {
        let notification: JSONRPCNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("item/started"),
        )
        .await??;
        let Some(params) = notification.params else {
            continue;
        };
        let started: ItemStartedNotification = serde_json::from_value(params)?;
        if matches!(&started.item, ThreadItem::DynamicToolCall { id, .. } if id == call_id) {
            return Ok(started);
        }
    }
}

async fn wait_for_dynamic_tool_completed(
    mcp: &mut TestAppServer,
    call_id: &str,
) -> Result<ItemCompletedNotification> {
    loop {
        let notification: JSONRPCNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("item/completed"),
        )
        .await??;
        let Some(params) = notification.params else {
            continue;
        };
        let completed: ItemCompletedNotification = serde_json::from_value(params)?;
        if matches!(&completed.item, ThreadItem::DynamicToolCall { id, .. } if id == call_id) {
            return Ok(completed);
        }
    }
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
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
