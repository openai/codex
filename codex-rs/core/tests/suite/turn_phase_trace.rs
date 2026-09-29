//! Cross-trace admission links and phase boundaries through a real blocked tool.
//! Each case uses an isolated process so SQLite workers share its tracing collector.

use codex_core::RecoverTurnRequest;
use codex_core::StartIfIdleSubmission;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::InMemorySpanExporter;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::SpanData;
use pretty_assertions::assert_eq;
use std::time::Duration;
use test_case::test_case;
use tokio::process::Command;
use tracing_subscriber::prelude::*;

const SUBPROCESS_ENV_VAR: &str = "CODEX_TURN_PHASE_TRACE_TEST_SUBPROCESS";

fn attribute(attributes: &[KeyValue], key: &str) -> Option<String> {
    attributes
        .iter()
        .find(|attribute| attribute.key.as_str() == key)
        .map(|attribute| attribute.value.to_string())
}

async fn wait_for_span(
    exported: &InMemorySpanExporter,
    predicate: impl Fn(&SpanData) -> bool,
) -> anyhow::Result<SpanData> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(span) = exported.get_finished_spans()?.into_iter().find(&predicate) {
                break Ok(span);
            }
            tokio::task::yield_now().await;
        }
    })
    .await?
}

#[derive(Clone, Copy)]
enum Finish {
    Reply,
    Interrupt,
}

#[test_case(Finish::Reply; "reply")]
#[test_case(Finish::Interrupt; "interrupt")]
#[tokio::test(flavor = "current_thread")]
async fn steer_joins_existing_turn_without_extending_sampling(
    finish: Finish,
) -> anyhow::Result<()> {
    if std::env::var_os(SUBPROCESS_ENV_VAR).is_none() {
        let case = match finish {
            Finish::Reply => "reply",
            Finish::Interrupt => "interrupt",
        };
        let test_name = format!(
            "suite::turn_phase_trace::steer_joins_existing_turn_without_extending_sampling::{case}"
        );
        let output = Command::new(std::env::current_exe()?)
            .arg("--exact")
            .arg(&test_name)
            .env(SUBPROCESS_ENV_VAR, "1")
            .output()
            .await?;
        assert!(
            output.status.success(),
            "subprocess test `{test_name}` failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return Ok(());
    }

    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
    let exported = InMemorySpanExporter::default();
    let telemetry = SdkTracerProvider::builder()
        .with_simple_exporter(exported.clone())
        .build();
    tracing_subscriber::registry()
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(telemetry.tracer("turn-phase-test"))
                .with_filter(tracing_subscriber::filter::filter_fn(
                    codex_otel::OtelProvider::trace_export_filter,
                )),
        )
        .try_init()?;
    let server = start_mock_server().await;
    let first = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("first"),
            ev_assistant_message("preamble", "I will check."),
            ev_function_call("gate", "gate", "{}"),
            ev_completed("first"),
        ]),
    )
    .await;
    let mut test = test_codex().build_with_auto_env(&server).await?;
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools: vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
                name: "gate".to_string(),
                description: "Wait for the host.".to_string(),
                input_schema: serde_json::json!({"type": "object", "properties": {}}),
                defer_loading: false,
            })],
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    let thread_id = thread.thread_id.to_string();
    test.codex = thread.thread;
    let input = |text: &str| {
        TurnInputRequest::user_input(vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }])
    };
    let initial = tracing::info_span!(parent: None, "initial_request");
    test.codex
        .start_or_steer_turn(
            input("Start checking").with_trace(codex_otel::span_w3c_trace_context(&initial)),
        )
        .await?;
    let EventMsg::DynamicToolCallRequest(call) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::DynamicToolCallRequest(_))
    })
    .await
    else {
        unreachable!();
    };
    first.single_request();
    // The tool is still blocked. Sampling must already be exportable, even though
    // the dispatch future and its trace remain alive until the host responds.
    let sampling = wait_for_span(&exported, |span| span.name == "codex.sampling").await?;
    let steer = tracing::info_span!(parent: None, "steer_request");
    assert_eq!(
        test.codex
            .start_or_steer_turn(
                input("New question").with_trace(codex_otel::span_w3c_trace_context(&steer)),
            )
            .await?,
        TurnInputSubmission::Steered {
            turn_id: call.turn_id.clone()
        }
    );
    match finish {
        Finish::Reply => {
            let reply = mount_sse_once(
                &server,
                sse(vec![
                    ev_response_created("second"),
                    ev_assistant_message("answer", "Here is the answer."),
                    ev_completed("second"),
                ]),
            )
            .await;
            test.codex
                .submit(Op::DynamicToolResponse {
                    id: call.call_id.clone(),
                    response: DynamicToolResponse {
                        content_items: Vec::new(),
                        success: true,
                    },
                })
                .await?;
            wait_for_event(&test.codex, |event| {
                matches!(event, EventMsg::TurnComplete(_))
            })
            .await;
            assert!(
                reply
                    .single_request()
                    .message_input_texts("user")
                    .contains(&"New question".to_string())
            );
        }
        Finish::Interrupt => {
            test.codex.submit(Op::Interrupt).await?;
            wait_for_event(&test.codex, |event| {
                matches!(event, EventMsg::TurnAborted(_))
            })
            .await;
            let reply = mount_sse_once(
                &server,
                sse(vec![
                    ev_response_created("recovered"),
                    ev_assistant_message("answer", "Recovered answer."),
                    ev_completed("recovered"),
                ]),
            )
            .await;
            let recovery = tracing::info_span!(parent: None, "recovery_request");
            assert_eq!(
                test.codex
                    .recover_turn_if_idle(RecoverTurnRequest {
                        turn_id: call.turn_id.clone(),
                        thread_settings: Default::default(),
                        trace: codex_otel::span_w3c_trace_context(&recovery),
                        cyber_access_program: None,
                    })
                    .await?,
                StartIfIdleSubmission::Started {
                    turn_id: call.turn_id.clone()
                }
            );
            wait_for_event(&test.codex, |event| {
                matches!(event, EventMsg::TurnComplete(_))
            })
            .await;
            reply.single_request();
            drop(recovery);
            let recovery = wait_for_span(&exported, |span| span.name == "recovery_request").await?;
            let admitted = wait_for_span(&exported, |span| {
                span.name == "codex.turn_input"
                    && span.span_context.trace_id() == recovery.span_context.trace_id()
            })
            .await?;
            assert_ne!(
                admitted.span_context.trace_id(),
                sampling.span_context.trace_id()
            );
            assert_eq!(
                (
                    attribute(&admitted.attributes, "conversation.id"),
                    attribute(&admitted.attributes, "turn.id"),
                ),
                (Some(thread_id.clone()), Some(call.turn_id.clone()))
            );
        }
    }
    test.codex.submit(Op::Shutdown).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ShutdownComplete)
    })
    .await;
    drop(initial);
    drop(steer);
    // ShutdownComplete can precede the final dispatch span's drop. Wait for
    // its exported ready event rather than racing the last task cleanup.
    let tool_span = wait_for_span(&exported, |span| {
        span.events.events.iter().any(|event| {
            event.name == "codex.tool_result_ready"
                && attribute(&event.attributes, "call_id").as_deref() == Some(call.call_id.as_str())
        })
    })
    .await?;
    let spans = exported.get_finished_spans()?;
    let steer = spans
        .iter()
        .find(|span| span.name == "steer_request")
        .expect("steer request span exported");
    let accepted = spans
        .iter()
        .find(|span| {
            span.name == "codex.turn_input"
                && span.span_context.trace_id() == steer.span_context.trace_id()
        })
        .expect("accepted input span exported in the steer trace");
    let blocking = spans
        .iter()
        .find(|span| {
            attribute(&span.attributes, "codex.turn.phase").as_deref() == Some("tool_blocking")
        })
        .expect("tool-blocking phase span exported");
    assert_ne!(
        accepted.span_context.trace_id(),
        sampling.span_context.trace_id()
    );
    assert_eq!(
        [
            &accepted.attributes,
            &sampling.attributes,
            &blocking.attributes
        ]
        .map(|attributes| (
            attribute(attributes, "conversation.id").expect("span has a conversation ID"),
            attribute(attributes, "turn.id").expect("span has a turn ID"),
        )),
        std::array::from_fn::<_, 3, _>(|_| (thread_id.clone(), call.turn_id.clone()))
    );
    assert!(sampling.end_time <= accepted.start_time);
    assert!(sampling.end_time <= blocking.start_time);
    assert!(accepted.end_time <= blocking.end_time);
    let ready = tool_span
        .events
        .events
        .iter()
        .find(|event| {
            event.name == "codex.tool_result_ready"
                && attribute(&event.attributes, "call_id").as_deref() == Some(call.call_id.as_str())
        })
        .expect("gated tool result-ready event exported");
    assert!(accepted.end_time <= ready.timestamp);
    assert!(ready.timestamp <= blocking.end_time);
    Ok(())
}
