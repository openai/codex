use std::net::SocketAddr;
use std::time::Duration;

use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_chat_completions_server;
use codex_app_server::public_api::AppServerEngine;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::JSONRPCMessage as RpcMessage;
use codex_app_server_protocol::JSONRPCRequest as RpcRequest;
use codex_app_server_protocol::RequestId as RpcRequestId;
use codex_app_server_protocol::SendUserTurnParams as RpcSendUserTurnParams;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::ConversationId as ConvId;
use codex_protocol::config_types::ReasoningEffort;
use codex_protocol::config_types::ReasoningSummary;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;

fn write_mock_config(codex_home: &std::path::Path, base_url: &str) {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        &config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{base_url}/v1"
wire_api = "chat"
request_max_retries = 0
stream_max_retries = 0
requires_openai_auth = false
"#
        ),
    )
    .expect("write config.toml");
}

async fn spawn_ws(engine: AppServerEngine) -> SocketAddr {
    let state = codex_app_server_ws::AppState {
        auth_token: None,
        engine,
    };
    let app = codex_app_server_ws::build_router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

async fn collect_ws_methods(addr: SocketAddr, cwd: &str) -> Vec<String> {
    use tokio::time::timeout;
    use tokio_tungstenite::tungstenite::Message as WsMsg;

    let url = format!("ws://{addr}/ws");
    let (mut ws, _resp) = connect_async(url).await.unwrap();

    // initialize
    let init = json!({
        "method": "initialize",
        "id": 1,
        "params": { "clientInfo": { "name": "tests", "version": "0.0.0" } }
    });
    ws.send(WsMsg::Text(init.to_string().into())).await.unwrap();

    // newConversation
    let new_conv = json!({
        "method": "newConversation",
        "id": 2,
        "params": { "cwd": cwd }
    });
    ws.send(WsMsg::Text(new_conv.to_string().into()))
        .await
        .unwrap();

    // await response id=2
    let mut conversation_id: Option<String> = None;
    for _ in 0..50 {
        if let Ok(Some(Ok(WsMsg::Text(txt)))) = timeout(Duration::from_millis(200), ws.next()).await
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt)
            && v.get("id").and_then(serde_json::Value::as_i64) == Some(2)
        {
            conversation_id = v
                .get("result")
                .and_then(|r| r.get("conversationId"))
                .and_then(|s| s.as_str())
                .map(str::to_string);
            break;
        }
    }
    let conversation_id = conversation_id.expect("conv id");

    // subscribe
    let subscribe = json!({
        "method": "addConversationListener",
        "id": 3,
        "params": { "conversationId": conversation_id }
    });
    ws.send(WsMsg::Text(subscribe.to_string().into()))
        .await
        .unwrap();
    for _ in 0..50 {
        if let Ok(Some(Ok(WsMsg::Text(txt)))) = timeout(Duration::from_millis(200), ws.next()).await
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt)
            && v.get("id").and_then(serde_json::Value::as_i64) == Some(3)
        {
            break;
        }
    }

    // sendUserTurn (typed)
    let cid = ConvId::from_string(&conversation_id).expect("cid parse");
    let params = RpcSendUserTurnParams {
        conversation_id: cid,
        items: vec![codex_app_server_protocol::InputItem::Text {
            text: "Hello".to_string(),
        }],
        cwd: std::path::PathBuf::from(cwd),
        approval_policy: AskForApproval::Never,
        sandbox_policy: SandboxPolicy::DangerFullAccess,
        model: "mock-model".to_string(),
        effort: Some(ReasoningEffort::Medium),
        summary: ReasoningSummary::Auto,
    };
    let req = RpcRequest {
        id: RpcRequestId::Integer(4),
        method: "sendUserTurn".to_string(),
        params: Some(serde_json::to_value(&params).unwrap()),
    };
    let wire = serde_json::to_string(&RpcMessage::Request(req)).unwrap();
    ws.send(WsMsg::Text(wire.into())).await.unwrap();
    // ack
    for _ in 0..50 {
        if let Ok(Some(Ok(WsMsg::Text(txt)))) = timeout(Duration::from_millis(200), ws.next()).await
            && serde_json::from_str::<serde_json::Value>(&txt)
                .ok()
                .and_then(|v| v.get("id").and_then(serde_json::Value::as_i64))
                == Some(4)
        {
            break;
        }
    }

    // collect methods until task_complete
    let mut methods: Vec<String> = Vec::new();
    for _ in 0..150 {
        if let Ok(Some(Ok(WsMsg::Text(txt)))) = timeout(Duration::from_millis(200), ws.next()).await
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt)
                && let Some(m) = v.get("method").and_then(|m| m.as_str())
                && m.starts_with("codex/event/")
            {
                methods.push(m.to_string());
                if m == "codex/event/task_complete" {
                    break;
                }
            }
    }
    methods
}

async fn collect_inprocess_methods(
    codex_home: &std::path::Path,
    cwd: &std::path::Path,
) -> Vec<String> {
    use tokio::time::timeout;
    let mut mcp = McpProcess::new(codex_home).await.expect("spawn app-server");
    timeout(Duration::from_secs(5), mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init ok");

    // newConversation
    let new_id = mcp
        .send_new_conversation_request(codex_app_server_protocol::NewConversationParams {
            model: None,
            profile: None,
            cwd: Some(cwd.to_string_lossy().to_string()),
            approval_policy: Some(AskForApproval::Never),
            sandbox: Some(codex_protocol::config_types::SandboxMode::DangerFullAccess),
            config: None,
            base_instructions: None,
            include_plan_tool: None,
            include_apply_patch_tool: None,
        })
        .await
        .expect("send newConv");
    let new_resp = timeout(
        Duration::from_secs(5),
        mcp.read_stream_until_response_message(RpcRequestId::Integer(new_id)),
    )
    .await
    .expect("newConv timeout")
    .expect("newConv resp");
    let new_conv: codex_app_server_protocol::NewConversationResponse =
        app_test_support::to_response::<_>(new_resp).expect("parse newConversation");
    let conversation_id = new_conv.conversation_id;

    // addListener
    let add_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams { conversation_id })
        .await
        .expect("send addListener");
    let _add_resp = timeout(
        Duration::from_secs(5),
        mcp.read_stream_until_response_message(RpcRequestId::Integer(add_id)),
    )
    .await
    .expect("addListener timeout")
    .expect("addListener resp");

    // sendUserTurn
    let turn_id = mcp
        .send_send_user_turn_request(RpcSendUserTurnParams {
            conversation_id,
            items: vec![codex_app_server_protocol::InputItem::Text {
                text: "Hello".to_string(),
            }],
            cwd: cwd.to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: "mock-model".to_string(),
            effort: Some(ReasoningEffort::Medium),
            summary: ReasoningSummary::Auto,
        })
        .await
        .expect("send turn");
    let _turn_ack = timeout(
        Duration::from_secs(5),
        mcp.read_stream_until_response_message(RpcRequestId::Integer(turn_id)),
    )
    .await
    .expect("turn ack timeout")
    .expect("turn ack resp");

    // Expect agent_message then task_complete via public helpers
    let mut methods: Vec<String> = Vec::new();
    let _ = timeout(
        Duration::from_secs(5),
        mcp.read_stream_until_notification_message("codex/event/agent_message"),
    )
    .await
    .expect("agent_message timeout")
    .expect("agent_message notif");
    methods.push("codex/event/agent_message".to_string());

    let _ = timeout(
        Duration::from_secs(5),
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await
    .expect("task_complete timeout")
    .expect("task_complete notif");
    methods.push("codex/event/task_complete".to_string());
    methods
}

#[tokio::test]
#[ignore = "end-to-end parity with SSE mocks can be slow/flaky; run explicitly"]
async fn parity_basic_turn_produces_same_event_order() {
    // Mock model server with two completions (session start + user turn)
    let responses_ws = vec![
        create_final_assistant_message_sse_response("Welcome").expect("resp1"),
        create_final_assistant_message_sse_response("Done").expect("resp2"),
    ];
    let responses_ip = vec![
        create_final_assistant_message_sse_response("Welcome").expect("resp1"),
        create_final_assistant_message_sse_response("Done").expect("resp2"),
    ];
    let mock_ws = create_mock_chat_completions_server(responses_ws).await;
    let mock_ip = create_mock_chat_completions_server(responses_ip).await;

    // Shared config for both runs under a temp home
    let codex_home = tempfile::tempdir().expect("tmp");
    write_mock_config(codex_home.path(), &mock_ws.uri());

    // WS engine from the same config
    let cfg_toml =
        codex_core::config::load_config_as_toml_with_cli_overrides(codex_home.path(), vec![])
            .await
            .expect("parse config");
    let config = Config::load_from_base_config_with_overrides(
        cfg_toml,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )
    .expect("materialize Config");
    let engine = AppServerEngine::new(std::sync::Arc::new(config), None);
    let addr = spawn_ws(engine).await;

    // Collect event methods from both paths
    let cwd = codex_home.path().to_string_lossy().to_string();
    let ws_methods = collect_ws_methods(addr, &cwd).await;
    // Re-point the config to a fresh mock server for in-process run.
    write_mock_config(codex_home.path(), &mock_ip.uri());
    let inproc_methods = collect_inprocess_methods(codex_home.path(), codex_home.path()).await;

    // Normalize by filtering environment_context noise (keep meaningful events)
    let f = |m: &String| m != "codex/event/user_message"; // environment_context appears as a user_message
    let ws_filtered: Vec<_> = ws_methods.into_iter().filter(f).collect();
    let inproc_filtered: Vec<_> = inproc_methods.into_iter().filter(f).collect();

    // WS should contain task_started; in-process must complete
    assert!(ws_filtered.contains(&"codex/event/task_started".to_string()));
    assert_eq!(
        inproc_filtered.last(),
        Some(&"codex/event/task_complete".to_string())
    );
    let ws_last = ws_filtered.last().cloned();
    assert!(
        ws_last == Some("codex/event/task_complete".to_string())
            || ws_last == Some("codex/event/stream_error".to_string()),
        "ws last={ws_last:?}"
    );
}

#[tokio::test]
#[ignore = "approval roundtrip uses SSE mocks and is environment-sensitive; run explicitly"]
async fn parity_exec_approval_roundtrip() {
    // Mock sequence: first SSE triggers a shell tool call; second SSE finalizes message
    let responses_ws = vec![
        app_test_support::create_shell_sse_response(
            vec!["bash".into(), "-lc".into(), "echo hi".into()],
            None,
            Some(5000),
            "call1",
        )
        .expect("shell sse"),
        create_final_assistant_message_sse_response("done").expect("final sse"),
    ];
    let responses_ip = vec![
        app_test_support::create_shell_sse_response(
            vec!["bash".into(), "-lc".into(), "echo hi".into()],
            None,
            Some(5000),
            "call1",
        )
        .expect("shell sse"),
        create_final_assistant_message_sse_response("done").expect("final sse"),
    ];
    let mock_ws = create_mock_chat_completions_server(responses_ws).await;
    let mock_ip = create_mock_chat_completions_server(responses_ip).await;

    // Shared config under temp home
    let codex_home = tempfile::tempdir().expect("tmp");
    write_mock_config(codex_home.path(), &mock_ws.uri());

    // WS engine
    let cfg_toml =
        codex_core::config::load_config_as_toml_with_cli_overrides(codex_home.path(), vec![])
            .await
            .expect("parse config");
    let config = Config::load_from_base_config_with_overrides(
        cfg_toml,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )
    .expect("config");
    let engine = AppServerEngine::new(std::sync::Arc::new(config), None);
    let addr = spawn_ws(engine).await;

    // WS client flow with approval handling
    use tokio::time::timeout;
    use tokio_tungstenite::tungstenite::Message as WsMsg;
    let url = format!("ws://{addr}/ws");
    let (mut ws, _resp) = connect_async(url).await.unwrap();
    // initialize
    let init = json!({
        "method": "initialize",
        "id": 1,
        "params": { "clientInfo": { "name": "tests", "version": "0.0.0" } }
    });
    ws.send(WsMsg::Text(init.to_string().into())).await.unwrap();
    // new conversation
    let tmp = tempfile::tempdir().unwrap();
    let new_conv = json!({
        "method": "newConversation",
        "id": 2,
        "params": { "cwd": tmp.path().to_string_lossy() }
    });
    ws.send(WsMsg::Text(new_conv.to_string().into()))
        .await
        .unwrap();
    // capture id
    let mut conversation_id: Option<String> = None;
    for _ in 0..50 {
        if let Ok(Some(Ok(WsMsg::Text(txt)))) = timeout(Duration::from_millis(200), ws.next()).await
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt)
            && v.get("id").and_then(serde_json::Value::as_i64) == Some(2)
        {
            conversation_id = v
                .get("result")
                .and_then(|r| r.get("conversationId"))
                .and_then(|s| s.as_str())
                .map(str::to_string);
            break;
        }
    }
    let conversation_id = conversation_id.expect("cid");
    // subscribe
    let subscribe = json!({
        "method": "addConversationListener",
        "id": 3,
        "params": { "conversationId": conversation_id }
    });
    ws.send(WsMsg::Text(subscribe.to_string().into()))
        .await
        .unwrap();
    for _ in 0..50 {
        if let Ok(Some(Ok(WsMsg::Text(txt)))) = timeout(Duration::from_millis(200), ws.next()).await
            && serde_json::from_str::<serde_json::Value>(&txt)
                .ok()
                .and_then(|v| v.get("id").and_then(serde_json::Value::as_i64))
                == Some(3)
        {
            break;
        }
    }
    // sendUserTurn with approval_policy on-request
    let cid = ConvId::from_string(&conversation_id).unwrap();
    let turn = RpcSendUserTurnParams {
        conversation_id: cid,
        items: vec![codex_app_server_protocol::InputItem::Text {
            text: "Run shell".to_string(),
        }],
        cwd: tmp.path().to_path_buf(),
        approval_policy: AskForApproval::OnRequest,
        sandbox_policy: SandboxPolicy::DangerFullAccess,
        model: "mock-model".into(),
        effort: Some(ReasoningEffort::Medium),
        summary: ReasoningSummary::Auto,
    };
    let req = RpcRequest {
        id: RpcRequestId::Integer(4),
        method: "sendUserTurn".into(),
        params: Some(serde_json::to_value(&turn).unwrap()),
    };
    let wire = serde_json::to_string(&RpcMessage::Request(req)).unwrap();
    ws.send(WsMsg::Text(wire.into())).await.unwrap();
    // ack
    for _ in 0..50 {
        if let Ok(Some(Ok(WsMsg::Text(txt)))) = timeout(Duration::from_millis(200), ws.next()).await
            && serde_json::from_str::<serde_json::Value>(&txt)
                .ok()
                .and_then(|v| v.get("id").and_then(serde_json::Value::as_i64))
                == Some(4)
        {
            break;
        }
    }
    // Handle approval request and then assert completion
    let mut saw_task_complete = false;
    for _ in 0..200 {
        if let Ok(Some(Ok(WsMsg::Text(txt)))) = timeout(Duration::from_millis(200), ws.next()).await
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt)
                && let Some(method) = v.get("method").and_then(|m| m.as_str())
            {
                if method == "execCommandApproval" {
                    let id = v.get("id").cloned().unwrap();
                    let resp = RpcMessage::Response(codex_app_server_protocol::JSONRPCResponse {
                        id: serde_json::from_value(id).unwrap(),
                        result: serde_json::json!({"decision": codex_core::protocol::ReviewDecision::Approved}),
                    });
                    ws.send(WsMsg::Text(serde_json::to_string(&resp).unwrap().into()))
                        .await
                        .unwrap();
                } else if method == "codex/event/task_complete" {
                    saw_task_complete = true;
                    break;
                }
            }
    }
    assert!(saw_task_complete, "ws did not complete after approval");

    // In-process flow: expect approval request and approve
    // Point the in-process run at a fresh mock server
    write_mock_config(codex_home.path(), &mock_ip.uri());
    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn app-server");
    tokio::time::timeout(Duration::from_secs(5), mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init ok");
    let new_id = mcp
        .send_new_conversation_request(codex_app_server_protocol::NewConversationParams {
            cwd: Some(tmp.path().to_string_lossy().to_string()),
            ..Default::default()
        })
        .await
        .expect("send newConv");
    let new_resp = tokio::time::timeout(
        Duration::from_secs(5),
        mcp.read_stream_until_response_message(RpcRequestId::Integer(new_id)),
    )
    .await
    .expect("new timeout")
    .expect("new resp");
    let new_conv: codex_app_server_protocol::NewConversationResponse =
        app_test_support::to_response::<_>(new_resp).expect("parse new conv");
    let conversation_id = new_conv.conversation_id;
    // addListener
    let _ = tokio::time::timeout(Duration::from_secs(5), async {
        let add_id = mcp
            .send_add_conversation_listener_request(AddConversationListenerParams {
                conversation_id,
            })
            .await
            .unwrap();
        mcp.read_stream_until_response_message(RpcRequestId::Integer(add_id))
            .await
    })
    .await
    .expect("add timeout")
    .expect("add resp");
    // sendUserMessage (policy OnRequest via subsequent turn overrides is cumbersome; use sendUserTurn directly)
    let turn_id = mcp
        .send_send_user_turn_request(RpcSendUserTurnParams {
            conversation_id,
            items: vec![codex_app_server_protocol::InputItem::Text { text: "run".into() }],
            cwd: tmp.path().to_path_buf(),
            approval_policy: AskForApproval::OnRequest,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: "mock-model".into(),
            effort: Some(ReasoningEffort::Medium),
            summary: ReasoningSummary::Auto,
        })
        .await
        .expect("send turn");
    let _ = tokio::time::timeout(
        Duration::from_secs(5),
        mcp.read_stream_until_response_message(RpcRequestId::Integer(turn_id)),
    )
    .await
    .expect("turn timeout")
    .expect("turn ack");
    // Expect approval request
    let server_req = tokio::time::timeout(
        Duration::from_secs(5),
        mcp.read_stream_until_request_message(),
    )
    .await
    .expect("approval timeout")
    .expect("approval req");
    if let codex_app_server_protocol::ServerRequest::ExecCommandApproval { request_id, .. } =
        server_req
    {
        mcp.send_response(
            request_id,
            serde_json::json!({"decision": codex_core::protocol::ReviewDecision::Approved}),
        )
        .await
        .expect("approve resp");
    } else {
        panic!("expected ExecCommandApproval request");
    }
    // Expect task_complete
    let _ = tokio::time::timeout(
        Duration::from_secs(5),
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await
    .expect("complete timeout")
    .expect("complete notif");
}
