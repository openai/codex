use super::*;
use crate::CodexFeedback;
use crate::FeedbackUploadOptions;
use pretty_assertions::assert_eq;
use std::fs::File;
use std::io::Read;
use zip::ZipArchive;

fn options() -> FeedbackUploadOptions<'static> {
    FeedbackUploadOptions {
        classification: "bug",
        reason: Some("synthetic feedback"),
        tags: None,
        include_logs: false,
        extra_attachments: Vec::new(),
        extra_attachment_paths: &[],
        session_source: None,
        logs_override: Some(b"consented log bytes".to_vec()),
    }
}

/// Exports preserve attachment bytes and duplicate names without unsafe ZIP paths.
#[test]
fn archive_preserves_selected_bytes_and_safe_names() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("feedback.zip");
    let expected = [
        ("../../same.log", b"first".as_slice()),
        ("../../same.log", b"second"),
        ("rollouts.tar.gz", b"compressed rollout bytes"),
    ];
    super::save(
        &path,
        b"{}",
        expected.iter().map(|(filename, buffer)| {
            Input::Owned(FeedbackAttachment {
                filename: filename.to_string(),
                buffer: buffer.to_vec(),
                content_type: None,
            })
        }),
        Limits::default(),
    )?;
    let mut archive = ZipArchive::new(File::open(&path)?)?;
    assert_eq!(archive.len(), 5);
    let manifest: serde_json::Value = serde_json::from_reader(archive.by_name("manifest.json")?)?;
    for (index, (filename, bytes)) in expected.iter().enumerate() {
        let basename = filename.rsplit('/').next().unwrap();
        let mut entry = archive.by_name(&format!("attachments/{index:04}/{basename}"))?;
        let mut actual = Vec::new();
        entry.read_to_end(&mut actual)?;
        assert_eq!(actual, *bytes);
        assert_eq!(manifest[index]["filename"], *filename);
        assert_eq!(entry.compression(), zip::CompressionMethod::Deflated);
    }
    Ok(())
}

/// Byte and count budgets include metadata and reject excess input without leaving a partial ZIP.
#[test]
fn archive_bounds_all_contents_and_cleans_failed_exports() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("feedback.zip");
    // Filtering removes the exact size hint, exercising streaming attachment limits.
    let attachments = |count| {
        (0..count).filter(|_| true).map(|_| {
            Input::Owned(FeedbackAttachment {
                filename: "test.log".to_string(),
                buffer: b"synthetic".to_vec(),
                content_type: None,
            })
        })
    };
    super::save(
        &path,
        b"{}",
        attachments(2),
        Limits {
            max_bytes: 1024,
            max_attachments: 2,
        },
    )?;
    let mut archive = ZipArchive::new(File::open(&path)?)?;
    let bytes = (0..archive.len()).try_fold(0, |sum, index| {
        Ok::<_, zip::result::ZipError>(sum + archive.by_index(index)?.size() as usize)
    })?;
    drop(archive);
    std::fs::remove_file(&path)?;
    super::save(
        &path,
        b"{}",
        attachments(2),
        Limits {
            max_bytes: bytes,
            max_attachments: 2,
        },
    )?;
    std::fs::remove_file(&path)?;
    for (count, max_bytes, error) in [
        (2, bytes - 1, "uncompressed size limit"),
        (0, 1, "uncompressed size limit"),
        (3, 1024, "attachment count limit"),
    ] {
        let failure = super::save(
            &path,
            b"{}",
            attachments(count),
            Limits {
                max_bytes,
                max_attachments: 2,
            },
        )
        .unwrap_err();
        assert!(failure.to_string().contains(error), "{failure:#}");
        assert_eq!(std::fs::read_dir(directory.path())?.count(), 0);
    }
    Ok(())
}

/// A local report keeps its description and includes log bytes only with consent.
#[test]
fn save_preserves_report_and_log_consent() -> Result<()> {
    let directory = tempfile::tempdir()?;
    for include_logs in [false, true] {
        let path = directory.path().join(format!("{include_logs}.zip"));
        CodexFeedback::new()
            .snapshot(/*session_id*/ None)
            .save_feedback(
                FeedbackUploadOptions {
                    include_logs,
                    ..options()
                },
                &path,
            )?;
        let mut archive = ZipArchive::new(File::open(path)?)?;
        let report: serde_json::Value = serde_json::from_reader(archive.by_name("report.json")?)?;
        assert_eq!(
            report["exception"]["values"][0]["value"],
            "synthetic feedback"
        );
        let logs = archive
            .file_names()
            .collect::<zip::result::ZipResult<Vec<_>>>()?
            .into_iter()
            .find(|name| name.ends_with("/codex-logs.log"))
            .map(std::borrow::Cow::into_owned);
        assert_eq!(logs.is_some(), include_logs);
        if let Some(logs) = logs {
            let mut bytes = Vec::new();
            archive.by_name(&logs)?.read_to_end(&mut bytes)?;
            assert_eq!(bytes, b"consented log bytes");
        }
    }
    Ok(())
}

/// Oversized reports fail before publishing, using the serialized report size.
#[test]
fn save_rejects_oversized_events_without_publishing() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let reason = "x".repeat(crate::MAX_EVENT_BYTES);
    let error = CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .save_feedback(
            FeedbackUploadOptions {
                reason: Some(&reason),
                ..options()
            },
            &directory.path().join("oversized.zip"),
        )
        .unwrap_err();
    assert_eq!(error.to_string(), "feedback event exceeds the size limit");
    assert_eq!(std::fs::read_dir(directory.path())?.count(), 0);
    Ok(())
}
