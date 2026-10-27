//! Exercises reviewer history safety at cancellation and budget boundaries.

use super::*;
use codex_guardian_context::ContextPresentation;
use codex_guardian_context::ContextProfile;
use codex_guardian_context::PlannedAction;
use codex_guardian_context::PlannedActionKind;
use codex_guardian_context::TruncationObservation;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnAbortReason;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use test_case::test_case;

#[tokio::test]
#[test_case(128_000; "accepted_input")]
#[test_case(1; "rejected_input")]
async fn finalization_exports_loss_totals_only_for_accepted_input(window: i64) {
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-core",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )
    .unwrap();
    let (mut session, mut turn) = crate::session::tests::make_session_and_context().await;
    session.services.session_telemetry = session
        .services
        .session_telemetry
        .clone()
        .with_metrics(metrics.clone());
    Arc::make_mut(&mut turn.config).model_context_window = Some(window);
    let session = Arc::new(session);
    let step = session
        .capture_step_context(Arc::new(turn), &tokio_util::sync::CancellationToken::new())
        .await
        .unwrap();
    let mut context = required_context("required action".to_owned());
    // Composition and final selection can reduce the same component multiple times.
    context.truncations = [(256, 100), (64, 40), (16, 0)]
        .into_iter()
        .map(|(original_bytes, retained_bytes)| TruncationObservation {
            component: "conversation_transcript",
            original_bytes,
            retained_bytes,
        })
        .collect();
    let mut input = vec![TurnInput::UserInput {
        metadata: Default::default(),
        content: context.clone().into_user_inputs().unwrap(),
        client_id: None,
    }];
    session
        .services
        .thread_extension_data
        .insert(PendingReviewContext(context));
    assert_eq!(
        finalize(&session, &step, &mut input, HistoryTruncation::Preserve)
            .await
            .is_ok(),
        window > 1,
    );
    // A second finalization has no pending input and must not emit the loss twice.
    if window > 1 {
        finalize(&session, &step, &mut input, HistoryTruncation::Preserve)
            .await
            .unwrap();
    }
    let snapshot = metrics.snapshot().unwrap();
    let mut counts = BTreeMap::new();
    let mut bytes = BTreeMap::new();
    for metric in snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
    {
        match metric.name() {
            codex_otel::GUARDIAN_REVIEW_TRUNCATION_METRIC => {
                let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() else {
                    panic!("expected loss counter");
                };
                for point in sum.data_points() {
                    let tags = point
                        .attributes()
                        .filter(|attr| matches!(attr.key.as_str(), "component" | "disposition"))
                        .map(|attr| (attr.key.as_str().to_owned(), attr.value.to_string()))
                        .collect::<BTreeMap<_, _>>();
                    counts.insert(tags, point.value());
                }
            }
            codex_otel::GUARDIAN_REVIEW_TRUNCATION_BYTES_METRIC => {
                let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
                    panic!("expected loss histogram");
                };
                for point in histogram.data_points() {
                    let tags = point
                        .attributes()
                        .filter(|attr| {
                            matches!(
                                attr.key.as_str(),
                                "component" | "disposition" | "measurement"
                            )
                        })
                        .map(|attr| (attr.key.as_str().to_owned(), attr.value.to_string()))
                        .collect::<BTreeMap<_, _>>();
                    bytes.insert(tags, (point.count(), point.sum()));
                }
            }
            _ => {}
        }
    }
    let mut expected_counts = BTreeMap::new();
    let mut expected_bytes = BTreeMap::new();
    if window > 1 {
        for (disposition, original, retained) in
            [("omitted", 16.0, 0.0), ("truncated", 320.0, 140.0)]
        {
            let tags = BTreeMap::from([
                ("component".to_owned(), "conversation_transcript".to_owned()),
                ("disposition".to_owned(), disposition.to_owned()),
            ]);
            expected_counts.insert(tags.clone(), 1);
            for (measurement, value) in [
                ("original", original),
                ("retained", retained),
                ("omitted", original - retained),
            ] {
                let mut tags = tags.clone();
                tags.insert("measurement".to_owned(), measurement.to_owned());
                expected_bytes.insert(tags, (1, value));
            }
        }
    }
    assert_eq!((counts, bytes), (expected_counts, expected_bytes));
}

fn required_context(text: String) -> ComposedContext {
    let action = PlannedAction {
        tool_descriptions: None,
        kind: PlannedActionKind::Command { json: text },
        reason: None,
    };
    let context = super::super::prompt::collect_guardian_context(
        &Vec::<ResponseItem>::new(),
        &ContextProfile::synchronous(),
        /*root_conversation_candidates*/ None,
        &[],
        Some(&action),
        /*permissions*/ None,
        /*node_repl*/ None,
    )
    .expect("collect required context");
    let transcript = ContextProfile::synchronous()
        .prepare_transcript(context.transcript_entries(), /*entry_number_offset*/ 0);
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
            crate::session::startup_prewarm::SessionStartupPrewarmHandle::new(
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
                metadata: Default::default(),
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
        metadata: Default::default(),
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

#[tokio::test]
async fn compacted_review_restores_originals_once_and_persists_the_request_prefix() {
    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let session = Arc::new(session);
    let step = session
        .capture_step_context(Arc::new(turn), &tokio_util::sync::CancellationToken::new())
        .await
        .unwrap();
    session
        .record_user_prompt_and_emit_turn_item(
            &step.turn,
            &step.settings.model_info,
            &[codex_protocol::user_input::UserInput::Text {
                text: "Do not publish.".to_owned(),
                text_elements: vec![],
            }],
            /*client_id*/ None,
            crate::session::UserInputMetadata {
                acceptance_order: Some(0),
                ..Default::default()
            },
            codex_thread_store::PersistContext::Standard,
        )
        .await;
    let context = super::super::prompt::build_guardian_prompt_items(
        &session,
        /*retry_reason*/ None,
        super::super::GuardianApprovalRequest::RequestPermissions {
            id: "review".to_owned(),
            environment_id: "local".to_owned(),
            turn_id: "parent".to_owned(),
            reason: None,
            permissions: Default::default(),
        },
        super::super::prompt::GuardianPromptMode::Full,
    )
    .await
    .unwrap()
    .context;
    step.turn.extension_data.insert(RetainedReviewContext {
        context: context.retained_instructions(),
        history_version: session.clone_history().await.history_version(),
    });
    // Simulate a reviewer compaction that discarded the earlier user-message block.
    session
        .replace_history(vec![], /*reference_context_item*/ None)
        .await;
    let mut prompt = build_prompt(
        vec![],
        &step,
        session.get_prompt_base_instructions().await,
        /*incremental_tools*/ false,
    );
    let metadata = session
        .responses_metadata(&step, CodexResponsesRequestKind::Turn)
        .await;
    super::super::request_budget::prepare_prompt(&session, &mut prompt, &step, &metadata)
        .await
        .unwrap();
    assert!(
        serde_json::to_string(&prompt.input)
            .unwrap()
            .contains("Do not publish.")
    );
    let persisted = session
        .clone_history()
        .await
        .for_prompt(
            &step.settings.model_info.input_modalities,
            session.services.image_store.as_ref(),
        )
        .await;
    assert_eq!(prompt.input, persisted);
    assert!(
        persisted
            .iter()
            .all(crate::context::is_guardian_context_message)
    );
    assert!(
        !persisted
            .iter()
            .any(crate::context::is_user_authorization_message)
    );
    super::super::request_budget::prepare_prompt(&session, &mut prompt, &step, &metadata)
        .await
        .unwrap();
    assert_eq!(prompt.input, persisted);
}
