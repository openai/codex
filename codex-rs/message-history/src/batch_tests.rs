use std::fs::File;
use std::io::Write;

use codex_config::types::History;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;

fn entry(offset: usize, text: impl Into<String>) -> HistoryEntry {
    HistoryEntry {
        session_id: "session".to_string(),
        ts: offset as u64,
        text: text.into(),
    }
}

fn write_entries(home: &TempDir, entries: &[HistoryEntry]) -> HistoryConfig {
    let mut file = File::create(home.path().join(HISTORY_FILENAME)).expect("create history");
    for entry in entries {
        serde_json::to_writer(&mut file, entry).expect("serialize entry");
        writeln!(file).expect("write entry");
    }
    HistoryConfig::new(home.path(), &History::default())
}

async fn batch_for(entries: &[HistoryEntry], end_offset: usize) -> (TempDir, HistoryBatch) {
    let home = TempDir::new().expect("temp dir");
    let config = write_entries(&home, entries);
    let (log_id, _) = history_metadata(&config).await;
    let batch = lookup_batch(log_id, HistoryBatchCursor::new(end_offset), &config)
        .expect("read history batch");
    (home, batch)
}

#[tokio::test]
async fn search_batch_returns_bounded_newest_first_absolute_offsets() {
    let entries: Vec<_> = (0..400)
        .map(|offset| entry(offset, format!("row {offset}")))
        .collect();
    let (home, batch) = batch_for(&entries, /*end_offset*/ 399).await;

    assert_eq!(batch.entries.len(), 128);
    assert_eq!(batch.entries.first().map(|entry| entry.offset), Some(399));
    assert_eq!(batch.entries.last().map(|entry| entry.offset), Some(272));
    let next_cursor = batch.next_older_cursor.expect("older cursor");
    assert_eq!(next_cursor.end_offset(), 271);
    assert_eq!(batch.entries[0].entry, Some(entries[399].clone()));

    let config = HistoryConfig::new(home.path(), &History::default());
    let (log_id, _) = history_metadata(&config).await;
    let mut offsets: Vec<_> = batch.entries.iter().map(|entry| entry.offset).collect();
    let mut cursor = Some(next_cursor);
    while let Some(next_cursor) = cursor {
        let older = lookup_batch(log_id, next_cursor, &config).expect("read older history batch");
        offsets.extend(older.entries.iter().map(|entry| entry.offset));
        cursor = older.next_older_cursor;
    }
    assert_eq!(offsets, (0..400).rev().collect::<Vec<_>>());
}

#[tokio::test]
async fn search_batch_invalidates_byte_cursor_after_in_place_rewrite() {
    let original: Vec<_> = (0..400)
        .map(|offset| entry(offset, format!("old row {offset}")))
        .collect();
    let (home, batch) = batch_for(&original, /*end_offset*/ 399).await;
    let cursor = batch.next_older_cursor.expect("older cursor");
    let config = HistoryConfig::new(home.path(), &History::default());
    let (log_id, _) = history_metadata(&config).await;
    let original_len = std::fs::metadata(home.path().join(HISTORY_FILENAME))
        .expect("history metadata")
        .len();

    let replacement: Vec<_> = (0..500)
        .map(|offset| entry(offset, format!("replacement row {offset} with padding")))
        .collect();
    let replacement_config = write_entries(&home, &replacement);
    let replacement_len = std::fs::metadata(home.path().join(HISTORY_FILENAME))
        .expect("replacement history metadata")
        .len();
    assert!(replacement_len > original_len);
    let (replacement_log_id, _) = history_metadata(&replacement_config).await;
    assert_eq!(replacement_log_id, log_id);

    let older =
        lookup_batch(log_id, cursor, &replacement_config).expect("read rewritten history batch");
    assert_eq!(older.entries.len(), 128);
    assert_eq!(older.entries[0].offset, 271);
    assert_eq!(older.entries[0].entry, Some(replacement[271].clone()));
    assert_eq!(older.entries[127].offset, 144);
    assert_eq!(older.entries[127].entry, Some(replacement[144].clone()));
}

#[tokio::test]
async fn search_batch_rescans_capped_history_after_same_size_rewrite() {
    let home = TempDir::new().expect("temp dir");
    let original: Vec<_> = (0..400)
        .map(|offset| entry(offset, "a".repeat(20)))
        .collect();
    write_entries(&home, &original);
    let path = home.path().join(HISTORY_FILENAME);
    let original_metadata = std::fs::metadata(&path).expect("history metadata");
    let original_modified = original_metadata.modified().expect("history modified time");
    let history = History {
        max_bytes: Some(original_metadata.len() as usize),
        ..History::default()
    };
    let config = HistoryConfig::new(home.path(), &history);
    let (log_id, _) = history_metadata(&config).await;
    let first = lookup_batch(log_id, HistoryBatchCursor::new(/*end_offset*/ 399), &config)
        .expect("read history batch");
    let cursor = first.next_older_cursor.expect("older cursor");

    let mut replacement: Vec<_> = (0..400)
        .map(|offset| entry(offset, "b".repeat(20)))
        .collect();
    replacement[0].text.push_str(&"b".repeat(10));
    replacement[399].text.truncate(10);
    write_entries(&home, &replacement);
    let file = File::options()
        .write(true)
        .open(&path)
        .expect("open replacement history");
    file.set_times(std::fs::FileTimes::new().set_modified(original_modified))
        .expect("restore history modified time");
    let replacement_metadata = std::fs::metadata(&path).expect("replacement history metadata");
    assert_eq!(replacement_metadata.len(), original_metadata.len());
    assert_eq!(
        replacement_metadata.modified().ok(),
        Some(original_modified)
    );

    let older = lookup_batch(log_id, cursor, &config).expect("read rewritten history batch");
    assert_eq!(
        older,
        HistoryBatch {
            entries: (144..=271)
                .rev()
                .map(|offset| HistoryBatchEntry {
                    offset,
                    entry: Some(replacement[offset].clone()),
                })
                .collect(),
            next_older_cursor: Some(HistoryBatchCursor::new(/*end_offset*/ 143)),
        }
    );
}

#[tokio::test]
async fn search_batch_stitches_chunks_and_keeps_malformed_offsets() {
    let home = TempDir::new().expect("temp dir");
    let first = entry(/*offset*/ 0, "a".repeat(HISTORY_READ_BUFFER_SIZE + 17));
    let third = entry(/*offset*/ 2, "third");
    let contents = format!(
        "{}\nnot-json\n{}\n",
        serde_json::to_string(&first).expect("serialize first"),
        serde_json::to_string(&third).expect("serialize third")
    );
    std::fs::write(home.path().join(HISTORY_FILENAME), contents).expect("write history");
    let config = HistoryConfig::new(home.path(), &History::default());
    let (log_id, _) = history_metadata(&config).await;

    assert_eq!(
        lookup_batch(log_id, HistoryBatchCursor::new(/*end_offset*/ 2), &config,)
            .expect("read history batch"),
        HistoryBatch {
            entries: vec![
                HistoryBatchEntry {
                    offset: 2,
                    entry: Some(third),
                },
                HistoryBatchEntry {
                    offset: 1,
                    entry: None,
                },
                HistoryBatchEntry {
                    offset: 0,
                    entry: Some(first),
                },
            ],
            next_older_cursor: None,
        }
    );
}

#[tokio::test]
async fn search_batch_preserves_identity_append_trim_and_short_file_semantics() {
    let home = TempDir::new().expect("temp dir");
    let initial = vec![entry(/*offset*/ 0, "zero"), entry(/*offset*/ 1, "one")];
    let config = write_entries(&home, &initial);
    let (log_id, _) = history_metadata(&config).await;
    assert_eq!(
        lookup_batch(
            log_id.wrapping_add(1),
            HistoryBatchCursor::new(/*end_offset*/ 1),
            &config,
        )
        .expect("read history batch"),
        HistoryBatch::default()
    );

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(home.path().join(HISTORY_FILENAME))
        .expect("open history");
    serde_json::to_writer(&mut file, &entry(/*offset*/ 2, "appended")).expect("serialize append");
    writeln!(file).expect("append entry");
    let batch = lookup_batch(log_id, HistoryBatchCursor::new(/*end_offset*/ 1), &config)
        .expect("read history batch");
    assert_eq!(
        batch.entries,
        vec![
            HistoryBatchEntry {
                offset: 1,
                entry: Some(initial[1].clone()),
            },
            HistoryBatchEntry {
                offset: 0,
                entry: Some(initial[0].clone()),
            },
        ]
    );

    let newest = "c".repeat(200);
    let history = History {
        max_bytes: Some(newest.len() + 80),
        ..History::default()
    };
    let trimmed_config = HistoryConfig::new(home.path(), &history);
    append_entry(&newest, "session", &trimmed_config)
        .await
        .expect("append and trim");
    let trimmed = lookup_batch(
        log_id,
        HistoryBatchCursor::new(/*end_offset*/ 20),
        &trimmed_config,
    )
    .expect("read trimmed history batch");
    assert_eq!(trimmed.entries.len(), 1);
    assert_eq!(trimmed.entries[0].offset, 0);
    assert_eq!(
        trimmed.entries[0].entry.as_ref().map(|entry| &entry.text),
        Some(&newest)
    );
    assert_eq!(trimmed.next_older_cursor, None);
}

#[tokio::test]
async fn search_batch_enforces_byte_cap_and_oversized_row_progress() {
    let entries: Vec<_> = (0..5)
        .map(|offset| {
            entry(
                offset,
                char::from(b'a' + offset as u8).to_string().repeat(20_000),
            )
        })
        .collect();
    let (_home, batch) = batch_for(&entries, /*end_offset*/ 4).await;
    assert_eq!(batch.entries.len(), 3);
    assert_eq!(batch.entries.first().map(|entry| entry.offset), Some(4));
    assert_eq!(batch.entries.last().map(|entry| entry.offset), Some(2));
    assert_eq!(
        batch.next_older_cursor.expect("older cursor").end_offset(),
        1
    );

    let entries = vec![
        entry(/*offset*/ 0, "small"),
        entry(/*offset*/ 1, "x".repeat(70_000)),
    ];
    let (home, oversized) = batch_for(&entries, /*end_offset*/ 1).await;
    assert_eq!(oversized.entries.len(), 1);
    assert_eq!(oversized.entries[0].entry, Some(entries[1].clone()));
    let next_cursor = oversized.next_older_cursor.expect("older cursor");
    assert_eq!(next_cursor.end_offset(), 0);
    let config = HistoryConfig::new(home.path(), &History::default());
    let (log_id, _) = history_metadata(&config).await;
    let next = lookup_batch(log_id, next_cursor, &config).expect("read older history batch");
    assert_eq!(next.entries[0].entry, Some(entries[0].clone()));
    assert_eq!(next.next_older_cursor, None);

    let entries = vec![
        entry(/*offset*/ 0, "x".repeat(70_000)),
        entry(/*offset*/ 1, "newest"),
    ];
    let (home, newest) = batch_for(&entries, /*end_offset*/ 1).await;
    assert_eq!(
        newest.entries,
        vec![HistoryBatchEntry {
            offset: 1,
            entry: Some(entries[1].clone()),
        }]
    );
    let next_cursor = newest.next_older_cursor.expect("older cursor");
    let config = HistoryConfig::new(home.path(), &History::default());
    let (log_id, _) = history_metadata(&config).await;
    let oversized = lookup_batch(log_id, next_cursor, &config).expect("read oversized older row");
    assert_eq!(
        oversized,
        HistoryBatch {
            entries: vec![HistoryBatchEntry {
                offset: 0,
                entry: Some(entries[0].clone()),
            }],
            next_older_cursor: None,
        }
    );
}

#[tokio::test]
async fn search_batch_defers_oversized_row_during_backward_scan() {
    let mut entries = vec![
        entry(/*offset*/ 0, "oldest"),
        entry(/*offset*/ 1, "x".repeat(70_000)),
    ];
    entries.extend((2..8).map(|offset| entry(offset, "x".repeat(20_000))));
    let (home, newest) = batch_for(&entries, /*end_offset*/ 7).await;
    assert_eq!(
        newest
            .entries
            .iter()
            .map(|entry| entry.offset)
            .collect::<Vec<_>>(),
        vec![7, 6, 5]
    );

    let config = HistoryConfig::new(home.path(), &History::default());
    let (log_id, _) = history_metadata(&config).await;
    let middle = lookup_batch(
        log_id,
        newest.next_older_cursor.expect("middle cursor"),
        &config,
    )
    .expect("read middle history batch");
    assert_eq!(
        middle
            .entries
            .iter()
            .map(|entry| entry.offset)
            .collect::<Vec<_>>(),
        vec![4, 3, 2]
    );

    let oversized = lookup_batch(
        log_id,
        middle.next_older_cursor.expect("oversized cursor"),
        &config,
    )
    .expect("read oversized history row");
    assert_eq!(oversized.entries[0].entry, Some(entries[1].clone()));
    assert_eq!(
        oversized
            .next_older_cursor
            .expect("oldest cursor")
            .end_offset(),
        0
    );
}
