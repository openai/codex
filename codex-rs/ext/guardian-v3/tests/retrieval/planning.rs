//! Planning reads only the selected channels and respects both the board and history cutoffs.

use crate::evidence_board::EvidenceBoard;
use crate::evidence_board::routed_original;
use anyhow::Context;
use codex_guardian_v3::DeliveryPlan;
use codex_guardian_v3::DeliveryState;
use codex_message_board::AgentMessageBoard;
use codex_message_board::PageRequest;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::TruncationPolicy;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn plans_unique_originals_in_source_order_from_a_fixed_view() -> anyhow::Result<()> {
    let owner = ThreadId::new();
    let reviewer = ThreadId::new();
    let board = EvidenceBoard::new(owner, reviewer, &["a", "b", "unselected"]).await?;
    board.append(/*ordinal*/ 3, "a", "three").await?;
    board.append(/*ordinal*/ 1, "a", "one").await?;
    board.append(/*ordinal*/ 1, "b", "one").await?;
    board
        .append(/*ordinal*/ 0, "unselected", "unselected")
        .await?;
    let view = board.writer.read_view(reviewer).await?;
    board.host.clock.store(-1000, Ordering::SeqCst);
    board.append(/*ordinal*/ 4, "b", "after capture").await?;
    let cutoff = HistoryPosition {
        thread_id: owner,
        end_ordinal_exclusive: 4,
        end_byte_offset: 64 * 1024,
    };
    let selected = BTreeSet::from(["a".into(), "b".into()]);
    let mut state = DeliveryState::new(reviewer, owner.into());
    let reads = board.host.agent_path_calls.load(Ordering::SeqCst);
    let mut plan = state.plan(view.as_ref(), &selected, cutoff).await?;
    // One short ID page per selected channel completes the scan.
    assert_eq!(
        board.host.agent_path_calls.load(Ordering::SeqCst) - reads,
        2
    );
    assert_eq!(
        admit_batch(&mut plan).await?,
        json!({"sources": [
            routed_original(/*ordinal*/ 1, "one"),
            routed_original(/*ordinal*/ 3, "three"),
        ]}),
    );
    let later = board.writer.read_view(reviewer).await?;
    assert!(state.plan(later.as_ref(), &selected, cutoff).await.is_err());
    Ok(())
}

#[tokio::test]
async fn plans_across_storage_pages_in_reverse_insertion_order() -> anyhow::Result<()> {
    let owner = ThreadId::new();
    let reviewer = ThreadId::new();
    let board = EvidenceBoard::new(owner, reviewer, &["a"]).await?;
    let count = u64::from(PageRequest::default().limit.get()) + 1;
    for ordinal in (0..count).rev() {
        board.append(ordinal, "a", "next review").await?;
    }
    let view = board.writer.read_view(reviewer).await?;
    let selected = BTreeSet::from(["a".into()]);
    let cutoff = HistoryPosition {
        thread_id: owner,
        end_ordinal_exclusive: count,
        end_byte_offset: 64 * 1024,
    };
    let mut state = DeliveryState::new(reviewer, owner.into());
    let mut plan = state.plan(view.as_ref(), &selected, cutoff).await?;
    let reads = board.host.agent_path_calls.load(Ordering::SeqCst);
    let expected: Vec<_> = (0..count)
        .map(|ordinal| routed_original(ordinal, "next review"))
        .collect();
    assert_eq!(admit_batch(&mut plan).await?, json!({"sources": expected}));
    assert_eq!(
        board.host.agent_path_calls.load(Ordering::SeqCst) - reads,
        2
    );
    Ok(())
}

async fn admit_batch(plan: &mut DeliveryPlan<'_>) -> anyhow::Result<Value> {
    let mut batch = Vec::new();
    plan.deliver(TruncationPolicy::Bytes(64 * 1024), |items| {
        batch = items;
        std::future::ready(Ok(()))
    })
    .await?;
    let [ResponseItem::FunctionCallOutput { output, .. }] = batch.as_slice() else {
        anyhow::bail!("expected a single evidence batch");
    };
    let text = output.text_content().context("expected evidence text")?;
    Ok(serde_json::from_str(text)?)
}
