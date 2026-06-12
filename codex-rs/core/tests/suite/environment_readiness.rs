use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_exec_server::ExecServerRuntimePaths;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::sync::oneshot;
use tokio::time::timeout;

fn environment_contexts(body: &Value) -> Vec<String> {
    body.get("input")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("role").and_then(Value::as_str) == Some("user"))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter_map(|content| content.get("text").and_then(Value::as_str))
        .filter(|text| text.starts_with("<environment_context>"))
        .map(str::to_string)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_environment_context_updates_before_next_sample() -> Result<()> {
    let first_plan = serde_json::to_string(&json!({
        "plan": [{"step": "wait for environment", "status": "in_progress"}]
    }))?;
    let second_plan = serde_json::to_string(&json!({
        "plan": [{"step": "wait for environment", "status": "completed"}]
    }))?;
    let (release_first_response, first_response_gate) = oneshot::channel();
    let (server, completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![
                    responses::ev_response_created("resp-1"),
                    responses::ev_function_call("plan-1", "update_plan", &first_plan),
                ]),
            },
            StreamingSseChunk {
                gate: Some(first_response_gate),
                body: responses::sse(vec![responses::ev_completed("resp-1")]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_function_call("plan-2", "update_plan", &second_plan),
                responses::ev_completed("resp-2"),
            ]),
        }],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                responses::ev_response_created("resp-3"),
                responses::ev_assistant_message("msg-1", "done"),
                responses::ev_completed("resp-3"),
            ]),
        }],
    ])
    .await;
    let test = test_codex().build_with_streaming_server(&server).await?;
    let environment_manager = test.thread_manager.environment_manager();
    let environment =
        environment_manager.register_pending_environment("pending-environment".to_string())?;
    let environment_selections = TurnEnvironmentSelections::new(
        test.config.cwd.clone(),
        vec![TurnEnvironmentSelection {
            environment_id: "pending-environment".to_string(),
            cwd: test.config.cwd.clone(),
        }],
    );

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "continue when the environment is ready".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                environments: Some(environment_selections),
                ..Default::default()
            },
        })
        .await?;

    timeout(Duration::from_secs(10), server.wait_for_request_count(1)).await?;
    let first_request = server
        .requests()
        .await
        .into_iter()
        .next()
        .context("first model request")?;
    let first_request: Value = serde_json::from_slice(&first_request)?;
    let first_contexts = environment_contexts(&first_request);
    assert_eq!(first_contexts.len(), 1);
    assert!(first_contexts[0].contains("<shell>still loading</shell>"));

    let reserved_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_address = reserved_listener.local_addr()?;
    drop(reserved_listener);
    let exec_server_url = format!("ws://{exec_server_address}");
    let exec_server_url_for_task = exec_server_url.clone();
    let runtime_paths = ExecServerRuntimePaths::new(
        std::env::current_exe()?,
        /*codex_linux_sandbox_exe*/ None,
    )?;
    let exec_server_task = tokio::spawn(async move {
        codex_exec_server::run_main(&exec_server_url_for_task, runtime_paths).await
    });
    timeout(Duration::from_secs(2), async {
        loop {
            if tokio::net::TcpStream::connect(exec_server_address)
                .await
                .is_ok()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;

    environment_manager.upsert_environment("pending-environment".to_string(), exec_server_url)?;
    let loaded_info = timeout(Duration::from_secs(2), async {
        loop {
            if let Some(info) = environment.current_info() {
                break info;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    release_first_response
        .send(())
        .map_err(|_| anyhow::anyhow!("first response gate closed"))?;

    timeout(Duration::from_secs(10), server.wait_for_request_count(3)).await?;
    for completion in completions {
        timeout(Duration::from_secs(10), completion).await??;
    }
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = server.requests().await;
    let second_request: Value = serde_json::from_slice(&requests[1])?;
    let third_request: Value = serde_json::from_slice(&requests[2])?;
    let second_contexts = environment_contexts(&second_request);
    let third_contexts = environment_contexts(&third_request);
    assert_eq!(second_contexts.len(), 2);
    assert!(second_contexts[0].contains("<shell>still loading</shell>"));
    let environment_update = &second_contexts[1];
    assert!(environment_update.contains(&format!("<shell>{}</shell>", loaded_info.shell.name)));
    assert!(!environment_update.contains("<current_date>"));
    assert!(!environment_update.contains("<timezone>"));
    assert!(!environment_update.contains("<network"));
    assert!(!environment_update.contains("<filesystem>"));
    assert_eq!(third_contexts, second_contexts);

    server.shutdown().await;
    exec_server_task.abort();
    let _ = exec_server_task.await;
    Ok(())
}
