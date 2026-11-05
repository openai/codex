#![cfg(test)]
//! Raw history captures retain complete originals and defer unfinished suffixes.

use std::fs::OpenOptions;
use std::io::Write;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::ThreadHistoryMode;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::super::test_support::test_config;
use super::super::test_support::write_session_file_with_history_mode;
use crate::LocalThreadStore;
use crate::OriginalHistoryRequest;
use crate::ThreadStore;

#[tokio::test]
async fn reads_complete_fixed_suffix_and_rejects_truncated_source()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let id = uuid::Uuid::new_v4();
    let owner = ThreadId::from_string(&id.to_string())?;
    let path = write_session_file_with_history_mode(
        dir.path(),
        "2025-01-03T10-00-00",
        id,
        ThreadHistoryMode::Paginated,
    )?;
    let store = LocalThreadStore::new(test_config(dir.path()), /*state_db*/ None);
    let start = store
        .read_original_history(owner, OriginalHistoryRequest::Tail)
        .await?
        .end;
    let original = json!({"timestamp":"2025-01-03T10:00:01Z","ordinal":1,"type":"response_item",
        "payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Original é🙂"}]}});
    let line = format!("{original}\n");
    let mut file = OpenOptions::new().append(true).open(&path)?;
    // The tail ignores a partially appended record rather than losing it.
    file.write_all(&line.as_bytes()[..12])?;
    assert_eq!(
        store
            .read_original_history(owner, OriginalHistoryRequest::Tail)
            .await?
            .end,
        start
    );
    let partial = store
        .read_original_history(owner, OriginalHistoryRequest::After(start))
        .await?;
    assert!(partial.items.is_empty());
    assert_eq!(partial.end, start);
    file.write_all(&line.as_bytes()[12..])?;
    let read = store
        .read_original_history(owner, OriginalHistoryRequest::After(start))
        .await?;
    let end = HistoryPosition {
        end_byte_offset: start.end_byte_offset + line.len() as u64,
        end_ordinal_exclusive: start.end_ordinal_exclusive + 1,
        ..start
    };
    let mut later = original.clone();
    later["ordinal"] = json!(2);
    later["payload"]["content"][0]["text"] = json!("After the cutoff");
    writeln!(file, "{later}")?;
    assert_eq!(read.end, end);
    assert_eq!(read.items.len(), 1);
    assert_eq!(read.items[0].end, end);
    assert_eq!(
        serde_json::to_value(&read.items[0].item)?,
        json!({"type":"response_item","payload":original["payload"]})
    );
    let suffix = store
        .read_original_history(owner, OriginalHistoryRequest::After(end))
        .await?;
    assert_eq!(suffix.items.len(), 1);
    assert_eq!(
        serde_json::to_value(&suffix.items[0].item)?,
        json!({"type":"response_item","payload":later["payload"]})
    );
    let other = ThreadId::new();
    let wrong = HistoryPosition {
        thread_id: other,
        ..start
    };
    assert!(
        store
            .read_original_history(owner, OriginalHistoryRequest::After(wrong))
            .await
            .is_err()
    );
    // Windows append-only handles cannot truncate the file.
    drop(file);
    OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(start.end_byte_offset)?;
    assert!(
        store
            .read_original_history(owner, OriginalHistoryRequest::After(end))
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn reads_compressed_suffix_and_defers_unfinished_record()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let id = uuid::Uuid::new_v4();
    let owner = ThreadId::from_string(&id.to_string())?;
    let path = write_session_file_with_history_mode(
        dir.path(),
        "2025-01-03T10-00-00",
        id,
        ThreadHistoryMode::Paginated,
    )?;
    let store = LocalThreadStore::new(test_config(dir.path()), /*state_db*/ None);
    let start = store
        .read_original_history(owner, OriginalHistoryRequest::Tail)
        .await?
        .end;
    let original = json!({"timestamp":"2025-01-03T10:00:01Z","ordinal":1,"type":"response_item",
        "payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Original é🙂"}]}});
    let mut later = original.clone();
    later["ordinal"] = json!(2);
    later["payload"]["content"][0]["text"] = json!("Completed in the next frame");
    let original_line = format!("{original}\n");
    let later_line = format!("{later}\n");
    let mut file = OpenOptions::new().append(true).open(&path)?;
    file.write_all(original_line.as_bytes())?;
    file.write_all(&later_line.as_bytes()[..12])?;
    drop(file);

    let compressed_path = path.with_extension("jsonl.zst");
    let contents = std::fs::read(&path)?;
    std::fs::write(
        &compressed_path,
        zstd::stream::encode_all(contents.as_slice(), /*level*/ 3)?,
    )?;
    std::fs::remove_file(&path)?;
    let read = store
        .read_original_history(owner, OriginalHistoryRequest::After(start))
        .await?;
    let end = HistoryPosition {
        end_byte_offset: start.end_byte_offset + original_line.len() as u64,
        end_ordinal_exclusive: start.end_ordinal_exclusive + 1,
        ..start
    };
    assert_eq!(read.end, end);
    assert_eq!(read.items.len(), 1);
    assert_eq!(
        (
            serde_json::to_value(&read.items[0].item)?,
            read.items[0].end
        ),
        (
            json!({"type":"response_item","payload":original["payload"]}),
            end
        )
    );

    // A complete zstd frame may still end with an unfinished logical JSONL record.
    let mut compressed = OpenOptions::new().append(true).open(&compressed_path)?;
    compressed.write_all(&zstd::stream::encode_all(
        &later_line.as_bytes()[12..],
        /*level*/ 3,
    )?)?;
    drop(compressed);
    let suffix = store
        .read_original_history(owner, OriginalHistoryRequest::After(end))
        .await?;
    let tail = HistoryPosition {
        end_byte_offset: end.end_byte_offset + later_line.len() as u64,
        end_ordinal_exclusive: end.end_ordinal_exclusive + 1,
        ..end
    };
    assert_eq!(suffix.end, tail);
    assert_eq!(suffix.items.len(), 1);
    assert_eq!(
        (
            serde_json::to_value(&suffix.items[0].item)?,
            suffix.items[0].end
        ),
        (
            json!({"type":"response_item","payload":later["payload"]}),
            tail
        )
    );
    assert!(!path.exists());
    Ok(())
}
