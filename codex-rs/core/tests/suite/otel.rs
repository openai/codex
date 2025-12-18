use codex_core::config::Constrained;
use codex_core::features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_local_shell_call;
use core_test_support::responses::ev_message_item_added;
use core_test_support::responses::ev_output_text_delta;
use core_test_support::responses::ev_reasoning_item;
use core_test_support::responses::ev_reasoning_summary_text_delta;
use core_test_support::responses::ev_reasoning_text_delta;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use std::sync::Mutex;
use tracing::Level;
use tracing_test::traced_test;

use tracing_subscriber::fmt::format::FmtSpan;
use tracing_test::internal::MockWriter;

#[tokio::test]
#[traced_test]
async fn responses_api_emits_api_request_event() {
    let server = start_mock_server().await;

    mount_sse_once(&server, sse(vec![ev_completed("done")])).await;

    let TestCodex { codex, .. } = test_codex().build(&server).await.unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| line.contains("codex.api_request"))
            .map(|_| Ok(()))
            .unwrap_or_else(|| Err("expected codex.api_request event".to_string()))
    });

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| line.contains("codex.conversation_starts"))
            .map(|_| Ok(()))
            .unwrap_or_else(|| Err("expected codex.conversation_starts event".to_string()))
    });
}

#[tokio::test]
#[traced_test]
async fn process_sse_emits_tracing_for_output_item() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![ev_assistant_message("id1", "hi"), ev_completed("id2")]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex().build(&server).await.unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| {
                line.contains("codex.sse_event")
                    && line.contains("event.kind=response.output_item.done")
            })
            .map(|_| Ok(()))
            .unwrap_or(Err("missing response.output_item.done event".to_string()))
    });
}

#[tokio::test]
#[traced_test]
async fn process_sse_emits_failed_event_on_parse_error() {
    let server = start_mock_server().await;

    mount_sse_once(&server, "data: not-json\n\n".to_string()).await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.features.disable(Feature::GhostCommit);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| {
                line.contains("codex.sse_event")
                    && line.contains("error.message")
                    && line.contains("expected ident at line 1 column 2")
            })
            .map(|_| Ok(()))
            .unwrap_or(Err("missing codex.sse_event".to_string()))
    });
}

#[tokio::test]
#[traced_test]
async fn process_sse_records_failed_event_when_stream_closes_without_completed() {
    let server = start_mock_server().await;

    mount_sse_once(&server, sse(vec![ev_assistant_message("id", "hi")])).await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.features.disable(Feature::GhostCommit);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| {
                line.contains("codex.sse_event")
                    && line.contains("error.message")
                    && line.contains("stream closed before response.completed")
            })
            .map(|_| Ok(()))
            .unwrap_or(Err("missing codex.sse_event".to_string()))
    });
}

#[tokio::test]
#[traced_test]
async fn process_sse_failed_event_records_response_error_message() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![serde_json::json!({
            "type": "response.failed",
            "response": {
                "error": {
                    "message": "boom",
                    "code": "bad"
                }
            }
        })]),
    )
    .await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.features.disable(Feature::GhostCommit);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| {
                line.contains("codex.sse_event")
                    && line.contains("event.kind=response.failed")
                    && line.contains("error.message")
                    && line.contains("boom")
            })
            .map(|_| Ok(()))
            .unwrap_or(Err("missing codex.sse_event".to_string()))
    });
}

#[tokio::test]
#[traced_test]
async fn process_sse_failed_event_logs_parse_error() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![serde_json::json!({
            "type": "response.failed",
            "response": {
                "error": "not-an-object"
            }
        })]),
    )
    .await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.features.disable(Feature::GhostCommit);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| {
                line.contains("codex.sse_event") && line.contains("event.kind=response.failed")
            })
            .map(|_| Ok(()))
            .unwrap_or(Err("missing codex.sse_event".to_string()))
    });
}

#[tokio::test]
#[traced_test]
async fn process_sse_failed_event_logs_missing_error() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![serde_json::json!({
            "type": "response.failed",
            "response": {}
        })]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.features.disable(Feature::GhostCommit);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| {
                line.contains("codex.sse_event") && line.contains("event.kind=response.failed")
            })
            .map(|_| Ok(()))
            .unwrap_or(Err("missing codex.sse_event".to_string()))
    });
}

#[tokio::test]
#[traced_test]
async fn process_sse_failed_event_logs_response_completed_parse_error() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![serde_json::json!({
            "type": "response.completed",
            "response": {}
        })]),
    )
    .await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.features.disable(Feature::GhostCommit);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| {
                line.contains("codex.sse_event")
                    && line.contains("event.kind=response.completed")
                    && line.contains("error.message")
                    && line.contains("failed to parse ResponseCompleted")
            })
            .map(|_| Ok(()))
            .unwrap_or(Err("missing codex.sse_event".to_string()))
    });
}

#[tokio::test]
#[traced_test]
async fn process_sse_emits_completed_telemetry() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![serde_json::json!({
            "type": "response.completed",
            "response": {
                "id": "resp1",
                "usage": {
                    "input_tokens": 3,
                    "input_tokens_details": { "cached_tokens": 1 },
                    "output_tokens": 5,
                    "output_tokens_details": { "reasoning_tokens": 2 },
                    "total_tokens": 9
                }
            }
        })]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex().build(&server).await.unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| {
                line.contains("codex.sse_event")
                    && line.contains("event.kind=response.completed")
                    && line.contains("input_token_count=3")
                    && line.contains("output_token_count=5")
                    && line.contains("cached_token_count=1")
                    && line.contains("reasoning_token_count=2")
                    && line.contains("tool_token_count=9")
            })
            .map(|_| Ok(()))
            .unwrap_or(Err("missing response.completed telemetry".to_string()))
    });
}

#[tokio::test]
async fn handle_responses_span_records_response_kind_and_tool_name() {
    let buffer: &'static Mutex<Vec<u8>> = Box::leak(Box::new(Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .with_level(true)
        .with_ansi(false)
        .with_max_level(Level::TRACE)
        .with_span_events(FmtSpan::FULL)
        .with_writer(MockWriter::new(buffer))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_function_call("function-call", "nonexistent", "{\"value\":1}"),
            ev_completed("done"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "tool handled"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.features.disable(Feature::GhostCommit);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let logs = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();

    assert!(
        logs.contains("handle_responses{otel.name=\"function_call\"")
            && logs.contains("tool_name=\"nonexistent\"")
            && logs.contains("from=\"output_item_done\""),
        "missing handle_responses span with function call metadata\nlogs:\n{logs}"
    );
    assert!(
        logs.contains("handle_responses{otel.name=\"completed\""),
        "missing handle_responses span for completion\nlogs:\n{logs}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn record_responses_sets_span_fields_for_response_events() {
    let buffer: &'static Mutex<Vec<u8>> = Box::leak(Box::new(Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .with_level(true)
        .with_ansi(false)
        .with_max_level(Level::TRACE)
        .with_span_events(FmtSpan::FULL)
        .with_writer(MockWriter::new(buffer))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let server = start_mock_server().await;

    let sse_body = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call("call-1", "fn", "{\"value\":1}"),
        ev_custom_tool_call("custom-1", "custom_tool", "{\"key\":\"value\"}"),
        ev_message_item_added("msg-added", "hi there"),
        ev_output_text_delta("delta"),
        ev_reasoning_summary_text_delta("summary-delta"),
        ev_reasoning_text_delta("raw-delta"),
        ev_function_call("call-1", "fn", "{\"key\":\"value\"}"),
        ev_custom_tool_call("custom-1", "custom_tool", "{\"key\":\"value\"}"),
        ev_assistant_message("msg-1", "agent"),
        ev_reasoning_item("reasoning-1", &["summary"], &[]),
        ev_completed("resp-1"),
    ]);

    mount_response_once(&server, sse_response(sse_body)).await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.features.disable(Feature::GhostCommit);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let logs = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();

    let expected = [
        ("created", None::<&str>, None::<&str>),
        ("rate_limits", None, None),
        ("function_call", Some("output_item_added"), Some("fn")),
        ("message_from_assistant", Some("output_item_done"), None),
        ("reasoning", Some("output_item_done"), None),
        ("text_delta", None, None),
        ("reasoning_summary_delta", None, None),
        ("reasoning_content_delta", None, None),
        ("completed", None, None),
    ];

    for (name, from, tool_name) in expected {
        assert!(
            logs.contains(&format!("handle_responses{{otel.name=\"{name}\"")),
            "missing otel.name={name}\nlogs:\n{logs}"
        );
        if let Some(from) = from {
            assert!(
                logs.contains(&format!("from=\"{from}\"")),
                "missing from={from} for {name}\nlogs:\n{logs}"
            );
        }
        if let Some(tool_name) = tool_name {
            assert!(
                logs.contains(&format!("tool_name=\"{tool_name}\"")),
                "missing tool_name={tool_name} for {name}\nlogs:\n{logs}"
            );
        }
    }
}

#[tokio::test]
#[traced_test]
async fn handle_response_item_records_tool_result_for_custom_tool_call() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_custom_tool_call(
                "custom-tool-call",
                "unsupported_tool",
                "{\"key\":\"value\"}",
            ),
            ev_completed("done"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.features.disable(Feature::GhostCommit);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TokenCount(_))).await;

    logs_assert(|lines: &[&str]| {
        let line = lines
            .iter()
            .find(|line| {
                line.contains("codex.tool_result") && line.contains("call_id=custom-tool-call")
            })
            .ok_or_else(|| "missing codex.tool_result event".to_string())?;

        if !line.contains("tool_name=unsupported_tool") {
            return Err("missing tool_name field".to_string());
        }
        if !line.contains("arguments={\"key\":\"value\"}") {
            return Err("missing arguments field".to_string());
        }
        if !line.contains("output=unsupported custom tool call: unsupported_tool") {
            return Err("missing output field".to_string());
        }
        if !line.contains("success=false") {
            return Err("missing success field".to_string());
        }

        Ok(())
    });
}

#[tokio::test]
#[traced_test]
async fn handle_response_item_records_tool_result_for_function_call() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_function_call("function-call", "nonexistent", "{\"value\":1}"),
            ev_completed("done"),
        ]),
    )
    .await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.features.disable(Feature::GhostCommit);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TokenCount(_))).await;

    logs_assert(|lines: &[&str]| {
        let line = lines
            .iter()
            .find(|line| {
                line.contains("codex.tool_result") && line.contains("call_id=function-call")
            })
            .ok_or_else(|| "missing codex.tool_result event".to_string())?;

        if !line.contains("tool_name=nonexistent") {
            return Err("missing tool_name field".to_string());
        }
        if !line.contains("arguments={\"value\":1}") {
            return Err("missing arguments field".to_string());
        }
        if !line.contains("output=unsupported call: nonexistent") {
            return Err("missing output field".to_string());
        }
        if !line.contains("success=false") {
            return Err("missing success field".to_string());
        }

        Ok(())
    });
}

#[tokio::test]
#[traced_test]
async fn handle_response_item_records_tool_result_for_local_shell_missing_ids() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![
            serde_json::json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "local_shell_call",
                    "status": "completed",
                    "action": {
                        "type": "exec",
                        "command": vec!["/bin/echo", "hello"],
                    }
                }
            }),
            ev_completed("done"),
        ]),
    )
    .await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.features.disable(Feature::GhostCommit);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TokenCount(_))).await;

    logs_assert(|lines: &[&str]| {
        let line = lines
            .iter()
            .find(|line| {
                line.contains("codex.tool_result")
                    && line.contains(&"tool_name=local_shell".to_string())
                    && line.contains("output=LocalShellCall without call_id or id")
            })
            .ok_or_else(|| "missing codex.tool_result event".to_string())?;

        if !line.contains("success=false") {
            return Err("missing success field".to_string());
        }

        Ok(())
    });
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[traced_test]
async fn handle_response_item_records_tool_result_for_local_shell_call() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_local_shell_call("shell-call", "completed", vec!["/bin/echo", "shell"]),
            ev_completed("done"),
        ]),
    )
    .await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.features.disable(Feature::GhostCommit);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TokenCount(_))).await;

    logs_assert(|lines: &[&str]| {
        let line = lines
            .iter()
            .find(|line| line.contains("codex.tool_result") && line.contains("call_id=shell-call"))
            .ok_or_else(|| "missing codex.tool_result event".to_string())?;

        if !line.contains("tool_name=local_shell") {
            return Err("missing tool_name field".to_string());
        }
        if !line.contains("arguments=/bin/echo shell") {
            return Err("missing arguments field".to_string());
        }
        let output_idx = line
            .find("output=")
            .ok_or_else(|| "missing output field".to_string())?;
        if line[output_idx + "output=".len()..].is_empty() {
            return Err("empty output field".to_string());
        }
        if !line.contains("success=false") {
            return Err("missing success field".to_string());
        }

        Ok(())
    });
}

fn tool_decision_assertion<'a>(
    call_id: &'a str,
    expected_decision: &'a str,
    expected_source: &'a str,
) -> impl Fn(&[&str]) -> Result<(), String> + 'a {
    let call_id = call_id.to_string();
    let expected_decision = expected_decision.to_string();
    let expected_source = expected_source.to_string();

    move |lines: &[&str]| {
        let line = lines
            .iter()
            .find(|line| {
                line.contains("codex.tool_decision") && line.contains(&format!("call_id={call_id}"))
            })
            .ok_or_else(|| format!("missing codex.tool_decision event for {call_id}"))?;

        let lower = line.to_lowercase();
        if !lower.contains("tool_name=local_shell") {
            return Err("missing tool_name for local_shell".to_string());
        }
        if !lower.contains(&format!("decision={expected_decision}")) {
            return Err(format!("unexpected decision for {call_id}"));
        }
        if !lower.contains(&format!("source={expected_source}")) {
            return Err(format!("unexpected source for {expected_source}"));
        }

        Ok(())
    }
}

#[tokio::test]
#[traced_test]
async fn handle_container_exec_autoapprove_from_config_records_tool_decision() {
    let server = start_mock_server().await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_local_shell_call(
                "auto_config_call",
                "completed",
                vec!["/bin/echo", "local shell"],
            ),
            ev_completed("done"),
        ]),
    )
    .await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.sandbox_policy = SandboxPolicy::DangerFullAccess;
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    logs_assert(tool_decision_assertion(
        "auto_config_call",
        "approved",
        "config",
    ));
}

#[tokio::test]
#[traced_test]
async fn handle_container_exec_user_approved_records_tool_decision() {
    let server = start_mock_server().await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_local_shell_call("user_approved_call", "completed", vec!["/bin/date"]),
            ev_completed("done"),
        ]),
    )
    .await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.approval_policy = Constrained::allow_any(AskForApproval::UnlessTrusted);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "approved".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecApprovalRequest(_))).await;

    codex
        .submit(Op::ExecApproval {
            id: "0".into(),
            decision: ReviewDecision::Approved,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TokenCount(_))).await;

    logs_assert(tool_decision_assertion(
        "user_approved_call",
        "approved",
        "user",
    ));
}

#[tokio::test]
#[traced_test]
async fn handle_container_exec_user_approved_for_session_records_tool_decision() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_local_shell_call("user_approved_session_call", "completed", vec!["/bin/date"]),
            ev_completed("done"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.approval_policy = Constrained::allow_any(AskForApproval::UnlessTrusted);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "persist".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecApprovalRequest(_))).await;

    codex
        .submit(Op::ExecApproval {
            id: "0".into(),
            decision: ReviewDecision::ApprovedForSession,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TokenCount(_))).await;

    logs_assert(tool_decision_assertion(
        "user_approved_session_call",
        "approvedforsession",
        "user",
    ));
}

#[tokio::test]
#[traced_test]
async fn handle_sandbox_error_user_approves_retry_records_tool_decision() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_local_shell_call("sandbox_retry_call", "completed", vec!["/bin/date"]),
            ev_completed("done"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.approval_policy = Constrained::allow_any(AskForApproval::UnlessTrusted);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "retry".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecApprovalRequest(_))).await;

    codex
        .submit(Op::ExecApproval {
            id: "0".into(),
            decision: ReviewDecision::Approved,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TokenCount(_))).await;

    logs_assert(tool_decision_assertion(
        "sandbox_retry_call",
        "approved",
        "user",
    ));
}

#[tokio::test]
#[traced_test]
async fn handle_container_exec_user_denies_records_tool_decision() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_local_shell_call("user_denied_call", "completed", vec!["/bin/date"]),
            ev_completed("done"),
        ]),
    )
    .await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;
    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.approval_policy = Constrained::allow_any(AskForApproval::UnlessTrusted);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "deny".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecApprovalRequest(_))).await;

    codex
        .submit(Op::ExecApproval {
            id: "0".into(),
            decision: ReviewDecision::Denied,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TokenCount(_))).await;

    logs_assert(tool_decision_assertion(
        "user_denied_call",
        "denied",
        "user",
    ));
}

#[tokio::test]
#[traced_test]
async fn handle_sandbox_error_user_approves_for_session_records_tool_decision() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_local_shell_call("sandbox_session_call", "completed", vec!["/bin/date"]),
            ev_completed("done"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.approval_policy = Constrained::allow_any(AskForApproval::UnlessTrusted);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "persist".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecApprovalRequest(_))).await;

    codex
        .submit(Op::ExecApproval {
            id: "0".into(),
            decision: ReviewDecision::ApprovedForSession,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TokenCount(_))).await;

    logs_assert(tool_decision_assertion(
        "sandbox_session_call",
        "approvedforsession",
        "user",
    ));
}

#[tokio::test]
#[traced_test]
async fn handle_sandbox_error_user_denies_records_tool_decision() {
    let server = start_mock_server().await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_local_shell_call("sandbox_deny_call", "completed", vec!["/bin/date"]),
            ev_completed("done"),
        ]),
    )
    .await;

    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("done"),
        ]),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.approval_policy = Constrained::allow_any(AskForApproval::UnlessTrusted);
        })
        .build(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "deny".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecApprovalRequest(_))).await;

    codex
        .submit(Op::ExecApproval {
            id: "0".into(),
            decision: ReviewDecision::Denied,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TokenCount(_))).await;

    logs_assert(tool_decision_assertion(
        "sandbox_deny_call",
        "denied",
        "user",
    ));
}
