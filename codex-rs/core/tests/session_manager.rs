#![allow(clippy::unwrap_used, clippy::expect_used)]
// Clippy: in test code it's fine to use unwrap/expect for brevity.

use std::fs::File;
use std::fs::{self};
use std::io::Write;
use std::path::Path;

use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;
use codex_core::session_manager::SessionsMode;
use codex_core::session_manager::get_sessions;
use tempfile::TempDir;
use time::OffsetDateTime;
use time::PrimitiveDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use uuid::Uuid;

/// Helper: write a single rollout file under the temporary `CODEX_HOME`.
/// Returns the `(OffsetDateTime, Uuid)` pair used for the file.
fn write_session_file(
    root: &Path,
    ts_str: &str,
    uuid: Uuid,
    num_records: usize,
) -> std::io::Result<(OffsetDateTime, Uuid)> {
    // Compute directory layout: sessions/YYYY/MM/DD
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

    // First line: session meta.
    let meta = serde_json::json!({
        "timestamp": ts_str,
        "id": uuid.to_string()
    });
    writeln!(file, "{meta}")?;

    // Additional dummy records.
    for i in 0..num_records {
        let rec = serde_json::json!({
            "record_type": "response",
            "index": i
        });
        writeln!(file, "{rec}")?;
    }
    Ok((dt, uuid))
}

/// Construct a minimal `Config` that points to the given `codex_home` directory.
fn make_config(codex_home: &Path) -> Config {
    Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides {
            cwd: Some(codex_home.to_path_buf()),
            ..Default::default()
        },
        codex_home.to_path_buf(),
    )
    .expect("failed to construct Config for tests")
}

#[tokio::test]
async fn test_basic_retrieval_full_mode() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    // Create three sessions.
    for day in 1..=3 {
        let ts = format!("2025-01-{day:02}T12-00-00");
        write_session_file(home, &ts, Uuid::new_v4(), 3).unwrap();
    }

    let cfg = make_config(home);
    let page = get_sessions(&cfg, SessionsMode::Full, 10, None, None, None, None)
        .await
        .unwrap();

    assert_eq!(page.sessions.len(), 3);
    assert!(!page.reached_scan_cap);
    assert_eq!(page.scanned_files, 3);
}

#[tokio::test]
async fn test_date_range_filter() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    // Sessions on Jan 1..3.
    let mut dts = Vec::new();
    for day in 1..=3 {
        let ts = format!("2025-01-{day:02}T08-00-00");
        let (dt, uuid) = write_session_file(home, &ts, Uuid::new_v4(), 1).unwrap();
        dts.push((dt, uuid));
    }

    let cfg = make_config(home);

    // Filter for only Jan-02.
    let start = Some(dts[1].0);
    let end = Some(dts[1].0);
    let page = get_sessions(&cfg, SessionsMode::Lite, 10, None, start, end, None)
        .await
        .unwrap();

    assert_eq!(page.sessions.len(), 1);
    let session_meta = &page.sessions[0];
    // Expect timestamp match (meta[0] is timestamp string)
    assert_eq!(session_meta[0].as_str().unwrap(), "2025-01-02T08-00-00");
}

#[tokio::test]
async fn test_filter_by_ids() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let ts1 = "2025-02-01T10-00-00";
    let (_dt1, uuid1) = write_session_file(home, ts1, Uuid::new_v4(), 2).unwrap();
    let ts2 = "2025-02-01T10-05-00";
    write_session_file(home, ts2, Uuid::new_v4(), 2).unwrap();

    let cfg = make_config(home);

    let page = get_sessions(
        &cfg,
        SessionsMode::Lite,
        10,
        None,
        None,
        None,
        Some(&[uuid1]),
    )
    .await
    .unwrap();

    assert_eq!(page.sessions.len(), 1);
    let meta = &page.sessions[0];
    assert_eq!(meta[1].as_str().unwrap(), uuid1.to_string());
    // Also ensure scanned_files counts both.
    assert_eq!(page.scanned_files, 2);
}

#[tokio::test]
async fn test_anchor_pagination() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    // Five sequential sessions.
    let mut anchors = Vec::new();
    for i in 0..5 {
        let ts = format!("2025-03-{:02}T09-00-00", i + 1); // 2025-03-01 .. 05
        let (dt, uuid) = write_session_file(home, &ts, Uuid::new_v4(), 1).unwrap();
        anchors.push((dt, uuid));
    }

    let cfg = make_config(home);

    // Newest-first ordering: anchor represents the last item of a previous page.
    // Use the 4th (2025-03-04) session as anchor; expect to receive strictly older sessions.
    let (anchor_dt, anchor_id) = anchors[3];
    let token = format!(
        "{}|{}",
        anchor_dt
            .format(&format_description!(
                "[year]-[month]-[day]T[hour]-[minute]-[second]"
            ))
            .unwrap(),
        anchor_id
    );

    let page = get_sessions(&cfg, SessionsMode::Lite, 10, Some(&token), None, None, None)
        .await
        .unwrap();

    // Should return sessions strictly older than the anchor => 3 remaining (03, 02, 01).
    assert_eq!(page.sessions.len(), 3);
    // Verify the first returned session is 2025-03-03 (the next older).
    assert_eq!(page.sessions[0][0].as_str().unwrap(), "2025-03-03T09-00-00");
}
