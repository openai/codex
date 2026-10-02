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

#[tokio::test]
async fn feedback_upload_archives_two_rollouts_that_can_be_extracted_and_read() -> Result<()> {
    use codex_http_client::OutboundProxyPolicy;
    use flate2::read::GzDecoder;
    use sentry::protocol::Envelope;
    use sentry::protocol::EnvelopeItem;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let fixture = Fixture::new();
    let second_filename = format!("rollout-2026-09-09T12-00-00-{}.jsonl", ThreadId::new());
    let second_bytes = b"{\"message\":\"second rollout\"}\n";
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/42/envelope/"))
        .respond_with(ResponseTemplate::new(200))
        .expect(/*r*/ 2)
        .mount(&server)
        .await;
    let dsn = format!("http://public@{}/42", server.address());
    CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .with_feedback_diagnostics(FeedbackDiagnostics::default())
        .upload_feedback_with_dsn(
            FeedbackUploadOptions {
                classification: "bug",
                reason: None,
                tags: None,
                include_logs: false,
                extra_attachments: vec![FeedbackAttachment {
                    filename: second_filename.clone(),
                    content_type: Some("text/plain".to_string()),
                    buffer: second_bytes.to_vec(),
                }],
                extra_attachment_paths: &[fixture.attachment()],
                session_source: None,
                logs_override: None,
            },
            &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            &dsn,
            Instant::now() + UPLOAD_TIMEOUT,
        )
        .await?;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    let mut body = requests[1].body.clone();
    if body.starts_with(&[0x1f, 0x8b]) {
        let mut decoded = Vec::new();
        GzDecoder::new(body.as_slice()).read_to_end(&mut decoded)?;
        body = decoded;
    }
    let envelope = Envelope::from_slice(&body)?;
    let mut items = envelope.items();
    let Some(EnvelopeItem::Attachment(attachment)) = items.next() else {
        anyhow::bail!("expected a rollout archive attachment");
    };
    assert_eq!(attachment.filename, "rollouts.tar.gz");
    assert!(items.next().is_none());

    let extracted = tempfile::tempdir()?;
    tar::Archive::new(GzDecoder::new(attachment.buffer.as_slice())).unpack(extracted.path())?;
    let files = fs::read_dir(extracted.path())?
        .map(|entry| {
            let entry = entry?;
            Ok((entry.file_name(), fs::read(entry.path())?))
        })
        .collect::<io::Result<BTreeMap<_, _>>>()?;
    assert_eq!(
        files,
        BTreeMap::from([
            (
                fixture.plain.file_name().unwrap().to_owned(),
                JSONL.to_vec()
            ),
            (second_filename.into(), second_bytes.to_vec()),
        ])
    );
    Ok(())
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
