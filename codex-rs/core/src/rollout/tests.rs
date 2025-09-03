#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs::File;
use std::fs::{self};
use std::io::Write;
use std::path::Path;

use tempfile::TempDir;
use time::OffsetDateTime;
use time::PrimitiveDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use uuid::Uuid;

use crate::rollout::list::get_conversation;
use crate::rollout::list::get_conversations;

fn write_session_file(
    root: &Path,
    ts_str: &str,
    uuid: Uuid,
    num_records: usize,
) -> std::io::Result<(OffsetDateTime, Uuid)> {
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let dt = PrimitiveDateTime::parse(ts_str, format)
        .unwrap()
        .assume_utc();
    let dir = root
        .join("sessions")
        .join(format!("{:04}", dt.year()))
        .join(format!("{:02}", u8::from(dt.month())))
        .join(format!("{:02}", dt.day()));
    fs::create_dir_all(&dir)?;

    let filename = format!("rollout-{ts_str}-{uuid}.jsonl");
    let file_path = dir.join(filename);
    let mut file = File::create(file_path)?;

    let meta = serde_json::json!({
        "timestamp": ts_str,
        "id": uuid.to_string()
    });
    writeln!(file, "{meta}")?;

    for i in 0..num_records {
        let rec = serde_json::json!({
            "record_type": "response",
            "index": i
        });
        writeln!(file, "{rec}")?;
    }
    Ok((dt, uuid))
}

#[tokio::test]
async fn test_list_conversations_latest_first() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    for day in 1..=3 {
        let ts = format!("2025-01-{day:02}T12-00-00");
        write_session_file(home, &ts, Uuid::new_v4(), 3).unwrap();
    }

    let page = get_conversations(home, 10, None).await.unwrap();

    assert_eq!(page.items.len(), 3);
    assert!(!page.reached_scan_cap);
    assert_eq!(page.scanned_files, 3);

    let names: Vec<String> = page
        .items
        .iter()
        .map(|it| it.path.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(names[0].contains("2025-01-03T12-00-00"));
    assert!(names[1].contains("2025-01-02T12-00-00"));
    assert!(names[2].contains("2025-01-01T12-00-00"));

    for it in page.items {
        assert!(!it.head.is_empty() && it.head.len() <= 5);
    }
}

#[tokio::test]
async fn test_pagination_cursor() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    for i in 0..5 {
        let ts = format!("2025-03-{:02}T09-00-00", i + 1);
        write_session_file(home, &ts, Uuid::new_v4(), 1).unwrap();
    }

    let page1 = get_conversations(home, 2, None).await.unwrap();
    assert_eq!(page1.items.len(), 2);
    let n1: Vec<_> = page1
        .items
        .iter()
        .map(|it| it.path.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(n1[0].contains("2025-03-05T09-00-00"));
    assert!(n1[1].contains("2025-03-04T09-00-00"));

    let page2 = get_conversations(home, 2, page1.next_cursor.as_deref())
        .await
        .unwrap();
    assert_eq!(page2.items.len(), 2);
    let n2: Vec<_> = page2
        .items
        .iter()
        .map(|it| it.path.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(n2[0].contains("2025-03-03T09-00-00"));
    assert!(n2[1].contains("2025-03-02T09-00-00"));

    let page3 = get_conversations(home, 2, page2.next_cursor.as_deref())
        .await
        .unwrap();
    assert_eq!(page3.items.len(), 1);
    let n3: Vec<_> = page3
        .items
        .iter()
        .map(|it| it.path.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(n3[0].contains("2025-03-01T09-00-00"));
}

#[tokio::test]
async fn test_get_conversation_contents() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let uuid = Uuid::new_v4();
    let ts = "2025-04-01T10-30-00";
    write_session_file(home, ts, uuid, 2).unwrap();

    let page = get_conversations(home, 1, None).await.unwrap();
    let path = &page.items[0].path;

    let content = get_conversation(path).await.unwrap();

    assert!(content.contains("2025-04-01T10-30-00"));
    assert!(content.contains(&uuid.to_string()));

    assert_eq!(page.items[0].head.len(), 3);
}

#[tokio::test]
async fn test_stable_ordering_same_second_pagination() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let ts = "2025-07-01T00-00-00";
    let u1 = Uuid::from_u128(1);
    let u2 = Uuid::from_u128(2);
    let u3 = Uuid::from_u128(3);

    write_session_file(home, ts, u1, 0).unwrap();
    write_session_file(home, ts, u2, 0).unwrap();
    write_session_file(home, ts, u3, 0).unwrap();

    let page1 = get_conversations(home, 2, None).await.unwrap();
    let names1: Vec<_> = page1
        .items
        .iter()
        .map(|it| it.path.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(names1[0].contains(&u3.to_string()));
    assert!(names1[1].contains(&u2.to_string()));

    let page2 = get_conversations(home, 2, page1.next_cursor.as_deref())
        .await
        .unwrap();
    assert_eq!(page2.items.len(), 1);
    let name2 = page2.items[0]
        .path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(name2.contains(&u1.to_string()));
}
