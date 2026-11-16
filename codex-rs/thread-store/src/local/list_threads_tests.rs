#![cfg(test)]

//! Tests SQLite thread listing. Each row for a repeated legacy thread retains
//! its own metadata and the name from the session index.

use std::collections::HashMap;

use chrono::DateTime;
use chrono::TimeDelta;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_state::ThreadMetadataBuilder;
use codex_state::ThreadSection;
use codex_state::ThreadsPage;
use pretty_assertions::assert_eq;

use super::thread_page_from_state;

#[test]
fn duplicate_legacy_rows_keep_the_indexed_name() {
    let id = ThreadId::new();
    let first_update = DateTime::UNIX_EPOCH;
    let second_update = first_update + TimeDelta::seconds(2);
    let mut first =
        ThreadMetadataBuilder::new(id, "rollout.jsonl".into(), first_update, SessionSource::Cli)
            .build("test-provider");
    first.section = Some(ThreadSection {
        id: "section".to_owned(),
        name: "Section".to_owned(),
        appearance: None,
    });
    first.section_position = Some(1_000_000);
    // A thread updated across the cursor can appear again when a missing rollout forces a refill.
    let mut repeated = first.clone();
    repeated.updated_at = second_update;
    repeated.section_position = Some(2_000_000);
    let page = ThreadsPage {
        items: vec![first, repeated],
        parent_thread_ids: HashMap::new(),
        next_anchor: None,
        num_scanned_rows: 3,
    };
    let name = "Indexed name";
    let indexed_names = HashMap::from([(id, name.to_owned())]);

    let page = thread_page_from_state(
        page,
        &indexed_names,
        /*archived*/ false,
        "test-provider",
    );

    let rows: Vec<_> = page
        .items
        .iter()
        .map(|thread| {
            (
                thread.thread_id,
                thread.updated_at,
                thread.name.as_deref(),
                thread.section_position,
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            (id, first_update, Some(name), Some(1_000_000)),
            (id, second_update, Some(name), Some(2_000_000)),
        ]
    );
}
