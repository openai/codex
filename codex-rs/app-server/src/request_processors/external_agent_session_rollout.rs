use std::fs::FileTimes;
use std::fs::OpenOptions;
use std::io;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use chrono::DateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_git_utils::collect_git_info;
use codex_protocol::protocol::GitInfo as ProtocolGitInfo;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_rollout::SESSIONS_SUBDIR;

pub(super) async fn materialize_imported_rollout(
    codex_home: &Path,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    session_meta: SessionMeta,
    rollout_items: Vec<RolloutItem>,
) -> io::Result<PathBuf> {
    let git = collect_git_info(session_meta.cwd.as_path())
        .await
        .map(|info| ProtocolGitInfo {
            commit_hash: info.commit_hash,
            branch: info.branch,
            repository_url: info.repository_url,
        });
    let codex_home = codex_home.to_path_buf();
    tokio::task::spawn_blocking(move || {
        write_imported_rollout(
            codex_home.as_path(),
            created_at,
            updated_at,
            SessionMetaLine {
                meta: session_meta,
                git,
            },
            rollout_items,
        )
    })
    .await
    .map_err(|err| io::Error::other(format!("imported rollout task failed: {err}")))?
}

fn write_imported_rollout(
    codex_home: &Path,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    session_meta: SessionMetaLine,
    rollout_items: Vec<RolloutItem>,
) -> io::Result<PathBuf> {
    let rollout_dir = codex_home
        .join(SESSIONS_SUBDIR)
        .join(created_at.format("%Y").to_string())
        .join(created_at.format("%m").to_string())
        .join(created_at.format("%d").to_string());
    std::fs::create_dir_all(rollout_dir.as_path())?;
    let filename_timestamp = created_at.format("%Y-%m-%dT%H-%M-%S");
    let rollout_path = rollout_dir.join(format!(
        "rollout-{filename_timestamp}-{}.jsonl",
        session_meta.meta.id
    ));
    if rollout_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "imported rollout already exists: {}",
                rollout_path.display()
            ),
        ));
    }
    let temporary_path = rollout_path.with_extension("jsonl.tmp");
    let mut renamed = false;
    let result = (|| {
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(temporary_path.as_path())?;
        let mut writer = BufWriter::new(file);
        write_rollout_line(
            &mut writer,
            created_at.to_rfc3339_opts(SecondsFormat::Millis, true),
            RolloutItem::SessionMeta(session_meta),
        )?;
        let item_timestamp = updated_at.to_rfc3339_opts(SecondsFormat::Millis, true);
        for item in rollout_items {
            write_rollout_line(&mut writer, item_timestamp.clone(), item)?;
        }
        writer.flush()?;
        writer.get_ref().sync_all()?;
        drop(writer);
        std::fs::rename(temporary_path.as_path(), rollout_path.as_path())?;
        renamed = true;
        OpenOptions::new()
            .write(true)
            .open(rollout_path.as_path())?
            .set_times(FileTimes::new().set_modified(updated_at.into()))?;
        Ok(())
    })();
    if let Err(err) = result {
        let _ = std::fs::remove_file(temporary_path);
        if renamed {
            let _ = std::fs::remove_file(rollout_path);
        }
        return Err(err);
    }
    Ok(rollout_path)
}

fn write_rollout_line(
    writer: &mut BufWriter<std::fs::File>,
    timestamp: String,
    item: RolloutItem,
) -> io::Result<()> {
    serde_json::to_writer(writer.by_ref(), &RolloutLine { timestamp, item })
        .map_err(io::Error::other)?;
    writer.write_all(b"\n")
}
