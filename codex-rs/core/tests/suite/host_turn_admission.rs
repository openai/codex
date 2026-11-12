//! Host capacity is acquired by the ordinary task startup path, including on a root.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_core::AgentExecutionGuard;
use codex_core::AgentRef;
use codex_core::AgentSessionReader;
use codex_core::AgentTurnAdmission;
use codex_core::AgentTurnAdmissionRequest;
use codex_core::CodexThread;
use codex_core::SuspendTurnOutcome;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::input_text;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::ReviewTarget;
use codex_protocol::user_input::UserInput;
use codex_thread_store::LoadThreadHistoryParams;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_successful_turn;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use wiremock::MockServer;

type Decision = oneshot::Sender<CodexResult<AgentExecutionGuard>>;
type Attempt = (AgentTurnAdmissionRequest, Decision);
struct HostAdmission(mpsc::UnboundedSender<Attempt>);

impl AgentTurnAdmission for HostAdmission {
    fn admit_turn(
        &self,
        request: AgentTurnAdmissionRequest,
    ) -> BoxFuture<'_, CodexResult<AgentExecutionGuard>> {
        Box::pin(async move {
            let (reply, decision) = oneshot::channel();
            self.0.send((request, reply)).expect("admission receiver");
            decision.await.expect("host decision")
        })
    }
}

#[derive(Default)]
struct PendingRosterRead(Notify);

impl AgentSessionReader for PendingRosterRead {
    fn root(&self) -> BoxFuture<'_, CodexResult<AgentRef>> {
        unreachable!("roster rendering reads children")
    }

    fn parent(&self) -> BoxFuture<'_, CodexResult<Option<AgentRef>>> {
        unreachable!("roster rendering reads children")
    }

    fn children(&self) -> BoxFuture<'_, CodexResult<Vec<AgentRef>>> {
        Box::pin(async {
            self.0.notify_one();
            std::future::pending().await
        })
    }
}

async fn root_with_admission(
    server: &MockServer,
    mut builder: TestCodexBuilder,
    session_reader: Option<Arc<dyn AgentSessionReader>>,
) -> Result<(TestCodex, mpsc::UnboundedReceiver<Attempt>)> {
    let mut test = builder.build_with_auto_env(server).await?;
    let mut options = test.start_thread_options().await;
    let (attempts, requests) = mpsc::unbounded_channel();
    let admission: Arc<dyn AgentTurnAdmission> = Arc::new(HostAdmission(attempts));
    options
        .thread_extension_init
        .insert(codex_core::AgentRuntimeOwnership::Host);
    options.thread_extension_init.insert(admission);
    if let Some(session_reader) = session_reader {
        options.thread_extension_init.insert(session_reader);
    }
    options.dynamic_tools = vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
        name: "hold_turn".into(),
        description: "Wait for the test to release this turn.".into(),
        input_schema: json!({"type": "object", "properties": {}}),
        defer_loading: false,
    })];
    test.codex.shutdown_and_wait().await?;
    let root = test.thread_manager.start_thread(options).await?;
    test.codex = root.thread;
    test.session_configured = root.session_configured;
    Ok((test, requests))
}

async fn submit(thread: &CodexThread, text: &str) -> CodexResult<TurnInputSubmission> {
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: text.into(),
            text_elements: vec![],
        }]))
        .await
}

async fn start(
    test: &TestCodex,
    attempts: &mut mpsc::UnboundedReceiver<Attempt>,
    text: &str,
) -> Result<(String, Decision)> {
    let submitted =
        tokio::time::timeout(Duration::from_secs(10), submit(&test.codex, text)).await??;
    let TurnInputSubmission::Started { turn_id, .. } = submitted else {
        anyhow::bail!("expected a new turn, got {submitted:?}");
    };
    let (request, decision) = tokio::time::timeout(Duration::from_secs(10), attempts.recv())
        .await?
        .context("admission provider dropped")?;
    assert_eq!(request.thread_id, test.session_configured.thread_id);
    assert_eq!(request.turn_id, turn_id);
    Ok((turn_id, decision))
}

fn deny(decision: Decision) {
    let error = CodexErr::new(CodexErrorDetails::AgentLimitReached { max_threads: 1 });
    assert!(decision.send(Err(error)).is_ok());
}

fn is_terminal(event: &EventMsg) -> bool {
    matches!(event, EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_))
}

fn assert_denied(event: &EventMsg, turn_id: &str) {
    assert!(
        matches!(event, EventMsg::TurnComplete(done) if done.turn_id == turn_id && done.error.is_some()),
        "expected failed turn {turn_id}, got {event:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn root_denial_and_interruption_retain_input_and_grants_span_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let (test, mut attempts) =
        root_with_admission(&server, test_codex(), /*session_reader*/ None).await?;
    let (turn_id, decision) = start(&test, &mut attempts, "Denied initial prompt.").await?;
    assert!(
        matches!(submit(&test.codex, "Steered while pending.").await?,
        TurnInputSubmission::Steered { turn_id: steered, .. } if steered == turn_id)
    );
    deny(decision);
    let mut recorded = Vec::new();
    wait_for_event(&test.codex, |event| {
        if let EventMsg::ItemCompleted(done) = event
            && let TurnItem::UserMessage(item) = &done.item
        {
            recorded.push(item.message());
        }
        matches!(event, EventMsg::Error(_))
    })
    .await;
    assert_eq!(
        recorded,
        ["Denied initial prompt.", "Steered while pending."]
    );

    // Retry on the first visible error, while the failed turn may still be finishing.
    let (retry_id, pending) = start(&test, &mut attempts, "Interrupted before admission.").await?;
    assert_denied(&wait_for_event(&test.codex, is_terminal).await, &turn_id);
    test.codex.submit(Op::Interrupt).await?;
    let event = wait_for_event(&test.codex, is_terminal).await;
    assert!(
        matches!(&event, EventMsg::TurnAborted(done) if done.turn_id.as_ref() == Some(&retry_id)),
        "expected interrupted turn {retry_id}, got {event:?}"
    );
    assert!(pending.is_closed());

    responses::mount_function_call_agent_response(&server, "hold", "{}", "hold_turn").await;
    let (_, grant) = start(&test, &mut attempts, "Continue the work.").await?;
    let released = CancellationToken::new();
    assert!(
        grant
            .send(Ok(AgentExecutionGuard::new(released.clone().drop_guard())))
            .is_ok()
    );
    wait_for_event(
        &test.codex,
        |event| matches!(event, EventMsg::DynamicToolCallRequest(call) if call.call_id == "hold"),
    )
    .await;
    assert!(!released.is_cancelled());
    test.codex
        .submit(Op::DynamicToolResponse {
            id: "hold".into(),
            response: DynamicToolResponse {
                content_items: vec![],
                success: true,
            },
        })
        .await?;
    wait_for_successful_turn(&test.codex).await;
    assert!(released.is_cancelled());
    insta::assert_snapshot!(
        "retained_input_after_admission",
        context_snapshot::format_request_history_snapshot(
            "The admitted turn receives input retained after denial and interruption, then completes its tool call.",
            &responses::received_responses_requests(&server).await,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_before_admission_skips_submit_hook_and_uninspected_prompt() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let builder = test_codex()
        .with_pre_build_hook(|home| {
            let hooks = json!({"hooks": {"UserPromptSubmit": [{"hooks": [
                {"type": "command", "command": "echo {}"}
            ]}]}});
            std::fs::write(home.join("hooks.json"), hooks.to_string()).unwrap();
        })
        .with_config(|config| {
            trust_discovered_hooks(config);
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("enable multi-agent V2");
        });
    let (test, mut attempts) =
        root_with_admission(&server, builder, /*session_reader*/ None).await?;
    let interrupted = "Interrupted before inspection.";
    let (turn_id, _pending) = start(&test, &mut attempts, interrupted).await?;
    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::HookStarted(_)), "{event:?}");
        matches!(event, EventMsg::TurnAborted(done) if done.turn_id.as_ref() == Some(&turn_id))
    })
    .await;

    let model = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("done")]),
    )
    .await;
    let (turn_id, grant) = start(&test, &mut attempts, "Continue.").await?;
    assert!(grant.send(Ok(AgentExecutionGuard::new(()))).is_ok());
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::HookStarted(start) if start.turn_id.as_ref() == Some(&turn_id))
    })
    .await;
    wait_for_successful_turn(&test.codex).await;
    assert_eq!(
        test.codex.multi_agent_version(),
        Some(MultiAgentVersion::V2)
    );
    let input = model.single_request().message_input_texts("user");
    assert!(input.iter().any(|text| text == "Continue."));
    assert!(input.iter().all(|text| text != interrupted));
    assert!(
        input
            .iter()
            .any(|text| text.contains("<environment_context>"))
    );
    assert!(input.iter().all(|text| !text.contains("<subagents>")));
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suspending_root_during_admission_preserves_the_recoverable_prompt() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let (test, mut attempts) =
        root_with_admission(&server, test_codex(), /*session_reader*/ None).await?;
    let prompt = "Save this prompt across handoff.";
    let (turn_id, pending) = start(&test, &mut attempts, prompt).await?;
    suspend_and_check_recoverable_prompt(&test, &server, turn_id, prompt).await?;
    assert!(pending.is_closed());
    Ok(())
}

#[tokio::test]
async fn tree_suspension_leaves_an_ephemeral_turn_running() -> Result<()> {
    let server = responses::start_mock_server().await;
    let reply = responses::mount_sse_once(&server, responses::sse_completed("done")).await;
    let builder = test_codex().with_config(|config| config.ephemeral = true);
    let (test, mut attempts) =
        root_with_admission(&server, builder, /*session_reader*/ None).await?;
    let prompt = "Keep working after the suspension attempt.";
    let (_, grant) = start(&test, &mut attempts, prompt).await?;
    assert_eq!(
        test.thread_manager.suspend_agent_tree(&test.codex).await?,
        None
    );
    assert!(grant.send(Ok(AgentExecutionGuard::new(()))).is_ok());
    wait_for_successful_turn(&test.codex).await;
    assert!(reply.single_request().body_contains_text(prompt));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suspending_root_during_roster_read_preserves_the_recoverable_prompt() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::MultiAgentV2)
            .expect("enable multi-agent V2");
    });
    let roster = Arc::new(PendingRosterRead::default());
    let (test, mut attempts) = root_with_admission(&server, builder, Some(roster.clone())).await?;
    let prompt = "Save this admitted prompt across handoff.";
    let (turn_id, grant) = start(&test, &mut attempts, prompt).await?;
    assert!(grant.send(Ok(AgentExecutionGuard::new(()))).is_ok());
    tokio::time::timeout(Duration::from_secs(10), roster.0.notified()).await?;
    suspend_and_check_recoverable_prompt(&test, &server, turn_id, prompt).await
}

async fn suspend_and_check_recoverable_prompt(
    test: &TestCodex,
    server: &MockServer,
    turn_id: String,
    prompt: &str,
) -> Result<()> {
    assert_eq!(
        test.codex.suspend_turn_and_shutdown().await?,
        SuspendTurnOutcome::Suspended { turn_id }
    );
    let history = test
        .thread_store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id: test.session_configured.thread_id,
            include_archived: false,
        })
        .await?;
    let expected_content = vec![input_text(prompt)];
    let saved_prompts = history
        .items
        .iter()
        .filter(|entry| {
            let RolloutItem::ResponseItem(item) = entry else {
                return false;
            };
            matches!(&item.item, ResponseItem::Message { role, content, .. }
            if role == "user" && content == &expected_content)
        })
        .count();
    assert_eq!(saved_prompts, 1);
    assert!(
        responses::received_responses_requests(server)
            .await
            .is_empty()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn denied_review_exits_review_mode() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let (test, mut attempts) =
        root_with_admission(&server, test_codex(), /*session_reader*/ None).await?;
    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Review this change.".into(),
                },
                user_facing_hint: None,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ItemCompleted(done)
        if matches!(done.item, TurnItem::EnteredReviewMode(_)))
    })
    .await;
    let (review, decision) = tokio::time::timeout(Duration::from_secs(10), attempts.recv())
        .await?
        .context("review admission provider dropped")?;
    deny(decision);
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ItemCompleted(done)
        if matches!(done.item, TurnItem::ExitedReviewMode(_)))
    })
    .await;
    assert_denied(
        &wait_for_event(&test.codex, is_terminal).await,
        &review.turn_id,
    );
    assert!(
        responses::received_responses_requests(&server)
            .await
            .is_empty()
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
