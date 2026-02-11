use crate::memories::memory_root;
use crate::truncate::TruncationPolicy;
use crate::truncate::truncate_text;
use askama::Template;
use std::path::Path;
use tokio::fs;
use tracing::warn;

#[derive(Template)]
#[template(path = "memories/consolidation.md", escape = "none")]
struct ConsolidationPromptTemplate<'a> {
    memory_root: &'a str,
}

#[derive(Template)]
#[template(path = "memories/stage_one_input.md", escape = "none")]
struct StageOneInputTemplate<'a> {
    rollout_path: &'a str,
    rollout_cwd: &'a str,
    rollout_contents: &'a str,
}

#[derive(Template)]
#[template(path = "memories/read_path.md", escape = "none")]
struct MemoryToolDeveloperInstructionsTemplate<'a> {
    base_path: &'a str,
    memory_summary: &'a str,
}

/// Builds the consolidation subagent prompt for a specific memory root.
///
pub(super) fn build_consolidation_prompt(memory_root: &Path) -> String {
    let memory_root = memory_root.display().to_string();
    let template = ConsolidationPromptTemplate {
        memory_root: &memory_root,
    };
    template.render().unwrap_or_else(|err| {
        warn!("failed to render memories consolidation prompt template: {err}");
        format!("## Memory Phase 2 (Consolidation)\nConsolidate Codex memories in: {memory_root}")
    })
}

/// Builds the stage-1 user message containing rollout metadata and content.
///
/// Large rollout payloads are truncated to a bounded byte budget while keeping
/// both head and tail context.
pub(super) fn build_stage_one_input_message(
    rollout_path: &Path,
    rollout_cwd: &Path,
    rollout_contents: &str,
) -> anyhow::Result<String> {
    let truncated_rollout_contents =
        truncate_text(rollout_contents, TruncationPolicy::Tokens(150_000));

    let rollout_path = rollout_path.display().to_string();
    let rollout_cwd = rollout_cwd.display().to_string();
    Ok(StageOneInputTemplate {
        rollout_path: &rollout_path,
        rollout_cwd: &rollout_cwd,
        rollout_contents: &truncated_rollout_contents,
    }
    .render()?)
}

pub(crate) async fn build_memory_tool_developer_instructions(codex_home: &Path) -> Option<String> {
    let base_path = memory_root(codex_home);
    let memory_summary_path = base_path.join("memory_summary.md");
    let memory_summary = fs::read_to_string(&memory_summary_path)
        .await
        .ok()?
        .trim()
        .to_string();
    if memory_summary.is_empty() {
        return None;
    }
    let base_path = base_path.display().to_string();
    let template = MemoryToolDeveloperInstructionsTemplate {
        base_path: &base_path,
        memory_summary: &memory_summary,
    };
    template.render().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_stage_one_input_message_truncates_rollout_with_standard_policy() {
        let input = format!("{}{}{}", "a".repeat(700_000), "middle", "z".repeat(700_000));
        let expected_truncated = truncate_text(&input, TruncationPolicy::Tokens(150_000));
        let message = build_stage_one_input_message(
            Path::new("/tmp/rollout.jsonl"),
            Path::new("/tmp"),
            &input,
        )
        .unwrap();

        assert!(expected_truncated.contains("tokens truncated"));
        assert!(expected_truncated.starts_with('a'));
        assert!(expected_truncated.ends_with('z'));
        assert!(message.contains(&expected_truncated));
    }
}
