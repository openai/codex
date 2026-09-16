//! Regression coverage for versioned memory paths in shell usage telemetry.

use super::MemoriesUsageKind;
use super::memories_usage_from_command;
use codex_protocol::MemoryVersion;
use pretty_assertions::assert_eq;

#[test]
fn classifies_reads_in_both_memory_versions() {
    for version in [MemoryVersion::V1, MemoryVersion::V2] {
        let root = version.directory_name();
        for (reader, root) in [
            ("cat", format!("/tmp/.codex/{root}")),
            ("Get-Content", format!(r"C:\Users\test\.codex\{root}")),
        ] {
            let command = format!(
                "{reader} '{root}/memory_summary.md' && {reader} '{root}/rollout_summaries/thread.md'"
            );
            assert_eq!(
                memories_usage_from_command(&command),
                vec![
                    (MemoriesUsageKind::MemorySummary, version),
                    (MemoriesUsageKind::RolloutSummaries, version),
                ],
                "memory root: {root}"
            );
        }
    }
}

#[test]
fn attributes_mixed_version_reads_to_their_roots() {
    let command = "cat /tmp/.codex/memories/MEMORY.md \
        && sed -n '1,20p' /tmp/.codex/memories_v2/rollout_summaries/thread.md \
        && cat /tmp/.codex/memories/raw_memories.md \
        && cat /tmp/.codex/memories_v2/skills/testing/SKILL.md";
    assert_eq!(
        memories_usage_from_command(command),
        vec![
            (MemoriesUsageKind::MemoryMd, MemoryVersion::V1),
            (MemoriesUsageKind::RolloutSummaries, MemoryVersion::V2),
            (MemoriesUsageKind::RawMemories, MemoryVersion::V1),
            (MemoriesUsageKind::Skills, MemoryVersion::V2),
        ],
    );
}
