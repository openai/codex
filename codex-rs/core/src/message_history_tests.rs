use super::*;
use crate::config::ConfigBuilder;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;

#[tokio::test]
async fn lookup_reads_history_entries() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let history_path = temp_dir.path().join(HISTORY_FILENAME);

    let entries = vec![
        HistoryEntry {
            session_id: "first-session".to_string(),
            ts: 1,
            text: "first".to_string(),
        },
        HistoryEntry {
            session_id: "second-session".to_string(),
            ts: 2,
            text: "second".to_string(),
        },
    ];

    let mut file = File::create(&history_path).expect("create history file");
    for entry in &entries {
        writeln!(
            file,
            "{}",
            serde_json::to_string(entry).expect("serialize history entry")
        )
        .expect("write history entry");
    }

    let (log_id, count) = history_metadata_for_file(&history_path).await;
    assert_eq!(count, entries.len());

    let second_entry = lookup_history_entry(&history_path, log_id, /*offset*/ 1)
        .expect("fetch second history entry");
    assert_eq!(second_entry, entries[1]);
}

#[tokio::test]
async fn lookup_uses_stable_log_id_after_appends() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let history_path = temp_dir.path().join(HISTORY_FILENAME);

    let initial = HistoryEntry {
        session_id: "first-session".to_string(),
        ts: 1,
        text: "first".to_string(),
    };
    let appended = HistoryEntry {
        session_id: "second-session".to_string(),
        ts: 2,
        text: "second".to_string(),
    };

    let mut file = File::create(&history_path).expect("create history file");
    writeln!(
        file,
        "{}",
        serde_json::to_string(&initial).expect("serialize initial entry")
    )
    .expect("write initial entry");

    let (log_id, count) = history_metadata_for_file(&history_path).await;
    assert_eq!(count, 1);

    let mut append = std::fs::OpenOptions::new()
        .append(true)
        .open(&history_path)
        .expect("open history file for append");
    writeln!(
        append,
        "{}",
        serde_json::to_string(&appended).expect("serialize appended entry")
    )
    .expect("append history entry");

    let fetched = lookup_history_entry(&history_path, log_id, /*offset*/ 1)
        .expect("lookup appended history entry");
    assert_eq!(fetched, appended);
}

#[tokio::test]
async fn append_entry_trims_history_when_beyond_max_bytes() {
    let codex_home = TempDir::new().expect("create temp dir");

    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("load config");

    let conversation_id = ThreadId::new();

    let entry_one = "a".repeat(200);
    let entry_two = "b".repeat(200);

    let history_path = codex_home.path().join("history.jsonl");

    append_entry(&entry_one, &conversation_id, &config)
        .await
        .expect("write first entry");

    let first_len = std::fs::metadata(&history_path).expect("metadata").len();
    let limit_bytes = first_len + 10;

    config.history.max_bytes =
        Some(usize::try_from(limit_bytes).expect("limit should fit into usize"));

    append_entry(&entry_two, &conversation_id, &config)
        .await
        .expect("write second entry");

    let contents = std::fs::read_to_string(&history_path).expect("read history");

    let entries = contents
        .lines()
        .map(|line| serde_json::from_str::<HistoryEntry>(line).expect("parse entry"))
        .collect::<Vec<HistoryEntry>>();

    assert_eq!(
        entries.len(),
        1,
        "only one entry left because entry_one should be evicted"
    );
    assert_eq!(entries[0].text, entry_two);
    assert!(std::fs::metadata(&history_path).expect("metadata").len() <= limit_bytes);
}

#[tokio::test]
async fn append_entry_trims_history_to_soft_cap() {
    let codex_home = TempDir::new().expect("create temp dir");

    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("load config");

    let conversation_id = ThreadId::new();

    let short_entry = "a".repeat(200);
    let long_entry = "b".repeat(400);

    let history_path = codex_home.path().join("history.jsonl");

    append_entry(&short_entry, &conversation_id, &config)
        .await
        .expect("write first entry");

    let short_entry_len = std::fs::metadata(&history_path).expect("metadata").len();

    append_entry(&long_entry, &conversation_id, &config)
        .await
        .expect("write second entry");

    let two_entry_len = std::fs::metadata(&history_path).expect("metadata").len();

    let long_entry_len = two_entry_len
        .checked_sub(short_entry_len)
        .expect("second entry length should be larger than first entry length");

    config.history.max_bytes = Some(
        usize::try_from((2 * long_entry_len) + (short_entry_len / 2))
            .expect("max bytes should fit into usize"),
    );

    append_entry(&long_entry, &conversation_id, &config)
        .await
        .expect("write third entry");

    let contents = std::fs::read_to_string(&history_path).expect("read history");

    let entries = contents
        .lines()
        .map(|line| serde_json::from_str::<HistoryEntry>(line).expect("parse entry"))
        .collect::<Vec<HistoryEntry>>();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].text, long_entry);

    let pruned_len = std::fs::metadata(&history_path).expect("metadata").len();
    let max_bytes = config
        .history
        .max_bytes
        .expect("max bytes should be configured") as u64;

    assert!(pruned_len <= max_bytes);

    let soft_cap_bytes = ((max_bytes as f64) * HISTORY_SOFT_CAP_RATIO)
        .floor()
        .clamp(1.0, max_bytes as f64) as u64;
    let len_without_first = 2 * long_entry_len;

    assert!(
        len_without_first <= max_bytes,
        "dropping only the first entry would satisfy the hard cap"
    );
    assert!(
        len_without_first > soft_cap_bytes,
        "soft cap should require more aggressive trimming than the hard cap"
    );

    assert_eq!(pruned_len, long_entry_len);
    assert!(pruned_len <= soft_cap_bytes.max(long_entry_len));
}
