use codex_state::Stage1Output;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;
use tracing::warn;

use crate::memories::ensure_layout;
use crate::memories::raw_memories_file;
use crate::memories::rollout_summaries_dir;

//TODO(jif) clean.

/// Rebuild `raw_memories.md` from DB-backed stage-1 outputs.
pub(super) async fn rebuild_raw_memories_file_from_memories(
    root: &Path,
    memories: &[Stage1Output],
    max_raw_memories_for_global: usize,
) -> std::io::Result<()> {
    ensure_layout(root).await?;
    rebuild_raw_memories_file(root, memories, max_raw_memories_for_global).await
}

/// Syncs canonical rollout summary files from DB-backed stage-1 output rows.
pub(super) async fn sync_rollout_summaries_from_memories(
    root: &Path,
    memories: &[Stage1Output],
    max_raw_memories_for_global: usize,
) -> std::io::Result<()> {
    ensure_layout(root).await?;

    let retained = memories
        .iter()
        .take(max_raw_memories_for_global)
        .collect::<Vec<_>>();
    let keep = retained
        .iter()
        .map(|memory| rollout_summary_file_stem(memory))
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

async fn rebuild_raw_memories_file(
    root: &Path,
    memories: &[Stage1Output],
    max_raw_memories_for_global: usize,
) -> std::io::Result<()> {
    let retained = memories
        .iter()
        .take(max_raw_memories_for_global)
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
        let Some(stem) = file_name.strip_suffix(".md") else {
            continue;
        };
        if !keep.contains(stem)
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
    let file_stem = rollout_summary_file_stem(memory);
    let path = rollout_summaries_dir(root).join(format!("{file_stem}.md"));

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

fn rollout_summary_file_stem(memory: &Stage1Output) -> String {
    const ROLLOUT_SLUG_MAX_LEN: usize = 20;

    let thread_id = memory.thread_id.to_string();
    let Some(raw_slug) = memory.rollout_slug.as_deref() else {
        return thread_id;
    };

    let mut slug = String::with_capacity(ROLLOUT_SLUG_MAX_LEN);
    for ch in raw_slug.chars() {
        if slug.len() >= ROLLOUT_SLUG_MAX_LEN {
            break;
        }

        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else {
            slug.push('_');
        }
    }

    while slug.ends_with('_') {
        slug.pop();
    }

    if slug.is_empty() {
        thread_id
    } else {
        format!("{thread_id}-{slug}")
    }
}

#[cfg(test)]
mod tests {
    use super::rollout_summary_file_stem;
    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_state::Stage1Output;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    fn stage1_output_with_slug(rollout_slug: Option<&str>) -> Stage1Output {
        Stage1Output {
            thread_id: ThreadId::new(),
            source_updated_at: Utc.timestamp_opt(123, 0).single().expect("timestamp"),
            raw_memory: "raw memory".to_string(),
            rollout_summary: "summary".to_string(),
            rollout_slug: rollout_slug.map(ToString::to_string),
            cwd: PathBuf::from("/tmp/workspace"),
            generated_at: Utc.timestamp_opt(124, 0).single().expect("timestamp"),
        }
    }

    #[test]
    fn rollout_summary_file_stem_uses_thread_id_when_slug_missing() {
        let memory = stage1_output_with_slug(None);
        let thread_id = memory.thread_id.to_string();

        assert_eq!(rollout_summary_file_stem(&memory), thread_id);
    }

    #[test]
    fn rollout_summary_file_stem_sanitizes_and_truncates_slug() {
        let memory =
            stage1_output_with_slug(Some("Unsafe Slug/With Spaces & Symbols + EXTRA_LONG_12345"));
        let thread_id = memory.thread_id.to_string();

        assert_eq!(
            rollout_summary_file_stem(&memory),
            format!("{thread_id}-unsafe_slug_with_spa")
        );
    }

    #[test]
    fn rollout_summary_file_stem_uses_thread_id_when_slug_is_empty() {
        let memory = stage1_output_with_slug(Some(""));
        let thread_id = memory.thread_id.to_string();

        assert_eq!(rollout_summary_file_stem(&memory), thread_id);
    }
}
