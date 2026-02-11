use super::text::prefix_at_char_boundary;
use super::text::suffix_at_char_boundary;
use crate::memories::memory_root;
use askama::Template;
use std::path::Path;
use tokio::fs;
use tracing::warn;

// TODO(jif) use proper truncation
const MAX_ROLLOUT_BYTES_FOR_PROMPT: usize = 100_000;

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
#[template(path = "memory_tool/developer_instructions.md", escape = "none")]
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
) -> String {
    let (rollout_contents, truncated) = truncate_rollout_for_prompt(rollout_contents);
    if truncated {
        warn!(
            "truncated rollout {} for stage-1 memory prompt to {} bytes",
            rollout_path.display(),
            MAX_ROLLOUT_BYTES_FOR_PROMPT
        );
    }

    let rollout_path = rollout_path.display().to_string();
    let rollout_cwd = rollout_cwd.display().to_string();
    let template = StageOneInputTemplate {
        rollout_path: &rollout_path,
        rollout_cwd: &rollout_cwd,
        rollout_contents: &rollout_contents,
    };
    template.render().unwrap_or_else(|err| {
        warn!("failed to render memories stage-one input template: {err}");
        format!(
            "Analyze this rollout and produce JSON with `raw_memory`, `rollout_summary`, and optional `rollout_slug`.\n\nrollout_context:\n- rollout_path: {rollout_path}\n- rollout_cwd: {rollout_cwd}\n\nrendered conversation:\n{rollout_contents}"
        )
    })
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

fn truncate_rollout_for_prompt(input: &str) -> (String, bool) {
    if input.len() <= MAX_ROLLOUT_BYTES_FOR_PROMPT {
        return (input.to_string(), false);
    }

    let marker = "\n\n[... ROLLOUT TRUNCATED FOR MEMORY EXTRACTION ...]\n\n";
    let marker_len = marker.len();
    let budget_without_marker = MAX_ROLLOUT_BYTES_FOR_PROMPT.saturating_sub(marker_len);
    let head_budget = budget_without_marker / 3;
    let tail_budget = budget_without_marker.saturating_sub(head_budget);
    let head = prefix_at_char_boundary(input, head_budget);
    let tail = suffix_at_char_boundary(input, tail_budget);
    let truncated = format!("{head}{marker}{tail}");

    (truncated, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_rollout_for_prompt_keeps_head_and_tail() {
        let input = format!("{}{}{}", "a".repeat(700_000), "middle", "z".repeat(700_000));
        let (truncated, was_truncated) = truncate_rollout_for_prompt(&input);

        assert!(was_truncated);
        assert!(truncated.contains("[... ROLLOUT TRUNCATED FOR MEMORY EXTRACTION ...]"));
        assert!(truncated.starts_with('a'));
        assert!(truncated.ends_with('z'));
        assert!(truncated.len() <= MAX_ROLLOUT_BYTES_FOR_PROMPT + 32);
    }
}
