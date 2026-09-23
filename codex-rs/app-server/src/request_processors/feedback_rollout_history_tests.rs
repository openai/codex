use super::*;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn includes_nested_history_bases() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let directory = home.path().join("sessions");
    std::fs::create_dir_all(&directory)?;
    let (grandparent, grandparent_end) = write_rollout(&directory, /*base*/ None)?;
    let (parent, parent_end) = write_rollout(&directory, Some(grandparent_end))?;
    let (rollout, _) = write_rollout(&directory, Some(parent_end))?;

    let attachments = history_base_attachments(
        home.path(),
        &[FeedbackAttachmentPath {
            path: rollout,
            attachment_filename_override: None,
        }],
    )
    .await;

    let expected = [parent, grandparent]
        .into_iter()
        .map(|path| {
            Ok((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(path)?,
            ))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(
        attachments
            .into_iter()
            .map(|attachment| (attachment.filename, attachment.buffer))
            .collect::<Vec<_>>(),
        expected
    );
    Ok(())
}

fn write_rollout(
    directory: &Path,
    base: Option<HistoryPosition>,
) -> anyhow::Result<(PathBuf, HistoryPosition)> {
    let id = ThreadId::new();
    let ordinal = base.map_or(0, |base| base.end_ordinal_exclusive);
    let path = directory.join(format!("rollout-2026-09-22T12-00-00-{id}.jsonl"));
    let meta = json!({
        "ordinal": ordinal,
        "timestamp": "2026-09-22T12:00:00Z", "type": "session_meta",
        "payload": {
            "id": id, "timestamp": "2026-09-22T12:00:00Z", "cwd": directory,
            "originator": "test", "cli_version": "test", "history_mode": "paginated", "history_base": base,
        },
    });
    let contents = format!("{meta}\n");
    std::fs::write(&path, &contents)?;
    Ok((
        path,
        HistoryPosition {
            thread_id: id,
            end_ordinal_exclusive: ordinal + 1,
            end_byte_offset: contents.len() as u64,
        },
    ))
}
