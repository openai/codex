//! Best-effort classification of shell reads by memory artifact and root version.

use codex_protocol::MemoryVersion;
use codex_protocol::parse_command::ParsedCommand;
use codex_shell_command::parse_command::parse_shell_script;

pub use crate::metrics::MEMORIES_USAGE_METRIC;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoriesUsageKind {
    MemoryMd,
    MemorySummary,
    RawMemories,
    RolloutSummaries,
    Skills,
}

impl MemoriesUsageKind {
    pub fn as_tag(self) -> &'static str {
        match self {
            Self::MemoryMd => "memory_md",
            Self::MemorySummary => "memory_summary",
            Self::RawMemories => "raw_memories",
            Self::RolloutSummaries => "rollout_summaries",
            Self::Skills => "skills",
        }
    }
}

pub fn memories_usage_from_command(command: &str) -> Vec<(MemoriesUsageKind, MemoryVersion)> {
    let commands = parse_shell_script(command);
    if commands
        .iter()
        .any(|command| matches!(command, ParsedCommand::Unknown { .. }))
    {
        return Vec::new();
    }

    commands
        .into_iter()
        .filter_map(|command| match command {
            ParsedCommand::Read { path, .. } => get_memory_usage(path.display().to_string()),
            ParsedCommand::Search { path, .. } => path.and_then(get_memory_usage),
            ParsedCommand::ListFiles { .. } | ParsedCommand::Unknown { .. } => None,
        })
        .collect()
}

fn get_memory_usage(path: String) -> Option<(MemoriesUsageKind, MemoryVersion)> {
    let path = path.replace('\\', "/");
    let version = if path.contains("memories_v2/") {
        MemoryVersion::V2
    } else {
        MemoryVersion::V1
    };
    let path = path.replace("memories_v2/", "memories/");
    let kind = if path.contains("memories/MEMORY.md") {
        MemoriesUsageKind::MemoryMd
    } else if path.contains("memories/memory_summary.md") {
        MemoriesUsageKind::MemorySummary
    } else if path.contains("memories/raw_memories.md") {
        MemoriesUsageKind::RawMemories
    } else if path.contains("memories/rollout_summaries/") {
        MemoriesUsageKind::RolloutSummaries
    } else if path.contains("memories/skills/") {
        MemoriesUsageKind::Skills
    } else {
        return None;
    };
    Some((kind, version))
}

#[cfg(test)]
#[path = "usage_tests.rs"]
mod tests;
