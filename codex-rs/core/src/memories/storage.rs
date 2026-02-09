use codex_state::ThreadMemory;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;
use std::path::PathBuf;
use tracing::warn;

use super::LEGACY_CONSOLIDATED_FILENAME;
use super::MAX_TRACES_PER_CWD;
use super::MEMORY_REGISTRY_FILENAME;
use super::SKILLS_SUBDIR;
use super::ensure_layout;
use super::memory_summary_file;
use super::trace_summaries_dir;
use super::types::RolloutCandidate;

/// Writes (or replaces) the per-thread markdown trace summary on disk.
///
/// This also removes older files for the same thread id to keep one canonical
/// trace summary file per thread.
pub(crate) async fn write_trace_memory(
    root: &Path,
    candidate: &RolloutCandidate,
    trace_memory: &str,
) -> std::io::Result<PathBuf> {
    let slug = build_trace_slug(&candidate.title);
    let filename = format!("{}_{}.md", candidate.thread_id, slug);
    let path = trace_summaries_dir(root).join(filename);

    remove_outdated_thread_trace_summaries(root, &candidate.thread_id.to_string(), &path).await?;

    let mut body = String::new();
    writeln!(body, "thread_id: {}", candidate.thread_id)
        .map_err(|err| std::io::Error::other(format!("format trace memory: {err}")))?;
    writeln!(body, "cwd: {}", candidate.cwd.display())
        .map_err(|err| std::io::Error::other(format!("format trace memory: {err}")))?;
    writeln!(body, "rollout_path: {}", candidate.rollout_path.display())
        .map_err(|err| std::io::Error::other(format!("format trace memory: {err}")))?;
    if let Some(updated_at) = candidate.updated_at.as_deref() {
        writeln!(body, "updated_at: {updated_at}")
            .map_err(|err| std::io::Error::other(format!("format trace memory: {err}")))?;
    }
    writeln!(body).map_err(|err| std::io::Error::other(format!("format trace memory: {err}")))?;
    body.push_str(trace_memory.trim());
    body.push('\n');

    tokio::fs::write(&path, body).await?;
    Ok(path)
}

/// Prunes stale trace files and rebuilds the routing summary for recent traces.
pub(crate) async fn prune_to_recent_traces_and_rebuild_summary(
    root: &Path,
    memories: &[ThreadMemory],
) -> std::io::Result<()> {
    ensure_layout(root).await?;

    let keep = memories
        .iter()
        .take(MAX_TRACES_PER_CWD)
        .map(|memory| memory.thread_id.to_string())
        .collect::<BTreeSet<_>>();

    prune_trace_summaries(root, &keep).await?;
    rebuild_memory_summary(root, memories).await
}

/// Clears consolidation outputs so a fresh consolidation run can regenerate them.
///
/// Phase-1 artifacts (`trace_summaries/` and `memory_summary.md`) are preserved.
pub(crate) async fn wipe_consolidation_outputs(root: &Path) -> std::io::Result<()> {
    for file_name in [MEMORY_REGISTRY_FILENAME, LEGACY_CONSOLIDATED_FILENAME] {
        let path = root.join(file_name);
        if let Err(err) = tokio::fs::remove_file(&path).await
            && err.kind() != std::io::ErrorKind::NotFound
        {
            warn!(
                "failed removing consolidation file {}: {err}",
                path.display()
            );
        }
    }

    let skills_dir = root.join(SKILLS_SUBDIR);
    if let Err(err) = tokio::fs::remove_dir_all(&skills_dir).await
        && err.kind() != std::io::ErrorKind::NotFound
    {
        warn!(
            "failed removing consolidation skills directory {}: {err}",
            skills_dir.display()
        );
    }

    Ok(())
}

async fn rebuild_memory_summary(root: &Path, memories: &[ThreadMemory]) -> std::io::Result<()> {
    let mut body = String::from("# Memory Summary\n\n");

    if memories.is_empty() {
        body.push_str("No memory traces yet.\n");
        return tokio::fs::write(memory_summary_file(root), body).await;
    }

    body.push_str("Map of concise summaries to trace IDs (latest first):\n\n");
    for memory in memories.iter().take(MAX_TRACES_PER_CWD) {
        let summary = compact_summary_for_index(&memory.memory_summary);
        writeln!(body, "- {summary} (trace: `{}`)", memory.thread_id)
            .map_err(|err| std::io::Error::other(format!("format memory summary: {err}")))?;
    }

    tokio::fs::write(memory_summary_file(root), body).await
}

async fn prune_trace_summaries(root: &Path, keep: &BTreeSet<String>) -> std::io::Result<()> {
    let dir_path = trace_summaries_dir(root);
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
        let Some(trace_id) = extract_trace_id_from_summary_filename(file_name) else {
            continue;
        };
        if !keep.contains(trace_id)
            && let Err(err) = tokio::fs::remove_file(&path).await
            && err.kind() != std::io::ErrorKind::NotFound
        {
            warn!(
                "failed pruning outdated trace summary {}: {err}",
                path.display()
            );
        }
    }

    Ok(())
}

async fn remove_outdated_thread_trace_summaries(
    root: &Path,
    thread_id: &str,
    keep_path: &Path,
) -> std::io::Result<()> {
    let dir_path = trace_summaries_dir(root);
    let mut dir = match tokio::fs::read_dir(&dir_path).await {
        Ok(dir) => dir,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };

    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if path == keep_path {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(existing_thread_id) = extract_trace_id_from_summary_filename(file_name) else {
            continue;
        };
        if existing_thread_id == thread_id
            && let Err(err) = tokio::fs::remove_file(&path).await
            && err.kind() != std::io::ErrorKind::NotFound
        {
            warn!(
                "failed removing outdated trace summary {}: {err}",
                path.display()
            );
        }
    }

    Ok(())
}

fn build_trace_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_sep = false;

    for ch in value.chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            slug.push(normalized);
            last_was_sep = false;
        } else if !last_was_sep {
            slug.push('_');
            last_was_sep = true;
        }
    }

    let slug = slug.trim_matches('_').to_string();
    if slug.is_empty() {
        "trace".to_string()
    } else {
        slug.chars().take(64).collect()
    }
}

fn compact_summary_for_index(summary: &str) -> String {
    summary.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_trace_id_from_summary_filename(file_name: &str) -> Option<&str> {
    let stem = file_name.strip_suffix(".md")?;
    let (trace_id, _) = stem.split_once('_')?;
    if trace_id.is_empty() {
        None
    } else {
        Some(trace_id)
    }
}
