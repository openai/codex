//! Tests the strict boundary between authentic legacy rollouts and bad files.
//!
//! Positive cases cover the historical header and record shapes. Negative
//! cases preserve Doctor warnings for mismatched identities, modern envelopes,
//! malformed records, and empty files.

use super::*;
use std::path::PathBuf;
use tempfile::TempDir;

const THREAD_ID: &str = "00000000-0000-0000-0000-000000000001";

#[tokio::test]
async fn recognizes_historical_rollout_shape() {
    let fixture = LegacyFixture::new(THREAD_ID);
    fixture.write_lines(&[
        &format!(
            r#"{{"id":"{THREAD_ID}","timestamp":"2025-08-13T15:22:56.917Z","instructions":null}}"#
        ),
        r#"{"record_type":"state"}"#,
        r#"{"type":"message","id":null,"role":"user","content":[{"type":"input_text","text":"hello"}]}"#,
    ]);

    assert!(is_legacy_rollout(&fixture.path).await.unwrap());
}

#[tokio::test]
async fn recognizes_instructions_and_git_metadata() {
    let fixture = LegacyFixture::new(THREAD_ID);
    fixture.write_lines(&[&format!(
        r#"{{"id":"{THREAD_ID}","timestamp":"2025-08-13T15:22:56.917Z","instructions":"be concise","git":{{"branch":"main"}}}}"#
    )]);

    assert!(is_legacy_rollout(&fixture.path).await.unwrap());
}

#[tokio::test]
async fn rejects_header_id_that_differs_from_filename() {
    let fixture = LegacyFixture::new(THREAD_ID);
    fixture.write_lines(&[r#"{"id":"00000000-0000-0000-0000-000000000002","timestamp":"2025-08-13T15:22:56.917Z","instructions":null}"#]);

    assert!(!is_legacy_rollout(&fixture.path).await.unwrap());
}

#[tokio::test]
async fn rejects_invalid_headers() {
    for header in [
        format!(r#"{{"id":"{THREAD_ID}","timestamp":"2025-08-13T15:22:56.917Z"}}"#),
        format!(
            r#"{{"id":"{THREAD_ID}","timestamp":"2025-08-13T15:22:56.917Z","instructions":null,"extra":true}}"#
        ),
        format!(
            r#"{{"timestamp":"2025-08-13T15:22:56.917Z","type":"session_meta","payload":{{"id":"{THREAD_ID}"}}}}"#
        ),
    ] {
        let fixture = LegacyFixture::new(THREAD_ID);
        fixture.write_lines(&[&header]);

        assert!(!is_legacy_rollout(&fixture.path).await.unwrap());
    }
}

#[tokio::test]
async fn rejects_invalid_following_records() {
    for record in ["{", "[]", "null"] {
        let fixture = LegacyFixture::new(THREAD_ID);
        fixture.write_lines(&[
            &format!(
                r#"{{"id":"{THREAD_ID}","timestamp":"2025-08-13T15:22:56.917Z","instructions":null}}"#
            ),
            record,
        ]);

        assert!(!is_legacy_rollout(&fixture.path).await.unwrap());
    }
}

#[tokio::test]
async fn rejects_empty_file() {
    let fixture = LegacyFixture::new(THREAD_ID);
    fixture.write_lines(&[]);

    assert!(!is_legacy_rollout(&fixture.path).await.unwrap());
}

struct LegacyFixture {
    _temp_dir: TempDir,
    path: PathBuf,
}

impl LegacyFixture {
    fn new(thread_id: &str) -> Self {
        let temp_dir = TempDir::new().expect("temp dir");
        let path = temp_dir
            .path()
            .join(format!("rollout-2025-08-13T15-22-56-{thread_id}.jsonl"));
        Self {
            _temp_dir: temp_dir,
            path,
        }
    }

    fn write_lines(&self, lines: &[&str]) {
        let mut contents = lines.join("\n");
        if !contents.is_empty() {
            contents.push('\n');
        }
        std::fs::write(&self.path, contents).expect("write rollout");
    }
}
