use codex_protocol::ThreadId;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use pretty_assertions::assert_eq;

use super::RolloutSizeTotals;
use super::ThreadTotals;
use super::add_totals;
use super::is_thread_sampled;
use super::measure_and_filter_rollout_items;
use super::measure_rollout_items;
use super::thread_totals_with_persisted_history;

fn retained_message(text: &str) -> RolloutItem {
    RolloutItem::ResponseItem(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    })
}

#[test]
fn thread_sampling_is_stable_and_selects_whole_threads() {
    let mut sampled = None;
    let mut unsampled = None;
    for value in 0..10_000_u128 {
        let thread_id = ThreadId::from_string(&format!("00000000-0000-0000-0000-{value:012x}"))
            .expect("valid thread id");
        if is_thread_sampled(thread_id) {
            sampled.get_or_insert(thread_id);
        } else {
            unsampled.get_or_insert(thread_id);
        }
        if sampled.is_some() && unsampled.is_some() {
            break;
        }
    }

    let sampled = sampled.expect("at least one sampled thread");
    let unsampled = unsampled.expect("at least one unsampled thread");
    assert!(is_thread_sampled(sampled));
    assert!(is_thread_sampled(sampled));
    assert!(!is_thread_sampled(unsampled));
    assert!(!is_thread_sampled(unsampled));
}

#[test]
fn mixed_batch_reports_exact_policy_counts_and_bytes() {
    let kept = retained_message("hello");
    let dropped = RolloutItem::ResponseItem(ResponseItem::Other);
    let items = vec![kept.clone(), dropped.clone()];

    let (persisted, measurement) = measure_and_filter_rollout_items(&items);
    let kept_bytes = serde_json::to_vec(&kept)
        .expect("serialize kept item")
        .len() as u64;
    let dropped_bytes = serde_json::to_vec(&dropped)
        .expect("serialize dropped item")
        .len() as u64;

    assert_eq!(
        serde_json::to_value(persisted).expect("serialize persisted items"),
        serde_json::to_value([kept]).expect("serialize expected items")
    );
    assert_eq!(measurement.pre_filter.items, 2);
    assert_eq!(
        measurement.pre_filter.payload_bytes,
        kept_bytes + dropped_bytes
    );
    assert_eq!(measurement.post_filter.items, 1);
    assert_eq!(measurement.post_filter.payload_bytes, kept_bytes);
    assert_eq!(measurement.items[0].payload_bytes, Some(kept_bytes));
    assert_eq!(measurement.items[1].payload_bytes, Some(dropped_bytes));
    assert_eq!(measurement.items[0].rollout_item_type, "response.message");
    assert_eq!(measurement.items[1].rollout_item_type, "response.other");
}

#[test]
fn retained_items_are_byte_identical() {
    let item = retained_message("a moderately sized payload");
    let (persisted, measurement) = measure_and_filter_rollout_items(std::slice::from_ref(&item));

    assert_eq!(
        serde_json::to_vec(&persisted[0]).expect("serialize persisted item"),
        serde_json::to_vec(&item).expect("serialize candidate item")
    );
    assert_eq!(
        measurement.post_filter.payload_bytes,
        measurement.items[0].payload_bytes.expect("payload bytes")
    );
}

#[test]
fn thread_totals_accumulate_append_batches() {
    let mut totals = RolloutSizeTotals::default();

    add_totals(
        &mut totals,
        RolloutSizeTotals {
            items: 2,
            payload_bytes: 30,
        },
    );
    add_totals(
        &mut totals,
        RolloutSizeTotals {
            items: 3,
            payload_bytes: 40,
        },
    );

    assert_eq!(
        totals,
        RolloutSizeTotals {
            items: 5,
            payload_bytes: 70,
        }
    );
}

#[test]
fn persisted_history_totals_include_session_metadata() {
    let session_meta = RolloutItem::SessionMeta(SessionMetaLine {
        meta: SessionMeta::default(),
        git: None,
    });
    let message = retained_message("persisted");
    let items = vec![session_meta, message];

    assert_eq!(
        measure_rollout_items(&items),
        RolloutSizeTotals {
            items: 2,
            payload_bytes: items
                .iter()
                .map(|item| serde_json::to_vec(item).expect("serialize item").len() as u64)
                .sum(),
        }
    );
}

#[test]
fn persisted_history_replaces_post_filter_totals_and_preserves_dropped_delta() {
    let totals = thread_totals_with_persisted_history(
        ThreadTotals {
            pre_filter: RolloutSizeTotals {
                items: 5,
                payload_bytes: 100,
            },
            post_filter: RolloutSizeTotals {
                items: 3,
                payload_bytes: 80,
            },
        },
        RolloutSizeTotals {
            items: 4,
            payload_bytes: 90,
        },
    );

    assert_eq!(
        totals,
        ThreadTotals {
            pre_filter: RolloutSizeTotals {
                items: 6,
                payload_bytes: 110,
            },
            post_filter: RolloutSizeTotals {
                items: 4,
                payload_bytes: 90,
            },
        }
    );
}

#[test]
fn filtered_item_completion_includes_its_nested_item_type() {
    let item = RolloutItem::EventMsg(EventMsg::ItemCompleted(ItemCompletedEvent {
        thread_id: ThreadId::default(),
        turn_id: "turn".to_string(),
        item: TurnItem::UserMessage(UserMessageItem {
            id: "item".to_string(),
            client_id: None,
            content: Vec::new(),
        }),
        completed_at_ms: 0,
    }));

    let (_, measurement) = measure_and_filter_rollout_items(&[item]);

    assert_eq!(
        measurement.items[0].rollout_item_type,
        "event.item_completed.user_message"
    );
    assert_eq!(
        measurement.items[0].decision,
        super::PersistenceDecision::Dropped
    );
}
