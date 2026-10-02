use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

fn thread_ids(count: usize) -> Vec<ThreadId> {
    (0..count)
        .map(|index| {
            ThreadId::from_string(&format!("00000000-0000-7000-8000-{index:012x}"))
                .expect("thread id")
        })
        .collect()
}

#[test]
fn old_failed_child_precedes_new_children_and_index_explains_selection() -> anyhow::Result<()> {
    let ids = thread_ids(/*count*/ 12);
    let failures = GuardianReviewFailures {
        attachment: None,
        thread_ids: vec![ids[1], ids[0]],
        process_discarded_records: 3,
    };
    let mut index = FeedbackThreadIndex::new(ids[0], ids.clone(), &failures);
    index.threads[0].rollout_filename = Some("parent.jsonl".to_string());
    index.threads[1].rollout_filename = Some("child.jsonl".to_string());
    index.threads[1].guardian_rollout_filename = Some("child-reviewer.jsonl".to_string());
    let attachment = index.attachment()?;
    let selected = [0, 1, 11, 10, 9, 8, 7, 6];
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&attachment.buffer)?,
        json!({
            "rollout_archive_filename": "rollouts.tar.gz",
            "threads": selected.into_iter().map(|i| json!({
                "thread_id": ids[i],
                "rollout_filename": match i {
                    0 => Some("parent.jsonl"),
                    1 => Some("child.jsonl"),
                    _ => None,
                },
                "guardian_rollout_filename": (i == 1).then_some("child-reviewer.jsonl"),
            })).collect::<Vec<_>>(),
            "retained_failure_thread_ids": [ids[1], ids[0]],
            "omitted_thread_ids": [ids[5], ids[4], ids[3], ids[2]],
            "unlisted_omitted_thread_count": 0,
            "process_discarded_review_records": 3,
            "notes": index.notes,
        })
    );
    Ok(())
}

#[test]
fn failure_priority_keeps_the_reported_thread_and_bounds_omission_details() {
    let ids = thread_ids(/*count*/ 100);
    let failures = GuardianReviewFailures {
        attachment: None,
        thread_ids: ids[1..10].to_vec(),
        process_discarded_records: 0,
    };
    // Exercise unsorted and duplicate subtree entries without inflating omission counts.
    let mut subtree = ids.iter().rev().copied().collect::<Vec<_>>();
    subtree.extend_from_slice(&ids[..3]);
    let index = FeedbackThreadIndex::new(ids[0], subtree, &failures);
    assert_eq!(
        (
            index
                .threads
                .iter()
                .map(|thread| thread.thread_id)
                .collect::<Vec<_>>(),
            index.omitted_thread_ids,
            index.unlisted_omitted_thread_count,
            index.retained_failure_thread_ids,
        ),
        (
            ids[..8].to_vec(),
            ids[36..].iter().rev().copied().collect::<Vec<_>>(),
            28,
            failures.thread_ids,
        )
    );
}
