#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs::File;
use std::fs::{self};
use std::io::BufWriter;
use std::io::Write;
use std::path::{Path, PathBuf};

use filetime::{set_file_mtime, FileTime};
use tempfile::TempDir;
use time::OffsetDateTime;
use time::PrimitiveDateTime;
use time::format_description::FormatItem;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use uuid::Uuid;

use crate::config::{Config, ConfigOverrides, ConfigToml};
use crate::rollout::INTERACTIVE_SESSION_SOURCES;
use crate::rollout::list::ConversationsPage;
use crate::rollout::list::Cursor;
use crate::rollout::list::get_conversation;
use crate::rollout::list::get_conversations;
use std::time::{Duration, SystemTime};
use code_protocol::models::{ContentItem, ResponseItem};
use code_protocol::ConversationId;
use code_protocol::protocol::{
    EventMsg as ProtoEventMsg,
    RecordedEvent,
    RolloutItem,
    RolloutLine,
    SessionMeta,
    SessionMetaLine,
    SessionSource,
    UserMessageEvent,
};

const NO_SOURCE_FILTER: &[SessionSource] = &[];

fn reconstruct_history_like_rollout(items: &[RolloutItem]) -> Vec<ResponseItem> {
    let mut history = Vec::new();
    for item in items {
        match item {
            RolloutItem::ResponseItem(response) => history.push(response.clone()),
            RolloutItem::Compacted(compacted) => {
                history.push(ResponseItem::from(compacted.clone()));
            }
            _ => {}
        }
    }
    history
}

fn assert_page_summary(
    page: &ConversationsPage,
    expected_items: &[(PathBuf, &str)],
    expected_cursor: Option<Cursor>,
    expected_scanned_files: usize,
) {
    assert_eq!(page.items.len(), expected_items.len());
    for (idx, (item, (expected_path, expected_ts))) in page
        .items
        .iter()
        .zip(expected_items.iter())
        .enumerate()
    {
        assert_eq!(item.path, *expected_path, "path mismatch for item {idx}");
        assert_eq!(
            item.created_at.as_deref(),
            Some(*expected_ts),
            "created_at mismatch for item {idx}"
        );
        assert_eq!(
            item.updated_at.as_deref(),
            Some(*expected_ts),
            "updated_at mismatch for item {idx}"
        );
        assert!(
            !item.head.is_empty(),
            "expected non-empty head for item {idx}"
        );
        assert!(
            !item.tail.is_empty(),
            "expected non-empty tail for item {idx}"
        );
        let head_type = item
            .head
            .first()
            .and_then(|value| value.get("type"))
            .and_then(|v| v.as_str());
        assert_eq!(head_type, Some("session_meta"));
    }

    assert_eq!(page.next_cursor, expected_cursor);
    assert_eq!(page.num_scanned_files, expected_scanned_files);
    assert!(!page.reached_scan_cap);
}

fn write_session_file(
    root: &Path,
    ts_str: &str,
    uuid: Uuid,
    num_records: usize,
    source: Option<SessionSource>,
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
    let file = File::create(file_path)?;
    let mut writer = BufWriter::new(file);

    let conversation_id = ConversationId::from(uuid);
    let session_meta = SessionMeta {
        id: conversation_id,
        timestamp: ts_str.to_string(),
        cwd: Path::new(".").to_path_buf(),
        originator: "test_originator".to_string(),
        cli_version: "test_version".to_string(),
        instructions: None,
        source: source.unwrap_or_default(),
    };
    let session_meta_line = RolloutLine {
        timestamp: ts_str.to_string(),
        item: RolloutItem::SessionMeta(SessionMetaLine {
            meta: session_meta,
            git: None,
        }),
    };
    serde_json::to_writer(&mut writer, &session_meta_line)?;
    writer.write_all(b"\n")?;

    for i in 0..num_records {
        let event = RecordedEvent {
            id: format!("event-{i}"),
            event_seq: i as u64,
            order: None,
            msg: ProtoEventMsg::UserMessage(UserMessageEvent {
                message: format!("Message {i}"),
                kind: None,
                images: None,
            }),
        };
        let line = RolloutLine {
            timestamp: ts_str.to_string(),
            item: RolloutItem::Event(event),
        };
        serde_json::to_writer(&mut writer, &line)?;
        writer.write_all(b"\n")?;
    }
    Ok((dt, uuid))
}

#[tokio::test]
async fn test_resume_reconstruct_history_drops_user_events() {
    let temp = TempDir::new().unwrap();
    let code_home = temp.path();
    let workspace = code_home.join("workspace");
    fs::create_dir_all(&workspace).unwrap();

    let sessions_dir = code_home
        .join("sessions")
        .join("2025")
        .join("10")
        .join("06");
    fs::create_dir_all(&sessions_dir).unwrap();

    let rollout_ts = "2025-10-06T09-00-00";
    let session_uuid = Uuid::from_u128(0xfeed_babe_u128);
    let rollout_path = sessions_dir.join(format!(
        "rollout-{rollout_ts}-{session_uuid}.jsonl"
    ));

    let mut writer = BufWriter::new(File::create(&rollout_path).unwrap());

    let session_meta = SessionMeta {
        id: ConversationId::from(session_uuid),
        timestamp: "2025-10-06T09:00:00.000Z".to_string(),
        cwd: workspace.clone(),
        originator: "regression-test".to_string(),
        cli_version: "0.0.0-test".to_string(),
        instructions: Some("explain async/await".to_string()),
        source: SessionSource::Cli,
    };
    serde_json::to_writer(&mut writer, &RolloutLine {
        timestamp: session_meta.timestamp.clone(),
        item: RolloutItem::SessionMeta(SessionMetaLine {
            meta: session_meta,
            git: None,
        }),
    })
    .unwrap();
    writer.write_all(b"\n").unwrap();

    let user_event = RecordedEvent {
        id: "evt-user-0".to_string(),
        event_seq: 0,
        order: None,
        msg: ProtoEventMsg::UserMessage(UserMessageEvent {
            message: "hi there".to_string(),
            kind: None,
            images: None,
        }),
    };
    serde_json::to_writer(&mut writer, &RolloutLine {
        timestamp: "2025-10-06T09:00:01.000Z".to_string(),
        item: RolloutItem::Event(user_event),
    })
    .unwrap();
    writer.write_all(b"\n").unwrap();

    let assistant_response = ResponseItem::Message {
        id: Some("msg-assistant-0".to_string()),
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: "Sure, let's talk about async/await.".to_string(),
        }],
    };
    serde_json::to_writer(&mut writer, &RolloutLine {
        timestamp: "2025-10-06T09:00:02.000Z".to_string(),
        item: RolloutItem::ResponseItem(assistant_response),
    })
    .unwrap();
    writer.write_all(b"\n").unwrap();
    writer.flush().unwrap();

    let mut overrides = ConfigOverrides::default();
    overrides.cwd = Some(workspace.clone());
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        overrides,
        code_home.to_path_buf(),
    )
    .unwrap();

    let (recorder, saved) = crate::rollout::RolloutRecorder::resume(&config, &rollout_path)
        .await
        .expect("resume should succeed");
    recorder.shutdown().await.unwrap();

    assert_eq!(
        saved.items.len(),
        3,
        "expected to load session meta + user event + assistant response"
    );

    let reconstructed = reconstruct_history_like_rollout(&saved.items);

    assert_eq!(
        reconstructed.len(),
        2,
        "expected replay history to surface both user and assistant messages"
    );

    let user_messages = reconstructed
        .iter()
        .filter(|item| matches!(item, ResponseItem::Message { role, .. } if role == "user"))
        .count();

    assert_eq!(
        user_messages,
        1,
        "expected one user message to survive resume replay"
    );
}

#[tokio::test]
async fn test_list_conversations_latest_first() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    // Fixed UUIDs for deterministic expectations
    let u1 = Uuid::from_u128(1);
    let u2 = Uuid::from_u128(2);
    let u3 = Uuid::from_u128(3);

    // Create three sessions across three days
    write_session_file(
        home,
        "2025-01-01T12-00-00",
        u1,
        3,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-01-02T12-00-00",
        u2,
        3,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-01-03T12-00-00",
        u3,
        3,
        Some(SessionSource::VSCode),
    )
    .unwrap();

    let page = get_conversations(home, 10, None, INTERACTIVE_SESSION_SOURCES)
        .await
        .unwrap();

    let expected_items = vec![
        (
            home
                .join("sessions")
                .join("2025")
                .join("01")
                .join("03")
                .join(format!("rollout-2025-01-03T12-00-00-{u3}.jsonl")),
            "2025-01-03T12-00-00",
        ),
        (
            home
                .join("sessions")
                .join("2025")
                .join("01")
                .join("02")
                .join(format!("rollout-2025-01-02T12-00-00-{u2}.jsonl")),
            "2025-01-02T12-00-00",
        ),
        (
            home
                .join("sessions")
                .join("2025")
                .join("01")
                .join("01")
                .join(format!("rollout-2025-01-01T12-00-00-{u1}.jsonl")),
            "2025-01-01T12-00-00",
        ),
    ];

    let expected_cursor: Cursor =
        serde_json::from_str(&format!("\"2025-01-01T12-00-00|{u1}\"")).unwrap();

    assert_page_summary(&page, &expected_items, Some(expected_cursor), 3);
}

#[tokio::test]
async fn test_pagination_cursor() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    // Fixed UUIDs for deterministic expectations
    let u1 = Uuid::from_u128(11);
    let u2 = Uuid::from_u128(22);
    let u3 = Uuid::from_u128(33);
    let u4 = Uuid::from_u128(44);
    let u5 = Uuid::from_u128(55);

    // Oldest to newest
    write_session_file(
        home,
        "2025-03-01T09-00-00",
        u1,
        1,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-03-02T09-00-00",
        u2,
        1,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-03-03T09-00-00",
        u3,
        1,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-03-04T09-00-00",
        u4,
        1,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-03-05T09-00-00",
        u5,
        1,
        Some(SessionSource::VSCode),
    )
    .unwrap();

    let page1 = get_conversations(home, 2, None, INTERACTIVE_SESSION_SOURCES)
        .await
        .unwrap();
    let expected_page1_items = vec![
        (
            home
                .join("sessions")
                .join("2025")
                .join("03")
                .join("05")
                .join(format!("rollout-2025-03-05T09-00-00-{u5}.jsonl")),
            "2025-03-05T09-00-00",
        ),
        (
            home
                .join("sessions")
                .join("2025")
                .join("03")
                .join("04")
                .join(format!("rollout-2025-03-04T09-00-00-{u4}.jsonl")),
            "2025-03-04T09-00-00",
        ),
    ];
    let expected_cursor1: Cursor =
        serde_json::from_str(&format!("\"2025-03-04T09-00-00|{u4}\"")).unwrap();
    assert_page_summary(&page1, &expected_page1_items, Some(expected_cursor1.clone()), 5);

    let page2 = get_conversations(
        home,
        2,
        page1.next_cursor.as_ref(),
        INTERACTIVE_SESSION_SOURCES,
    )
    .await
    .unwrap();
    let expected_page2_items = vec![
        (
            home
                .join("sessions")
                .join("2025")
                .join("03")
                .join("03")
                .join(format!("rollout-2025-03-03T09-00-00-{u3}.jsonl")),
            "2025-03-03T09-00-00",
        ),
        (
            home
                .join("sessions")
                .join("2025")
                .join("03")
                .join("02")
                .join(format!("rollout-2025-03-02T09-00-00-{u2}.jsonl")),
            "2025-03-02T09-00-00",
        ),
    ];
    let expected_cursor2: Cursor =
        serde_json::from_str(&format!("\"2025-03-02T09-00-00|{u2}\"")).unwrap();
    assert_page_summary(&page2, &expected_page2_items, Some(expected_cursor2.clone()), 5);

    let page3 = get_conversations(
        home,
        2,
        page2.next_cursor.as_ref(),
        INTERACTIVE_SESSION_SOURCES,
    )
    .await
    .unwrap();
    let expected_cursor3: Cursor =
        serde_json::from_str(&format!("\"2025-03-01T09-00-00|{u1}\"")).unwrap();
    let expected_page3_items = vec![
        (
            home
                .join("sessions")
                .join("2025")
                .join("03")
                .join("01")
                .join(format!("rollout-2025-03-01T09-00-00-{u1}.jsonl")),
            "2025-03-01T09-00-00",
        ),
    ];
    assert_page_summary(&page3, &expected_page3_items, Some(expected_cursor3.clone()), 5);
}

#[tokio::test]
async fn test_get_conversation_contents() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let uuid = Uuid::new_v4();
    let ts = "2025-04-01T10-30-00";
    write_session_file(home, ts, uuid, 2, Some(SessionSource::VSCode)).unwrap();

    let page = get_conversations(home, 1, None, INTERACTIVE_SESSION_SOURCES)
        .await
        .unwrap();
    let expected_path = home
        .join("sessions")
        .join("2025")
        .join("04")
        .join("01")
        .join(format!("rollout-2025-04-01T10-30-00-{uuid}.jsonl"));
    let expected_items = vec![(expected_path.clone(), ts)];
    let expected_cursor: Cursor = serde_json::from_str(&format!("\"{ts}|{uuid}\"")).unwrap();
    assert_page_summary(&page, &expected_items, Some(expected_cursor), 1);

    let content = get_conversation(&page.items[0].path).await.unwrap();
    let lines: Vec<_> = content.lines().collect();
    assert_eq!(lines.len(), 3);
    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let first_type = first.get("type").and_then(|v| v.as_str());
    assert_eq!(first_type, Some("session_meta"));
    let payload_id = first
        .get("payload")
        .and_then(|payload| payload.get("id"))
        .and_then(|v| v.as_str());
    assert_eq!(payload_id, Some(uuid.to_string().as_str()));
    for (idx, line) in lines.iter().enumerate().skip(1) {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let item_type = value.get("type").and_then(|v| v.as_str());
        assert_eq!(item_type, Some("event"), "line {idx} should be event");
    }
}

#[tokio::test]
async fn test_list_conversations_prefers_recent_mtime() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let older_uuid = Uuid::from_u128(0x11);
    let newer_uuid = Uuid::from_u128(0x22);

    write_session_file(
        home,
        "2025-08-01T10-00-00",
        older_uuid,
        1,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-09-01T10-00-00",
        newer_uuid,
        1,
        Some(SessionSource::VSCode),
    )
    .unwrap();

    let older_path = home
        .join("sessions")
        .join("2025")
        .join("08")
        .join("01")
        .join(format!("rollout-2025-08-01T10-00-00-{older_uuid}.jsonl"));

    let future_time = SystemTime::now() + Duration::from_secs(300);
    set_file_mtime(&older_path, FileTime::from_system_time(future_time)).unwrap();

    let page = get_conversations(home, 1, None, INTERACTIVE_SESSION_SOURCES)
        .await
        .unwrap();

    let first = page.items.first().expect("expected at least one session");
    assert_eq!(first.path, older_path);

    let modified_at = first
        .modified_at
        .as_deref()
        .expect("expected modified_at");
    let updated_at = first
        .updated_at
        .as_deref()
        .expect("expected updated_at");

    let modified_dt = OffsetDateTime::parse(modified_at, &Rfc3339).expect("parse modified_at");
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let updated_dt = PrimitiveDateTime::parse(updated_at, format)
        .expect("parse updated_at")
        .assume_utc();
    assert!(
        modified_dt > updated_dt,
        "modified_at should reflect newer filesystem mtime"
    );
}

#[tokio::test]
async fn test_stable_ordering_same_second_pagination() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let ts = "2025-07-01T00-00-00";
    let u1 = Uuid::from_u128(1);
    let u2 = Uuid::from_u128(2);
    let u3 = Uuid::from_u128(3);

    write_session_file(home, ts, u1, 1, Some(SessionSource::VSCode)).unwrap();
    write_session_file(home, ts, u2, 1, Some(SessionSource::VSCode)).unwrap();
    write_session_file(home, ts, u3, 1, Some(SessionSource::VSCode)).unwrap();

    let page1 = get_conversations(home, 2, None, INTERACTIVE_SESSION_SOURCES)
        .await
        .unwrap();

    let expected_cursor1: Cursor = serde_json::from_str(&format!("\"{ts}|{u2}\"")).unwrap();
    let expected_page1_items = vec![
        (
            home
                .join("sessions")
                .join("2025")
                .join("07")
                .join("01")
                .join(format!("rollout-2025-07-01T00-00-00-{u3}.jsonl")),
            ts,
        ),
        (
            home
                .join("sessions")
                .join("2025")
                .join("07")
                .join("01")
                .join(format!("rollout-2025-07-01T00-00-00-{u2}.jsonl")),
            ts,
        ),
    ];
    assert_page_summary(&page1, &expected_page1_items, Some(expected_cursor1.clone()), 3);

    let page2 = get_conversations(
        home,
        2,
        page1.next_cursor.as_ref(),
        INTERACTIVE_SESSION_SOURCES,
    )
    .await
    .unwrap();
    let expected_cursor2: Cursor = serde_json::from_str(&format!("\"{ts}|{u1}\"")).unwrap();
    let expected_page2_items = vec![
        (
            home
                .join("sessions")
                .join("2025")
                .join("07")
                .join("01")
                .join(format!("rollout-2025-07-01T00-00-00-{u1}.jsonl")),
            ts,
        ),
    ];
    assert_page_summary(&page2, &expected_page2_items, Some(expected_cursor2.clone()), 3);
}

#[tokio::test]
async fn test_source_filter_excludes_non_matching_sessions() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let interactive_id = Uuid::from_u128(42);
    let non_interactive_id = Uuid::from_u128(77);

    write_session_file(
        home,
        "2025-08-02T10-00-00",
        interactive_id,
        2,
        Some(SessionSource::Cli),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-08-01T10-00-00",
        non_interactive_id,
        2,
        Some(SessionSource::Exec),
    )
    .unwrap();

    let interactive_only = get_conversations(home, 10, None, INTERACTIVE_SESSION_SOURCES)
        .await
        .unwrap();
    let paths: Vec<_> = interactive_only
        .items
        .iter()
        .map(|item| item.path.as_path())
        .collect();

    assert_eq!(paths.len(), 1);
    assert!(paths.iter().all(|path| {
        path.ends_with("rollout-2025-08-02T10-00-00-00000000-0000-0000-0000-00000000002a.jsonl")
    }));

    let all_sessions = get_conversations(home, 10, None, NO_SOURCE_FILTER)
        .await
        .unwrap();
    let all_paths: Vec<_> = all_sessions
        .items
        .into_iter()
        .map(|item| item.path)
        .collect();
    assert_eq!(all_paths.len(), 2);
    assert!(all_paths.iter().any(|path| {
        path.ends_with("rollout-2025-08-02T10-00-00-00000000-0000-0000-0000-00000000002a.jsonl")
    }));
    assert!(all_paths.iter().any(|path| {
        path.ends_with("rollout-2025-08-01T10-00-00-00000000-0000-0000-0000-00000000004d.jsonl")
    }));
}
