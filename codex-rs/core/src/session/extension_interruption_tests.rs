//! Regression coverage for conditional interruption and detached abort tracing.
#![cfg(test)]

use super::*;
use crate::session::tests::HeldStepTask;
use crate::session::tests::make_session_and_context_with_rx;
use crate::session::tests::make_thread_for_session;
use crate::state::TaskKind;
use codex_protocol::protocol::SessionSource;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::trace::InMemorySpanExporter;
use opentelemetry_sdk::trace::SdkTracerProvider;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::sync::Notify;
use tracing::Instrument;
use tracing_subscriber::prelude::*;

#[tokio::test]
async fn interrupt_with_warning_exports_turn_profile_under_caller_span() -> anyhow::Result<()> {
    let exported = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exported.clone())
        .build();
    let subscriber = tracing_subscriber::registry().with(
        tracing_opentelemetry::layer()
            .with_tracer(provider.tracer("guardian-interrupt-test"))
            .with_filter(tracing_subscriber::filter::filter_fn(
                codex_otel::OtelProvider::trace_export_filter,
            )),
    );
    let _interest_cache_guard =
        tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);
    let (session, turn, events) = make_session_and_context_with_rx().await;
    session
        .spawn_task(
            Arc::clone(&turn),
            Vec::new(),
            HeldStepTask {
                kind: TaskKind::Regular,
                finish: Arc::new(Notify::new()),
            },
        )
        .await;
    turn.turn_timing_state
        .mark_turn_started(std::time::Instant::now())
        .await;
    drop(turn.turn_timing_state.begin_sampling());
    turn.turn_timing_state.record_sampling_retry();
    // Guardian uses this detached abort path rather than submitting Op::Interrupt.
    session
        .interrupt_turn_with_warning(
            &turn.sub_id,
            EventMsg::Warning(codex_protocol::protocol::WarningEvent {
                message: "Guardian denial limit reached".into(),
            }),
            /*error*/ None,
        )
        .instrument(tracing::info_span!("guardian_review"))
        .await;
    tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
        while !matches!(events.recv().await?.msg, EventMsg::TurnAborted(_)) {}
        anyhow::Ok(())
    })
    .await??;

    let profile = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
        loop {
            let spans = exported.get_finished_spans()?;
            for span in spans {
                if span.name == "guardian_review"
                    && let Some(event) = span
                        .events
                        .iter()
                        .find(|event| event.name == "codex.turn_profile")
                {
                    return anyhow::Ok(event.clone());
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await??;
    let attributes: std::collections::BTreeMap<_, _> = profile
        .attributes
        .iter()
        .filter(|attribute| {
            matches!(
                attribute.key.as_str(),
                "conversation.id" | "turn_id" | "sampling_request_count" | "sampling_retry_count"
            )
        })
        .map(|attribute| (attribute.key.as_str(), attribute.value.to_string()))
        .collect();
    assert_eq!(
        attributes,
        std::collections::BTreeMap::from([
            ("conversation.id", session.thread_id.to_string()),
            ("turn_id", turn.sub_id.clone()),
            ("sampling_request_count", "1".into()),
            ("sampling_retry_count", "1".into()),
        ])
    );
    Ok(())
}

#[tokio::test]
async fn interrupt_if_no_pending_input_handles_cancelled_submission() {
    let (session, turn_context, rx_event) = make_session_and_context_with_rx().await;
    session
        .spawn_task(
            Arc::clone(&turn_context),
            Vec::new(),
            HeldStepTask {
                kind: TaskKind::Regular,
                finish: Arc::new(Notify::new()),
            },
        )
        .await;
    let cancellation_token = session
        .active_turn
        .lock()
        .await
        .as_ref()
        .expect("active turn")
        .task
        .as_ref()
        .expect("active task")
        .cancellation_token
        .clone();
    let (thread, rx_sub) = make_thread_for_session(
        Arc::clone(&session),
        &turn_context,
        rx_event,
        SessionSource::Cli,
    )
    .await;
    thread.submit(Op::Interrupt).await.expect("fill the queue");

    let interrupt = thread.interrupt_if_no_pending_input(&turn_context.sub_id);
    tokio::pin!(interrupt);
    assert!(futures::poll!(interrupt.as_mut()).is_pending());
    // The pending submission still owns its reply sender when cancellation wins.
    cancellation_token.cancel();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 1), interrupt)
            .await
            .expect("cancellation should unblock the submission")
            .expect("cancelling a submission does not mean the agent died"),
        false,
    );
    assert!(!rx_sub.is_closed());
    assert_eq!(rx_sub.len(), 1);
    session.abort_all_tasks(TurnAbortReason::Interrupted).await;
}
