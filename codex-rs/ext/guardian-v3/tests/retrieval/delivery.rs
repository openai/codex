//! Whole originals enter a real retained conversation without scheduling inference.

use codex_analytics::GuardianReviewSessionKind;
use codex_core::guardian_review::prepare_review_prewarm;
use codex_extension_api::SessionIsolation;
use codex_extension_api::ToolPolicy;
use codex_guardian_v3::DeliveryState;
use codex_message_board::AgentMessageBoard;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TruncationPolicy;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::user_input::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses::*;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

use crate::evidence_board::EvidenceBoard;
use crate::evidence_board::routed_original;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delivers_ordered_originals_once_after_admission_and_keeps_older_channel_evidence()
-> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_model_info_override("test-gpt-5.1-codex", |model| {
            model.auto_review_model_override = Some(model.slug.clone());
            model.truncation_policy = TruncationPolicyConfig::bytes(/*limit*/ 64 * 1024);
        })
        .with_config(|config| {
            config.approvals_reviewer = ApprovalsReviewer::User;
            config.include_environment_context = false;
        })
        .build_with_auto_env(&server)
        .await?;
    let owner = test.session_configured.thread_id;
    let prepared = prepare_review_prewarm(&test.codex).await?;
    let (mut options, _state) = prepared
        .thread_options(GuardianReviewSessionKind::TrunkNew, /*snapshot*/ None)
        .await;
    options.config.base_instructions = Some("Inspect the supplied original evidence.".into());
    options.session_source = Some(SessionSource::Internal(InternalSessionSource::Guardian));
    options
        .thread_extension_init
        .insert(SessionIsolation::Isolated);
    options.thread_extension_init.insert(ToolPolicy {
        allowed_tools: Some(Vec::new()),
        ..Default::default()
    });
    let spawned = test.thread_manager.start_thread(options).await?;
    let reviewer = spawned.thread_id;
    let board = EvidenceBoard::new(owner, reviewer, &["a", "b", "older", "large"]).await?;
    let large = "é".repeat(20_001);
    // The housekeeper can route one source to several channels and post out of source order.
    for (ordinal, channel, output) in [
        (3, "a", "two"),
        (2, "b", "one"),
        (2, "a", "one"),
        (1, "older", "older"),
        (4, "large", large.as_str()),
    ] {
        board.append(ordinal, channel, output).await?;
    }
    let evidence = board.writer.read_view(reviewer).await?;
    let cutoff = HistoryPosition {
        thread_id: owner,
        end_ordinal_exclusive: 6,
        end_byte_offset: 64 * 1024,
    };
    let mut state = DeliveryState::new(reviewer, owner.into());
    let before_admission = state.clone();
    let selected = BTreeSet::from(["a".into(), "b".into()]);
    let mut plan = state.plan(evidence.as_ref(), &selected, cutoff).await?;
    let small = TruncationPolicy::Bytes(160);
    assert!(
        plan.deliver(small, |_| async { anyhow::bail!("admission unavailable") })
            .await
            .is_err()
    );
    drop(plan);
    assert_eq!(state, before_admission);
    let mut plan = state.plan(evidence.as_ref(), &selected, cutoff).await?;
    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("review", "Evidence received."),
            ev_completed("review"),
        ]),
    )
    .await;
    let reads = board.host.agent_path_calls.load(Ordering::SeqCst);
    for _ in 0..2 {
        assert_eq!(
            plan.deliver(small, |items| async {
                Ok(spawned.thread.inject_response_items(items).await?)
            })
            .await?,
            1
        );
    }
    // Two admissions share one page of board reads.
    assert_eq!(
        board.host.agent_path_calls.load(Ordering::SeqCst) - reads,
        1
    );
    assert!(response.requests().is_empty());
    spawned
        .thread
        .start_turn_if_idle(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Assess.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&spawned.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    insta::assert_snapshot!(context_snapshot::format_request_history_snapshot(
        "Selected originals arrive once in source order before a later reviewer turn.", &response.requests(),
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    ), @r###"
    Scenario: Selected originals arrive once in source order before a later reviewer turn.

    ## Window 1
    -- request 1 (turn) --
    00:message/developer:
        <MODEL_INSTRUCTIONS>
    01:message/developer:
        <PERMISSIONS_INSTRUCTIONS>
    02:function_call_output/guardian.guardian_original_evidence:{"sources":[{"id":{"ordinal":2},"original":{"type":"response_item","payload":{"type":"function_call_output","output":"one"}}}]}
    03:function_call_output/guardian.guardian_original_evidence:{"sources":[{"id":{"ordinal":3},"original":{"type":"response_item","payload":{"type":"function_call_output","output":"two"}}}]}
    04:message/user:
        Assess.
    "###);
    // A later subscription can add an older source without resetting delivered progress.
    let mut older = state
        .plan(evidence.as_ref(), &BTreeSet::from(["older".into()]), cutoff)
        .await?;
    assert_eq!(
        older
            .deliver(small, |items| async {
                Ok(spawned.thread.inject_response_items(items).await?)
            })
            .await?,
        1
    );
    let checkpoint = state.clone();
    // A whole source larger than the admission budget can be retried with more room.
    let mut long = state
        .plan(evidence.as_ref(), &BTreeSet::from(["large".into()]), cutoff)
        .await?;
    assert!(
        long.deliver(small, |_| async {
            panic!("oversized originals must not reach admission")
        })
        .await
        .is_err()
    );
    drop(long);
    assert_eq!(state, checkpoint);
    let mut long = state
        .plan(evidence.as_ref(), &BTreeSet::from(["large".into()]), cutoff)
        .await?;
    assert_eq!(
        long.deliver(TruncationPolicy::Bytes(64 * 1024), |items| async {
            Ok(spawned.thread.inject_response_items(items).await?)
        })
        .await?,
        1
    );
    // A backdated post and a newly routed copy both arrive after the board capture.
    board.host.clock.store(-1000, Ordering::SeqCst);
    for (ordinal, channel, output) in [(5, "a", "late"), (3, "b", "two")] {
        board.append(ordinal, channel, output).await?;
    }
    let mut repeated = state.plan(evidence.as_ref(), &selected, cutoff).await?;
    assert_eq!(
        repeated
            .deliver(small, |_| async {
                panic!("the old view has no additional evidence")
            })
            .await?,
        0
    );
    let changed = board.writer.read_view(reviewer).await?;
    let mut fresh = state.plan(changed.as_ref(), &selected, cutoff).await?;
    assert_eq!(
        fresh
            .deliver(small, |items| async {
                Ok(spawned.thread.inject_response_items(items).await?)
            })
            .await?,
        1
    );
    assert_eq!(
        fresh
            .deliver(small, |_| async {
                panic!("the duplicate source must not be admitted again")
            })
            .await?,
        0
    );

    let history = spawned.thread.conversation_history_snapshot().await;
    let mut delivered = Vec::new();
    for item in history.items() {
        if let ResponseItem::FunctionCallOutput {
            name: Some(name),
            output,
            ..
        } = item
            && name == "guardian_original_evidence"
        {
            let text = output
                .text_content()
                .ok_or_else(|| anyhow::anyhow!("expected evidence text"))?;
            delivered.push(serde_json::from_str::<serde_json::Value>(text)?);
        }
    }
    let expected = [
        (2, "one"),
        (3, "two"),
        (1, "older"),
        (4, large.as_str()),
        (5, "late"),
    ]
    .map(|(ordinal, output)| json!({"sources":[routed_original(ordinal, output)]}));
    assert_eq!(delivered.as_slice(), expected.as_slice());
    assert_eq!(response.requests().len(), 1);

    spawned.thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
