//! Exercises lazy feedback attachment reads across rollout representations.

use super::*;
use pretty_assertions::assert_eq;

const JSONL: &[u8] = b"{\"message\":\"old diagnostic\"}\n{\"message\":\"more details\"}\n";
// A fixed zstd frame for JSONL; the attachment reader does not need an encoder dependency.
const COMPRESSED: &[u8] = &[
    40, 181, 47, 253, 0, 88, 165, 1, 0, 196, 2, 123, 34, 109, 101, 115, 115, 97, 103, 101, 34, 58,
    34, 111, 108, 100, 32, 100, 105, 97, 103, 110, 111, 115, 116, 105, 99, 34, 125, 10, 109, 111,
    114, 101, 32, 100, 101, 116, 97, 105, 108, 115, 34, 125, 10, 1, 0, 1, 78, 57, 1,
];

struct Fixture {
    directory: PathBuf,
    plain: PathBuf,
    compressed: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let thread_id = ThreadId::new();
        let directory = std::env::temp_dir().join(format!("codex-feedback-rollout-{thread_id}"));
        fs::create_dir(&directory).expect("create fixture directory");
        let plain = directory.join(format!("rollout-2026-09-09T12-00-00-{thread_id}.jsonl"));
        let compressed = plain.with_extension("jsonl.zst");
        fs::write(&compressed, COMPRESSED).expect("write compressed rollout");
        Self {
            directory,
            plain,
            compressed,
        }
    }

    fn attachment(&self) -> FeedbackAttachmentPath {
        FeedbackAttachmentPath {
            path: self.plain.clone(),
            attachment_filename_override: None,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn unloaded_compressed_rollout_is_included_as_jsonl_attachment() {
    let fixture = Fixture::new();
    // The app-server supplies the DB's logical .jsonl path without loading this thread.
    let paths = [fixture.attachment()];
    let snapshot = CodexFeedback::new().snapshot(/*session_id*/ None);
    let attachments = snapshot
        .feedback_attachments(
            /*include_logs*/ false,
            &[],
            &paths,
            /*logs_override*/ None,
        )
        .collect::<Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        attachments
            .iter()
            .map(|attachment| (
                attachment.filename.as_str(),
                attachment.content_type.as_deref(),
                attachment.buffer.as_slice(),
            ))
            .collect::<Vec<_>>(),
        vec![(
            fixture.plain.file_name().unwrap().to_str().unwrap(),
            Some("text/plain"),
            JSONL
        )]
    );
    assert!(
        !fixture.plain.exists(),
        "feedback must not materialize the durable rollout"
    );
}

#[test]
fn compressed_paths_keep_canonical_names_and_filename_overrides() {
    let mut fixture = Fixture::new();
    // Reverted rollouts carry a stable thread ID and a separate immutable rollout ID.
    let plain = fixture.directory.join(format!(
        "rollout-2026-09-09T12-00-00-{}_{}.jsonl",
        ThreadId::new(),
        ThreadId::new()
    ));
    let compressed = plain.with_extension("jsonl.zst");
    fs::rename(&fixture.compressed, &compressed).unwrap();
    fixture.plain = plain;
    fixture.compressed = compressed;

    for filename_override in [None, Some("reviewer-rollout.jsonl".to_string())] {
        let path = FeedbackAttachmentPath {
            path: fixture.compressed.clone(),
            attachment_filename_override: filename_override.clone(),
        };
        let attachment = path.read_attachment(JSONL.len()).unwrap().unwrap();
        let expected_filename = filename_override.unwrap_or_else(|| {
            fixture
                .plain
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        });
        assert_eq!(
            (
                attachment.filename,
                attachment.content_type,
                attachment.buffer
            ),
            (
                expected_filename,
                Some("text/plain".to_string()),
                JSONL.to_vec()
            )
        );
    }
}

#[test]
fn compressed_attachment_prefers_newer_plain_sibling() {
    let fixture = Fixture::new();
    let current = b"{\"message\":\"new diagnostic\"}\n";
    fs::write(&fixture.plain, current).unwrap();
    let attachment = FeedbackAttachmentPath {
        path: fixture.compressed.clone(),
        attachment_filename_override: None,
    }
    .read_attachment(/*max_bytes*/ 1024)
    .unwrap()
    .unwrap();
    assert_eq!(attachment.buffer, current);
    assert_eq!(fs::read(&fixture.compressed).unwrap(), COMPRESSED);
}

#[test]
fn queued_attachments_follow_both_representation_transitions() {
    let fixture = Fixture::new();
    let compressed_path = FeedbackAttachmentPath {
        path: fixture.compressed.clone(),
        attachment_filename_override: None,
    };
    // The compressed file is materialized after the attachment path was queued.
    fs::write(&fixture.plain, JSONL).unwrap();
    fs::remove_file(&fixture.compressed).unwrap();
    assert_eq!(
        compressed_path
            .read_attachment(/*max_bytes*/ 1024)
            .unwrap()
            .unwrap()
            .buffer,
        JSONL
    );
    let plain_path = fixture.attachment();
    // The plain file is compressed after the attachment path was queued.
    fs::write(&fixture.compressed, COMPRESSED).unwrap();
    fs::remove_file(&fixture.plain).unwrap();
    assert_eq!(
        plain_path
            .read_attachment(/*max_bytes*/ 1024)
            .unwrap()
            .unwrap()
            .buffer,
        JSONL
    );
}

#[test]
fn compressed_attachment_bounds_decoded_bytes() {
    let fixture = Fixture::new();
    let path = fixture.attachment();
    assert!(path.read_attachment(/*max_bytes*/ 35).unwrap().is_none());
    assert_eq!(
        path.read_attachment(JSONL.len()).unwrap().unwrap().buffer,
        JSONL
    );
    assert!(!fixture.plain.exists());
}

#[test]
fn nonregular_rollout_is_omitted_and_unrelated_zstd_attachment_stays_opaque() {
    let fixture = Fixture::new();
    fs::create_dir(&fixture.plain).unwrap();
    assert!(
        fixture
            .attachment()
            .read_attachment(/*max_bytes*/ 1024)
            .unwrap()
            .is_none()
    );
    let opaque_path = fixture.directory.join("diagnostics.zst");
    fs::rename(&fixture.compressed, &opaque_path).unwrap();
    let attachment = FeedbackAttachmentPath {
        path: opaque_path,
        attachment_filename_override: None,
    }
    .read_attachment(/*max_bytes*/ 1024)
    .unwrap()
    .unwrap();
    assert_eq!(
        (attachment.filename.as_str(), attachment.buffer.as_slice()),
        ("diagnostics.zst", COMPRESSED)
    );
}
