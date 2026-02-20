use codex_state::Stage1Output;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;
use tracing::warn;
use uuid::Uuid;

use crate::memories::ensure_layout;
use crate::memories::raw_memories_file;
use crate::memories::rollout_summaries_dir;

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

    let retained = retained_memories(memories, max_raw_memories_for_global);
    let keep = retained
        .iter()
        .map(rollout_summary_file_stem)
        .collect::<HashSet<_>>();
    prune_rollout_summaries(root, &keep).await?;

    for memory in retained {
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
    let retained = retained_memories(memories, max_raw_memories_for_global);
    let mut body = String::from("# Raw Memories\n\n");

    if retained.is_empty() {
        body.push_str("No raw memories yet.\n");
        return tokio::fs::write(raw_memories_file(root), body).await;
    }

    body.push_str("Merged stage-1 raw memories (latest first):\n\n");
    for memory in retained {
        writeln!(body, "## Thread `{}`", memory.thread_id).map_err(raw_memories_format_error)?;
        writeln!(
            body,
            "updated_at: {}",
            memory.source_updated_at.to_rfc3339()
        )
        .map_err(raw_memories_format_error)?;
        writeln!(body, "cwd: {}", memory.cwd.display()).map_err(raw_memories_format_error)?;
        writeln!(body).map_err(raw_memories_format_error)?;
        let rollout_summary_file = format!("{}.md", rollout_summary_file_stem(memory));
        let raw_memory =
            replace_rollout_summary_file_in_raw_memory(&memory.raw_memory, &rollout_summary_file);
        body.push_str(raw_memory.trim());
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
) -> std::io::Result<()> {
    let file_stem = rollout_summary_file_stem(memory);
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

fn replace_rollout_summary_file_in_raw_memory(
    raw_memory: &str,
    rollout_summary_file: &str,
) -> String {
    const ROLLOUT_SUMMARY_PREFIX: &str = "rollout_summary_file: ";

    let replacement = format!("rollout_summary_file: {rollout_summary_file}");
    raw_memory
        .split('\n')
        .map(|line| {
            if line.starts_with(ROLLOUT_SUMMARY_PREFIX) {
                replacement.as_str()
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn rollout_summary_file_stem(memory: &Stage1Output) -> String {
    rollout_summary_file_stem_from_parts(
        memory.thread_id,
        memory.source_updated_at,
        memory.rollout_slug.as_deref(),
    )
}

pub(super) fn rollout_summary_file_stem_from_parts(
    thread_id: codex_protocol::ThreadId,
    source_updated_at: chrono::DateTime<chrono::Utc>,
    rollout_slug: Option<&str>,
) -> String {
    const ROLLOUT_SLUG_MAX_LEN: usize = 60;
    const SHORT_HASH_ALPHABET: &[u8; 62] =
        b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const SHORT_HASH_SPACE: u32 = 14_776_336;

    let thread_id = thread_id.to_string();
    let (timestamp_fragment, short_hash_seed) = match Uuid::parse_str(&thread_id) {
        Ok(thread_uuid) => {
            let timestamp = thread_uuid
                .get_timestamp()
                .and_then(|uuid_timestamp| {
                    let (seconds, nanos) = uuid_timestamp.to_unix();
                    i64::try_from(seconds).ok().and_then(|secs| {
                        chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nanos)
                    })
                })
                .unwrap_or(source_updated_at);
            let short_hash_seed = (thread_uuid.as_u128() & 0xFFFF_FFFF) as u32;
            (
                timestamp.format("%Y-%m-%dT%H-%M-%S").to_string(),
                short_hash_seed,
            )
        }
        Err(_) => {
            let mut short_hash_seed = 0u32;
            for byte in thread_id.bytes() {
                short_hash_seed = short_hash_seed
                    .wrapping_mul(31)
                    .wrapping_add(u32::from(byte));
            }
            (
                source_updated_at.format("%Y-%m-%dT%H-%M-%S").to_string(),
                short_hash_seed,
            )
        }
    };
    let mut short_hash_value = short_hash_seed % SHORT_HASH_SPACE;
    let mut short_hash_chars = ['0'; 4];
    for idx in (0..short_hash_chars.len()).rev() {
        let alphabet_idx = (short_hash_value % SHORT_HASH_ALPHABET.len() as u32) as usize;
        short_hash_chars[idx] = SHORT_HASH_ALPHABET[alphabet_idx] as char;
        short_hash_value /= SHORT_HASH_ALPHABET.len() as u32;
    }
    let short_hash: String = short_hash_chars.iter().collect();
    let file_prefix = format!("{timestamp_fragment}-{short_hash}");

    let Some(raw_slug) = rollout_slug else {
        return file_prefix;
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
        file_prefix
    } else {
        format!("{file_prefix}-{slug}")
    }
}

#[cfg(test)]
mod tests {
    use super::replace_rollout_summary_file_in_raw_memory;
    use super::rollout_summary_file_stem;
    use super::rollout_summary_file_stem_from_parts;
    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_state::Stage1Output;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    const FIXED_PREFIX: &str = "2025-02-11T15-35-19-jqmb";

    fn stage1_output_with_slug(thread_id: ThreadId, rollout_slug: Option<&str>) -> Stage1Output {
        Stage1Output {
            thread_id,
            source_updated_at: Utc.timestamp_opt(123, 0).single().expect("timestamp"),
            raw_memory: "raw memory".to_string(),
            rollout_summary: "summary".to_string(),
            rollout_slug: rollout_slug.map(ToString::to_string),
            cwd: PathBuf::from("/tmp/workspace"),
            generated_at: Utc.timestamp_opt(124, 0).single().expect("timestamp"),
        }
    }

    fn fixed_thread_id() -> ThreadId {
        ThreadId::try_from("0194f5a6-89ab-7cde-8123-456789abcdef").expect("valid thread id")
    }

    #[test]
    fn rollout_summary_file_stem_uses_uuid_timestamp_and_hash_when_slug_missing() {
        let thread_id = fixed_thread_id();
        let memory = stage1_output_with_slug(thread_id, None);

        assert_eq!(rollout_summary_file_stem(&memory), FIXED_PREFIX);
        assert_eq!(
            rollout_summary_file_stem_from_parts(
                memory.thread_id,
                memory.source_updated_at,
                memory.rollout_slug.as_deref(),
            ),
            FIXED_PREFIX
        );
    }

    #[test]
    fn rollout_summary_file_stem_sanitizes_and_truncates_slug() {
        let thread_id = fixed_thread_id();
        let memory = stage1_output_with_slug(
            thread_id,
            Some("Unsafe Slug/With Spaces & Symbols + EXTRA_LONG_12345_67890_ABCDE_fghij_klmno"),
        );

        let stem = rollout_summary_file_stem(&memory);
        let slug = stem
            .strip_prefix(&format!("{FIXED_PREFIX}-"))
            .expect("slug suffix should be present");
        assert_eq!(slug.len(), 60);
        assert_eq!(
            slug,
            "unsafe_slug_with_spaces___symbols___extra_long_12345_67890_a"
        );
    }

    #[test]
    fn rollout_summary_file_stem_uses_uuid_timestamp_and_hash_when_slug_is_empty() {
        let thread_id = fixed_thread_id();
        let memory = stage1_output_with_slug(thread_id, Some(""));

        assert_eq!(rollout_summary_file_stem(&memory), FIXED_PREFIX);
    }

    #[test]
    fn replace_rollout_summary_file_in_raw_memory_replaces_existing_value() {
        let raw_memory = "\
---
rollout_summary_file: wrong.md
description: demo
keywords: one, two
---
- body line";
        let normalized = replace_rollout_summary_file_in_raw_memory(
            raw_memory,
            "2025-01-01T00-00-00-abcd-demo.md",
        );

        assert_eq!(
            normalized,
            "\
---
rollout_summary_file: 2025-01-01T00-00-00-abcd-demo.md
description: demo
keywords: one, two
---
- body line"
        );
    }

    #[test]
    fn replace_rollout_summary_file_in_raw_memory_replaces_placeholder() {
        let raw_memory = "\
---
rollout_summary_file: <system_populated_file.md>
description: demo
keywords: one, two
---
- body line";
        let normalized = replace_rollout_summary_file_in_raw_memory(
            raw_memory,
            "2025-01-01T00-00-00-abcd-demo.md",
        );

        assert_eq!(
            normalized,
            "\
---
rollout_summary_file: 2025-01-01T00-00-00-abcd-demo.md
description: demo
keywords: one, two
---
- body line"
        );
    }

    #[test]
    fn replace_rollout_summary_file_in_raw_memory_leaves_text_without_field_unchanged() {
        let raw_memory = "\
---
description: demo
keywords: one, two
---
- body line";
        let normalized = replace_rollout_summary_file_in_raw_memory(
            raw_memory,
            "2025-01-01T00-00-00-abcd-demo.md",
        );
        assert_eq!(normalized, raw_memory);
    }
}
