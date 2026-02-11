use super::text::prefix_at_char_boundary;
use super::text::suffix_at_char_boundary;
use crate::memories::memory_root;
use std::path::Path;
use tokio::fs;
use tracing::error;
use tracing::warn;

// TODO(jif) use proper truncation
const MAX_ROLLOUT_BYTES_FOR_PROMPT: usize = 100_000;
const CONSOLIDATION_PROMPT_TEMPLATE: &str =
    include_str!("../../templates/memories/consolidation.md");
const STAGE_ONE_INPUT_TEMPLATE: &str = include_str!("../../templates/memories/stage_one_input.md");
const READ_PATH_TEMPLATE: &str = include_str!("../../templates/memories/read_path.md");

/// Builds the consolidation subagent prompt for a specific memory root.
///
pub(super) fn build_consolidation_prompt(memory_root: &Path) -> String {
    let memory_root = memory_root.display().to_string();
    render_template(
        CONSOLIDATION_PROMPT_TEMPLATE,
        &[("memory_root", memory_root.as_str())],
    )
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
    render_template(
        STAGE_ONE_INPUT_TEMPLATE,
        &[
            ("rollout_path", rollout_path.as_str()),
            ("rollout_cwd", rollout_cwd.as_str()),
            ("rollout_contents", rollout_contents.as_str()),
        ],
    )
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
    Some(render_template(
        READ_PATH_TEMPLATE,
        &[
            ("base_path", base_path.as_str()),
            ("memory_summary", memory_summary.as_str()),
        ],
    ))
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

fn render_template(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut rendered = template.to_string();
    for (key, value) in replacements {
        let placeholder = format!("{{{{ {key} }}}}");
        rendered = rendered.replace(&placeholder, value);
    }

    if rendered.contains("{{") {
        error!(
            "unresolved template placeholders after memory prompt rendering; template may have changed"
        );
    }

    rendered
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
