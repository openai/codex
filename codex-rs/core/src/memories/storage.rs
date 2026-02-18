use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_state::Stage1Output;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;
use tracing::warn;

use crate::memories::ensure_layout;
use crate::memories::raw_memories_file;
use crate::memories::rollout_summaries_dir;
use crate::rollout::list::parse_timestamp_uuid_from_filename;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ResolvedRolloutSummary {
    pub(super) thread_id: ThreadId,
    pub(super) file_stem: String,
    pub(super) file_name: String,
}

/// Resolves rollout summary filenames for retained stage-1 outputs.
pub(super) fn resolve_rollout_summary_files(
    memories: &[Stage1Output],
    max_raw_memories_for_global: usize,
) -> Vec<ResolvedRolloutSummary> {
    let retained = retained_memories(memories, max_raw_memories_for_global);
    let mut base_name_counts = HashMap::<String, usize>::new();
    let mut resolved = Vec::with_capacity(retained.len());

    for memory in retained {
        let timestamp = if let Some(file_name) = memory
            .rollout_path
            .file_name()
            .and_then(|name| name.to_str())
            && let Some((parsed_timestamp, _)) = parse_timestamp_uuid_from_filename(file_name)
            && let Some(parsed) =
                DateTime::<Utc>::from_timestamp(parsed_timestamp.unix_timestamp(), 0)
        {
            parsed.format("%Y-%m-%dT%H-%M-%S").to_string()
        } else {
            memory
                .source_updated_at
                .format("%Y-%m-%dT%H-%M-%S")
                .to_string()
        };
        let slug = normalize_rollout_slug(memory.rollout_slug.as_deref());
        let base_stem = format!("{timestamp}-{slug}");

        let counter = base_name_counts.entry(base_stem.clone()).or_default();
        *counter += 1;
        let file_stem = if *counter == 1 {
            base_stem
        } else {
            format!("{base_stem}-{counter}")
        };
        let file_name = format!("{file_stem}.md");

        resolved.push(ResolvedRolloutSummary {
            thread_id: memory.thread_id,
            file_stem,
            file_name,
        });
    }

    resolved
}

/// Rebuild `raw_memories.md` from DB-backed stage-1 outputs.
pub(super) async fn rebuild_raw_memories_file_from_memories(
    root: &Path,
    memories: &[Stage1Output],
    resolved: &[ResolvedRolloutSummary],
) -> std::io::Result<()> {
    ensure_layout(root).await?;
    rebuild_raw_memories_file(root, memories, resolved).await
}

/// Syncs canonical rollout summary files from DB-backed stage-1 output rows.
pub(super) async fn sync_rollout_summaries_from_memories(
    root: &Path,
    memories: &[Stage1Output],
    resolved: &[ResolvedRolloutSummary],
) -> std::io::Result<()> {
    ensure_layout(root).await?;

    let keep = resolved
        .iter()
        .map(|item| item.file_stem.clone())
        .collect::<HashSet<_>>();
    prune_rollout_summaries(root, &keep).await?;

    let memory_by_thread = memories
        .iter()
        .map(|memory| (memory.thread_id.to_string(), memory))
        .collect::<HashMap<_, _>>();

    for item in resolved {
        let Some(memory) = memory_by_thread.get(&item.thread_id.to_string()) else {
            return Err(std::io::Error::other(format!(
                "missing stage1 output for thread {} while syncing rollout summaries",
                item.thread_id
            )));
        };
        write_rollout_summary_for_thread(root, memory, &item.file_stem).await?;
    }

    if resolved.is_empty() {
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
    resolved: &[ResolvedRolloutSummary],
) -> std::io::Result<()> {
    let mut body = String::from("# Raw Memories\n\n");

    if resolved.is_empty() {
        body.push_str("No raw memories yet.\n");
        return tokio::fs::write(raw_memories_file(root), body).await;
    }

    let memory_by_thread = memories
        .iter()
        .map(|memory| (memory.thread_id.to_string(), memory))
        .collect::<HashMap<_, _>>();

    body.push_str("Merged stage-1 raw memories (latest first):\n\n");
    for item in resolved {
        let Some(memory) = memory_by_thread.get(&item.thread_id.to_string()) else {
            return Err(std::io::Error::other(format!(
                "missing stage1 output for thread {} while rebuilding raw memories",
                item.thread_id
            )));
        };

        writeln!(body, "## Thread `{}`", memory.thread_id).map_err(raw_memories_format_error)?;
        writeln!(
            body,
            "updated_at: {}",
            memory.source_updated_at.to_rfc3339()
        )
        .map_err(raw_memories_format_error)?;
        writeln!(body, "cwd: {}", memory.cwd.display()).map_err(raw_memories_format_error)?;
        writeln!(body, "rollout_summary_file_name: {}", item.file_name)
            .map_err(raw_memories_format_error)?;
        writeln!(body).map_err(raw_memories_format_error)?;
        body.push_str(memory.raw_memory.trim());
        body.push_str("\n\n");
    }

    tokio::fs::write(raw_memories_file(root), body).await
}

async fn prune_rollout_summaries(root: &Path, keep: &HashSet<String>) -> std::io::Result<()> {
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
    file_stem: &str,
) -> std::io::Result<()> {
    let path = rollout_summaries_dir(root).join(format!("{file_stem}.md"));

    let mut body = String::new();
    writeln!(body, "thread_id: {}", memory.thread_id).map_err(rollout_summary_format_error)?;
    writeln!(
        body,
        "updated_at: {}",
        memory.source_updated_at.to_rfc3339()
    )
    .map_err(rollout_summary_format_error)?;
    writeln!(body, "cwd: {}", memory.cwd.display()).map_err(rollout_summary_format_error)?;
    writeln!(body).map_err(rollout_summary_format_error)?;
    body.push_str(&memory.rollout_summary);
    body.push('\n');

    tokio::fs::write(path, body).await
}

fn retained_memories(
    memories: &[Stage1Output],
    max_raw_memories_for_global: usize,
) -> &[Stage1Output] {
    &memories[..memories.len().min(max_raw_memories_for_global)]
}

fn raw_memories_format_error(err: std::fmt::Error) -> std::io::Error {
    std::io::Error::other(format!("format raw memories: {err}"))
}

fn rollout_summary_format_error(err: std::fmt::Error) -> std::io::Error {
    std::io::Error::other(format!("format rollout summary: {err}"))
}

fn normalize_rollout_slug(raw_slug: Option<&str>) -> String {
    const ROLLOUT_SLUG_MAX_LEN: usize = 60;

    let mut normalized = String::with_capacity(ROLLOUT_SLUG_MAX_LEN);
    for ch in raw_slug.unwrap_or_default().chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else if ch == '_' || ch == '-' {
            ch
        } else {
            '_'
        };

        let mapped_is_sep = mapped == '_' || mapped == '-';
        let prev_is_sep = normalized
            .chars()
            .last()
            .is_some_and(|previous| previous == '_' || previous == '-');
        if mapped_is_sep && prev_is_sep {
            continue;
        }

        normalized.push(mapped);
        if normalized.len() == ROLLOUT_SLUG_MAX_LEN {
            break;
        }
    }

    let trimmed = normalized
        .trim_matches(|ch| ch == '_' || ch == '-')
        .to_string();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_rollout_slug;
    use super::resolve_rollout_summary_files;
    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_state::Stage1Output;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    fn stage1_output_with_slug_and_path(
        rollout_slug: Option<&str>,
        rollout_path: &str,
    ) -> Stage1Output {
        Stage1Output {
            thread_id: ThreadId::new(),
            source_updated_at: Utc.timestamp_opt(123, 0).single().expect("timestamp"),
            raw_memory: "raw memory".to_string(),
            rollout_summary: "summary".to_string(),
            rollout_slug: rollout_slug.map(ToString::to_string),
            rollout_summary_filename: None,
            rollout_path: PathBuf::from(rollout_path),
            cwd: PathBuf::from("/tmp/workspace"),
            generated_at: Utc.timestamp_opt(124, 0).single().expect("timestamp"),
        }
    }

    #[test]
    fn normalize_rollout_slug_applies_capping_and_separator_rules() {
        let value = normalize_rollout_slug(Some(
            "--Unsafe Slug//With---Spaces&&Symbols____________________01234567890123456789012345",
        ));
        assert_eq!(
            value,
            "unsafe_slug_with-spaces_symbols_01234567890123456789012345"
        );
        assert!(value.len() <= 60);
    }

    #[test]
    fn normalize_rollout_slug_uses_unknown_for_empty_result() {
        assert_eq!(normalize_rollout_slug(Some("!!!")), "unknown");
        assert_eq!(normalize_rollout_slug(Some("")), "unknown");
        assert_eq!(normalize_rollout_slug(None), "unknown");
    }

    #[test]
    fn resolve_rollout_summary_files_uses_timestamp_and_suffixes_collisions() {
        let first = stage1_output_with_slug_and_path(
            Some("Unsafe Slug/With Spaces & Symbols"),
            "sessions/2026/02/17/rollout-2026-02-17T19-22-07-00000000-0000-0000-0000-000000000001.jsonl",
        );
        let second = Stage1Output {
            thread_id: ThreadId::new(),
            source_updated_at: Utc.timestamp_opt(124, 0).single().expect("timestamp"),
            raw_memory: "raw memory 2".to_string(),
            rollout_summary: "summary 2".to_string(),
            rollout_slug: Some("Unsafe Slug/With Spaces & Symbols".to_string()),
            rollout_summary_filename: None,
            rollout_path: PathBuf::from(
                "sessions/2026/02/17/rollout-2026-02-17T19-22-07-00000000-0000-0000-0000-000000000002.jsonl",
            ),
            cwd: PathBuf::from("/tmp/workspace"),
            generated_at: Utc.timestamp_opt(125, 0).single().expect("timestamp"),
        };

        let resolved = resolve_rollout_summary_files(&[first, second], 8);
        assert_eq!(resolved.len(), 2);
        assert_eq!(
            resolved[0].file_name,
            "2026-02-17T19-22-07-unsafe_slug_with_spaces_symbols.md"
        );
        assert_eq!(
            resolved[1].file_name,
            "2026-02-17T19-22-07-unsafe_slug_with_spaces_symbols-2.md"
        );
    }

    #[test]
    fn resolve_rollout_summary_files_falls_back_to_source_updated_at_when_rollout_timestamp_is_missing()
     {
        let memory =
            stage1_output_with_slug_and_path(Some("alpha"), "sessions/rollout-not-parseable.jsonl");
        let resolved = resolve_rollout_summary_files(&[memory], 8);

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].file_name, "1970-01-01T00-02-03-alpha.md");
    }
}
