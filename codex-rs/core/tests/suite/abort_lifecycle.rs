//! Verifies abort callbacks finish exactly once before the terminal event is published.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::TurnAbortInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use test_case::test_case;
use tokio::sync::Notify;

#[derive(Default)]
struct AbortGate {
    entered: Notify,
    release: Notify,
    completed: Mutex<Vec<(String, TurnAbortReason)>>,
}

impl TurnLifecycleContributor for AbortGate {
    fn on_turn_abort<'a>(&'a self, input: TurnAbortInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            self.entered.notify_one();
            self.release.notified().await;
            self.completed
                .lock()
                .expect("abort callback records")
                .push((input.turn_store.level_id().to_string(), input.reason));
        })
    }
}

#[derive(Clone, Copy)]
enum AbortOperation {
    Interrupt,
    ConditionalInterrupt,
    Shutdown,
}

#[test_case(AbortOperation::Interrupt; "interrupt")]
#[test_case(AbortOperation::ConditionalInterrupt; "conditional_interrupt")]
#[test_case(AbortOperation::Shutdown; "shutdown")]
#[tokio::test]
async fn abort_lifecycle_finishes_before_terminal_event(
    operation: AbortOperation,
) -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let request = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("response"),
            responses::ev_function_call("gate-call", "gate", "{}"),
            responses::ev_completed("response"),
        ]),
    )
    .await;
    let gate = Arc::new(AbortGate::default());
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.turn_lifecycle_contributor(gate.clone());
    let mut builder = test_codex().with_extensions(Arc::new(extensions.build()));
    let test = builder.build_with_auto_env(&server).await?;
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![test.executor_environment().selection().clone()]),
            dynamic_tools: vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
                name: "gate".to_string(),
                description: "Wait for the host.".to_string(),
                input_schema: serde_json::json!({"type": "object", "properties": {}}),
                defer_loading: false,
            })],
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?
        .thread;
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Call gate.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let EventMsg::DynamicToolCallRequest(call) = wait_for_event(&thread, |event| {
        matches!(event, EventMsg::DynamicToolCallRequest(_))
    })
    .await
    else {
        unreachable!();
    };
    request.single_request();

    match operation {
        AbortOperation::Interrupt => {
            thread.submit(Op::Interrupt).await?;
        }
        AbortOperation::ConditionalInterrupt => {
            let (reply, received) = tokio::sync::oneshot::channel();
            thread
                .submit(Op::InterruptIfNoPendingInput {
                    turn_id: call.turn_id.clone(),
                    reply,
                })
                .await?;
            assert!(received.await?);
        }
        AbortOperation::Shutdown => {
            thread.submit(Op::Shutdown).await?;
        }
    }
    tokio::time::timeout(Duration::from_secs(10), gate.entered.notified()).await?;
    // Keep the callback suspended: scheduling alone must not hide an early event.
    let early_event = tokio::time::timeout(
        Duration::from_millis(100),
        wait_for_event(&thread, |event| matches!(event, EventMsg::TurnAborted(_))),
    )
    .await;
    gate.release.notify_one();
    assert!(
        early_event.is_err(),
        "TurnAborted arrived before its lifecycle callback finished"
    );
    let EventMsg::TurnAborted(aborted) =
        wait_for_event(&thread, |event| matches!(event, EventMsg::TurnAborted(_))).await
    else {
        unreachable!();
    };
    assert_eq!(aborted.turn_id, Some(call.turn_id.clone()));
    assert_eq!(aborted.reason, TurnAbortReason::Interrupted);
    if !matches!(operation, AbortOperation::Shutdown) {
        thread.submit(Op::Shutdown).await?;
    }
    wait_for_event(&thread, |event| matches!(event, EventMsg::ShutdownComplete)).await;
    assert_eq!(
        *gate.completed.lock().expect("abort callback records"),
        vec![(call.turn_id, TurnAbortReason::Interrupted)],
    );
    Ok(())
}
