use codex_state::Stage1Output;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;
use tracing::warn;

use crate::memories::ensure_layout;
use crate::memories::phase_two;
use crate::memories::raw_memories_file;
use crate::memories::rollout_summaries_dir;

//TODO(jif) clean.

/// Rebuild `raw_memories.md` from DB-backed stage-1 outputs.
pub(super) async fn rebuild_raw_memories_file_from_memories(
    root: &Path,
    memories: &[Stage1Output],
) -> std::io::Result<()> {
    ensure_layout(root).await?;
    rebuild_raw_memories_file(root, memories).await
}

/// Syncs canonical rollout summary files from DB-backed stage-1 output rows.
pub(super) async fn sync_rollout_summaries_from_memories(
    root: &Path,
    memories: &[Stage1Output],
) -> std::io::Result<()> {
    ensure_layout(root).await?;

    let retained = memories
        .iter()
        .take(phase_two::MAX_RAW_MEMORIES_FOR_GLOBAL)
        .collect::<Vec<_>>();
    let keep = retained
        .iter()
        .map(|memory| memory.thread_id.to_string())
        .collect::<BTreeSet<_>>();
    prune_rollout_summaries(root, &keep).await?;

    for memory in &retained {
        write_rollout_summary_for_thread(root, memory).await?;
    }

    if retained.is_empty() {
        for file_name in ["MEMORY.md", "memory_summary.md"] {
            let path = root.join(file_name);
            if let Err(err) = tokio::fs::remove_file(path).await
                && err.kind() != std::io::ErrorKind::NotFound
            {
                return Err(err);
            }
        }

        let skills_dir = root.join("skills");
        if let Err(err) = tokio::fs::remove_dir_all(skills_dir).await
            && err.kind() != std::io::ErrorKind::NotFound
        {
            return Err(err);
        }
    }

    Ok(())
}

async fn rebuild_raw_memories_file(root: &Path, memories: &[Stage1Output]) -> std::io::Result<()> {
    let retained = memories
        .iter()
        .take(phase_two::MAX_RAW_MEMORIES_FOR_GLOBAL)
        .collect::<Vec<_>>();
    let mut body = String::from("# Raw Memories\n\n");

    if retained.is_empty() {
        body.push_str("No raw memories yet.\n");
        return tokio::fs::write(raw_memories_file(root), body).await;
    }

    body.push_str("Merged stage-1 raw memories (latest first):\n\n");
    for memory in retained {
        writeln!(body, "## Thread `{}`", memory.thread_id)
            .map_err(|err| std::io::Error::other(format!("format raw memories: {err}")))?;
        writeln!(
            body,
            "updated_at: {}",
            memory.source_updated_at.to_rfc3339()
        )
        .map_err(|err| std::io::Error::other(format!("format raw memories: {err}")))?;
        writeln!(body, "cwd: {}", memory.cwd.display())
            .map_err(|err| std::io::Error::other(format!("format raw memories: {err}")))?;
        writeln!(body)
            .map_err(|err| std::io::Error::other(format!("format raw memories: {err}")))?;
        body.push_str(memory.raw_memory.trim());
        body.push_str("\n\n");
    }

    tokio::fs::write(raw_memories_file(root), body).await
}

async fn prune_rollout_summaries(root: &Path, keep: &BTreeSet<String>) -> std::io::Result<()> {
    let dir_path = rollout_summaries_dir(root);
    let mut dir = match tokio::fs::read_dir(&dir_path).await {
        Ok(dir) => dir,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };

    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(thread_id) = extract_thread_id_from_rollout_summary_filename(file_name) else {
            continue;
        };
        if !keep.contains(thread_id)
            && let Err(err) = tokio::fs::remove_file(&path).await
            && err.kind() != std::io::ErrorKind::NotFound
        {
            warn!(
                "failed pruning outdated rollout summary {}: {err}",
                path.display()
            );
        }
    }

    Ok(())
}

async fn write_rollout_summary_for_thread(
    root: &Path,
    memory: &Stage1Output,
) -> std::io::Result<()> {
    let path = rollout_summaries_dir(root).join(format!("{}.md", memory.thread_id));

    let mut body = String::new();
    writeln!(body, "thread_id: {}", memory.thread_id)
        .map_err(|err| std::io::Error::other(format!("format rollout summary: {err}")))?;
    writeln!(
        body,
        "updated_at: {}",
        memory.source_updated_at.to_rfc3339()
    )
    .map_err(|err| std::io::Error::other(format!("format rollout summary: {err}")))?;
    writeln!(body, "cwd: {}", memory.cwd.display())
        .map_err(|err| std::io::Error::other(format!("format rollout summary: {err}")))?;
    writeln!(body)
        .map_err(|err| std::io::Error::other(format!("format rollout summary: {err}")))?;
    body.push_str(&memory.rollout_summary);
    body.push('\n');

    tokio::fs::write(path, body).await
}

fn extract_thread_id_from_rollout_summary_filename(file_name: &str) -> Option<&str> {
    let stem = file_name.strip_suffix(".md")?;
    if stem.is_empty() { None } else { Some(stem) }
}
