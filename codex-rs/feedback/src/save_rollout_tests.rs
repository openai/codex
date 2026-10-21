//! Regression coverage for complete, bounded file selection during local saving.
use super::*;
use pretty_assertions::assert_eq;

fn rollout_name() -> String {
    format!("rollout-2026-10-19T12-00-00-{}.jsonl", ThreadId::new())
}

#[test]
fn save_bounds_files_before_reading_or_compression() -> Result<()> {
    let inputs = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let path = inputs.path().join(rollout_name());
    fs::write(&path, vec![b' '; 1024])?;
    let path = FeedbackAttachmentPath {
        path,
        attachment_filename_override: None,
    };
    let missing = FeedbackAttachmentPath {
        path: inputs.path().join("missing"),
        attachment_filename_override: None,
    };
    for (max_bytes, max_attachments, second, expected_error) in [
        (1500, 2, &path, "decoded size limit"),
        (4096, 1, &missing, "count limit"),
    ] {
        let error = archive::save(
            &output.path().join("feedback.zip"),
            b"{}",
            [
                archive::Input::Owned(FeedbackAttachment {
                    filename: rollout_name(),
                    content_type: None,
                    buffer: vec![b' '; 1024],
                }),
                archive::Input::Path(second),
            ]
            .into_iter(),
            archive::Limits {
                max_bytes,
                max_attachments,
            },
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains(expected_error), "{error:#}");
        assert_eq!(fs::read_dir(output.path())?.count(), 0);
    }
    Ok(())
}

#[test]
fn save_preserves_duplicate_names_and_rejects_missing_selected_files() -> Result<()> {
    let inputs = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let name = rollout_name();
    let paths = [
        inputs.path().join(&name),
        inputs.path().join("nested").join(&name),
    ];
    fs::create_dir(paths[1].parent().unwrap())?;
    fs::write(&paths[0], b"first\n")?;
    fs::write(&paths[1], b"second\n")?;
    let paths = paths.map(|path| FeedbackAttachmentPath {
        path,
        attachment_filename_override: None,
    });
    let destination = output.path().join("feedback.zip");
    archive::save(
        &destination,
        b"{}",
        paths.iter().map(archive::Input::Path),
        archive::Limits::default(),
    )?;
    let mut zip = zip::ZipArchive::new(fs::File::open(&destination)?)?;
    for (index, expected) in [b"first\n".as_slice(), b"second\n"].iter().enumerate() {
        let mut actual = Vec::new();
        zip.by_name(&format!("attachments/{index:04}/{name}"))?
            .read_to_end(&mut actual)?;
        assert_eq!(&actual, expected);
    }
    drop(zip);
    fs::remove_file(&destination)?;
    fs::remove_file(&paths[1].path)?;
    let error = archive::save(
        &destination,
        b"{}",
        paths.iter().map(archive::Input::Path),
        archive::Limits::default(),
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("selected attachment"));
    assert_eq!(fs::read_dir(output.path())?.count(), 0);
    // Upload still uses its existing best-effort aggregation and skips the missing rollout.
    let snapshot = CodexFeedback::new().snapshot(/*session_id*/ None);
    let uploaded = snapshot
        .feedback_attachments(
            &sentry::protocol::EnvelopeHeaders::default(),
            /*include_logs*/ false,
            Vec::new(),
            &paths,
            /*logs_override*/ None,
        )
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(uploaded.len(), 1);
    assert_eq!(uploaded[0].filename, "rollouts.tar.gz");
    Ok(())
}
