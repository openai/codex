//! Exercises reviewer history safety at cancellation and budget boundaries.

use super::*;
use codex_guardian_context::ContextPresentation;
use codex_guardian_context::ContextProfile;
use codex_guardian_context::PlannedAction;
use codex_guardian_context::PlannedActionKind;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnAbortReason;
use std::sync::Arc;

fn required_context(text: String) -> ComposedContext {
    let action = PlannedAction {
        tool_descriptions: None,
        json: text,
        kind: PlannedActionKind::Command,
        reason: None,
    };
    let context = super::super::prompt::collect_guardian_context(
        &Vec::<ResponseItem>::new(),
        super::super::GUARDIAN_MAX_TOOL_ENTRY_TOKENS,
        &[],
        &[],
        Some(&action),
        /*permissions*/ None,
        /*node_repl*/ None,
    )
    .expect("collect required context");
    let transcript = ContextProfile::synchronous()
        .render_transcript(context.transcript_entries(), /*entry_number_offset*/ 0);
    context
        .compose(
            ContextPresentation::SyncFull {
                session_id: "test-parent",
            },
            transcript,
        )
        .expect("compose required context")
}

#[tokio::test]
async fn cancelled_startup_does_not_record_unselected_review_evidence() {
    let (session, turn, events) = crate::session::tests::make_session_and_context_with_rx().await;
    let context = required_context("unselected review evidence ".repeat(/*n*/ 10_000));
    let content = context.clone().into_user_inputs().unwrap();
    session
        .services
        .thread_extension_data
        .insert(PendingReviewContext(context));
    session
        .set_session_startup_prewarm(
            crate::session_startup_prewarm::SessionStartupPrewarmHandle::new(
                tokio::spawn(std::future::pending()),
                std::time::Instant::now(),
                crate::client::WEBSOCKET_CONNECT_TIMEOUT,
            ),
        )
        .await;
    session
        .spawn_task(
            turn,
            vec![TurnInput::UserInput {
                acceptance_order: None,
                content,
                client_id: None,
            }],
            crate::tasks::RegularTask::new(),
        )
        .await;
    let started = tokio::time::timeout(std::time::Duration::from_secs(5), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(started.msg, EventMsg::TurnStarted(_)));
    session.abort_all_tasks(TurnAbortReason::Interrupted).await;

    let history = session.clone_history().await;
    let recorded = history.raw_items().collect::<Vec<_>>();
    assert!(
        !serde_json::to_string(&recorded)
            .unwrap()
            .contains("unselected review evidence")
    );
    assert!(
        session
            .services
            .thread_extension_data
            .get::<PendingReviewContext>()
            .is_some()
    );
}

#[tokio::test]
async fn feasibility_rejects_required_evidence_inside_the_reserved_margin() {
    let (session, mut turn) = crate::session::tests::make_session_and_context().await;
    let context = required_context("required action".to_owned());
    let base = session.get_prompt_base_instructions().await;
    let prefix = codex_protocol::protocol::TruncationPolicy::Bytes(base.text.len()).token_budget();
    let model = Arc::make_mut(&mut Arc::make_mut(&mut turn.initial_settings).model_info);
    model.effective_context_window_percent = 100;
    Arc::make_mut(&mut turn.config).model_context_window =
        Some(i64::try_from(context.estimated_tokens() + prefix + 128).unwrap());
    session
        .services
        .thread_extension_data
        .insert(PendingReviewContext(context));

    assert!(check_pending(&session, &turn).await.is_err());
}

#[tokio::test]
async fn finalization_overflow_marks_the_reviewer_exhausted() {
    let (session, mut turn) = crate::session::tests::make_session_and_context().await;
    Arc::make_mut(&mut turn.config).model_context_window = Some(1);
    let session = Arc::new(session);
    let step = session
        .capture_step_context(Arc::new(turn), &tokio_util::sync::CancellationToken::new())
        .await
        .unwrap();
    let context = required_context("required action".to_owned());
    let mut input = vec![TurnInput::UserInput {
        acceptance_order: None,
        content: context.clone().into_user_inputs().unwrap(),
        client_id: None,
    }];
    session
        .services
        .thread_extension_data
        .insert(PendingReviewContext(context));

    assert!(
        finalize(&session, &step, &mut input, HistoryTruncation::Preserve)
            .await
            .is_err()
    );
    assert!(
        session
            .services
            .thread_extension_data
            .get::<super::super::request_budget::ExhaustedReviewBudget>()
            .is_some()
    );
}
