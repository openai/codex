use askama::Template;
use std::path::Path;
use tracing::warn;

use super::text::prefix_at_char_boundary;
use super::text::suffix_at_char_boundary;

const MAX_ROLLOUT_BYTES_FOR_PROMPT: usize = 1_000_000;

#[derive(Template)]
#[template(path = "memories/consolidation.md", escape = "none")]
struct ConsolidationPromptTemplate<'a> {
    memory_root: &'a str,
}

#[derive(Template)]
#[template(path = "memories/stage_one_input.md", escape = "none")]
struct StageOneInputTemplate<'a> {
    rollout_path: &'a str,
    rollout_contents: &'a str,
}

/// Builds the consolidation subagent prompt for a specific memory root.
///
/// Falls back to a simple string replacement if Askama rendering fails.
pub(super) fn build_consolidation_prompt(memory_root: &Path) -> String {
    let memory_root = memory_root.display().to_string();
    let template = ConsolidationPromptTemplate {
        memory_root: &memory_root,
    };
    match template.render() {
        Ok(prompt) => prompt,
        Err(err) => {
            warn!("failed to render memories consolidation prompt template: {err}");
            include_str!("../../templates/memories/consolidation.md")
                .replace("{{ memory_root }}", &memory_root)
        }
    }
}

/// Builds the stage-1 user message containing rollout metadata and content.
///
/// Large rollout payloads are truncated to a bounded byte budget while keeping
/// both head and tail context.
pub(super) fn build_stage_one_input_message(rollout_path: &Path, rollout_contents: &str) -> String {
    let (rollout_contents, truncated) = truncate_rollout_for_prompt(rollout_contents);
    if truncated {
        warn!(
            "truncated rollout {} for stage-1 memory prompt to {} bytes",
            rollout_path.display(),
            MAX_ROLLOUT_BYTES_FOR_PROMPT
        );
    }

    let rollout_path = rollout_path.display().to_string();
    let template = StageOneInputTemplate {
        rollout_path: &rollout_path,
        rollout_contents: &rollout_contents,
    };
    match template.render() {
        Ok(prompt) => prompt,
        Err(err) => {
            warn!("failed to render memories stage-one input template: {err}");
            include_str!("../../templates/memories/stage_one_input.md")
                .replace("{{ rollout_path }}", &rollout_path)
                .replace("{{ rollout_contents }}", &rollout_contents)
        }
    }
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
