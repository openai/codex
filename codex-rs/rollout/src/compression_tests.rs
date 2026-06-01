use std::fs;

use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use uuid::Uuid;

use super::*;
use crate::RolloutRecorder;

#[tokio::test]
async fn load_rollout_items_reads_compressed_rollout() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(1);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "hello compressed")?;
    compress_now(&rollout_path)?;

    let (items, loaded_thread_id, parse_errors) =
        RolloutRecorder::load_rollout_items(&rollout_path).await?;

    assert_eq!(loaded_thread_id, Some(thread_id));
    assert_eq!(parse_errors, 0);
    assert_eq!(items.len(), 2);
    assert!(!rollout_path.exists());
    assert!(compressed_rollout_path(&rollout_path).exists());
    Ok(())
}

#[test]
fn rollout_file_from_path_normalizes_compressed_file_names() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(7);
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    let compressed_path = compressed_rollout_path(&rollout_path);

    assert_eq!(
        RolloutFile::from_path(compressed_path.clone()),
        Some(RolloutFile {
            path: compressed_path,
            plain_file_name: format!("rollout-2025-01-03T12-00-00-{uuid}.jsonl"),
        })
    );
    Ok(())
}

#[test]
fn rollout_file_from_path_hides_compressed_sibling_when_plain_exists() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(8);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "plain wins")?;

    assert_eq!(
        RolloutFile::from_path(compressed_rollout_path(&rollout_path)),
        None
    );
    Ok(())
}

#[tokio::test]
async fn find_thread_path_by_id_handles_compressed_rollout_filenames() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(8);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "compressed filename lookup")?;
    compress_now(&rollout_path)?;
    let compressed_path = compressed_rollout_path(&rollout_path);

    assert_eq!(
        crate::find_thread_path_by_id_str(
            home.path(),
            &uuid.to_string(),
            /*state_db_ctx*/ None
        )
        .await?,
        Some(compressed_path)
    );
    assert_eq!(
        crate::find_thread_path_by_id_str(home.path(), "not-a-uuid", /*state_db_ctx*/ None).await?,
        None
    );
    Ok(())
}

#[tokio::test]
async fn find_thread_path_by_id_ignores_compression_temp_matches() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(9);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let temp_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid).with_file_name(format!(
        "rollout-2025-01-03T12-00-00-{uuid}.jsonl.zst.compress.1.0.tmp"
    ));
    write_rollout(&temp_path, thread_id, "temporary file should not resolve")?;

    assert_eq!(
        crate::find_thread_path_by_id_str(
            home.path(),
            &uuid.to_string(),
            /*state_db_ctx*/ None
        )
        .await?,
        None
    );
    Ok(())
}

fn rollout_path(home: &std::path::Path, ts: &str, uuid: Uuid) -> std::path::PathBuf {
    home.join("sessions/2025/01/03")
        .join(format!("rollout-{ts}-{uuid}.jsonl"))
}

fn write_rollout(path: &std::path::Path, thread_id: ThreadId, message: &str) -> anyhow::Result<()> {
    let parent = path.parent().expect("rollout path should have parent");
    fs::create_dir_all(parent)?;
    let session_meta_line = SessionMetaLine {
        meta: SessionMeta {
            id: thread_id,
            forked_from_id: None,
            timestamp: "2025-01-03T12:00:00Z".to_string(),
            cwd: parent.to_path_buf(),
            originator: "test".to_string(),
            cli_version: "test".to_string(),
            source: SessionSource::Cli,
            thread_source: None,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
            model_provider: None,
            base_instructions: None,
            dynamic_tools: None,
            memory_mode: None,
        },
        git: None,
    };
    let lines = [
        RolloutLine {
            timestamp: "2025-01-03T12:00:00Z".to_string(),
            item: RolloutItem::SessionMeta(session_meta_line),
        },
        RolloutLine {
            timestamp: "2025-01-03T12:00:01Z".to_string(),
            item: RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: message.to_string(),
                ..Default::default()
            })),
        },
    ];
    let jsonl = lines
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    fs::write(path, format!("{jsonl}\n"))?;
    Ok(())
}

fn compress_now(path: &std::path::Path) -> anyhow::Result<()> {
    let compressed_path = compressed_rollout_path(path);
    let input = fs::File::open(path)?;
    let output = fs::File::create(compressed_path)?;
    let mut encoder = zstd::stream::write::Encoder::new(output, 3)?;
    let mut input = std::io::BufReader::new(input);
    std::io::copy(&mut input, &mut encoder)?;
    encoder.finish()?;
    fs::remove_file(path)?;
    Ok(())
}
