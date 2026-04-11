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
        rollout_path: PathBuf::from("/tmp/rollout.jsonl"),
        cwd: PathBuf::from("/tmp/workspace"),
        git_branch: None,
        generated_at: Utc.timestamp_opt(124, 0).single().expect("timestamp"),
    }
}

fn fixed_thread_id() -> ThreadId {
    ThreadId::try_from("0194f5a6-89ab-7cde-8123-456789abcdef").expect("valid thread id")
}

#[test]
fn rollout_summary_file_stem_uses_uuid_timestamp_and_hash_when_slug_missing() {
    let thread_id = fixed_thread_id();
    let memory = stage1_output_with_slug(thread_id, /*rollout_slug*/ None);

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
