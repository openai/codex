use crate::memories::memory_root;
use crate::memories::phase_one;
use crate::truncate::TruncationPolicy;
use crate::truncate::truncate_text;
use codex_protocol::openai_models::ModelInfo;
use std::path::Path;
use tokio::fs;

const CONSOLIDATION_TEMPLATE: &str = include_str!("../../templates/memories/consolidation.md");
const STAGE_ONE_INPUT_TEMPLATE: &str = include_str!("../../templates/memories/stage_one_input.md");
const READ_PATH_TEMPLATE: &str = include_str!("../../templates/memories/read_path.md");

/// Builds the consolidation subagent prompt for a specific memory root.
pub(super) fn build_consolidation_prompt(memory_root: &Path) -> String {
    let memory_root = memory_root.display().to_string();
    CONSOLIDATION_TEMPLATE.replace("{{ memory_root }}", &memory_root)
}

/// Builds the stage-1 user message containing rollout metadata and content.
///
/// Large rollout payloads are truncated to 70% of the active model's effective
/// input window token budget while keeping both head and tail context.
pub(super) fn build_stage_one_input_message(
    model_info: &ModelInfo,
    rollout_path: &Path,
    rollout_cwd: &Path,
    rollout_contents: &str,
) -> anyhow::Result<String> {
    let rollout_token_limit = model_info
        .context_window
        .and_then(|limit| (limit > 0).then_some(limit))
        .map(|limit| limit.saturating_mul(model_info.effective_context_window_percent) / 100)
        .map(|limit| (limit.saturating_mul(phase_one::CONTEXT_WINDOW_PERCENT) / 100).max(1))
        .and_then(|limit| usize::try_from(limit).ok())
        .unwrap_or(phase_one::DEFAULT_STAGE_ONE_ROLLOUT_TOKEN_LIMIT);
    let truncated_rollout_contents = truncate_text(
        rollout_contents,
        TruncationPolicy::Tokens(rollout_token_limit),
    );

    let rollout_path = rollout_path.display().to_string();
    let rollout_cwd = rollout_cwd.display().to_string();
    Ok(STAGE_ONE_INPUT_TEMPLATE
        .replace("{{ rollout_path }}", &rollout_path)
        .replace("{{ rollout_cwd }}", &rollout_cwd)
        .replace("{{ rollout_contents }}", &truncated_rollout_contents))
}

/// Build prompt used for read path. This prompt must be added to the developer instructions. In
/// case of large memory files, the `memory_summary.md` is truncated at
/// [phase_one::MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_SUMMARY_TOKEN_LIMIT].
pub(crate) async fn build_memory_tool_developer_instructions(codex_home: &Path) -> Option<String> {
    let base_path = memory_root(codex_home);
    let memory_summary_path = base_path.join("memory_summary.md");
    let memory_summary = fs::read_to_string(&memory_summary_path)
        .await
        .ok()?
        .trim()
        .to_string();
    let memory_summary = truncate_text(
        &memory_summary,
        TruncationPolicy::Tokens(phase_one::MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_SUMMARY_TOKEN_LIMIT),
    );
    if memory_summary.is_empty() {
        return None;
    }
    let base_path = base_path.display().to_string();
    Some(
        READ_PATH_TEMPLATE
            .replace("{{ base_path }}", &base_path)
            .replace("{{ memory_summary }}", &memory_summary),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models_manager::model_info::model_info_from_slug;

    #[test]
    fn build_stage_one_input_message_truncates_rollout_using_model_context_window() {
        let input = format!("{}{}{}", "a".repeat(700_000), "middle", "z".repeat(700_000));
        let mut model_info = model_info_from_slug("gpt-5.2-codex");
        model_info.context_window = Some(123_000);
        let expected_rollout_token_limit = usize::try_from(
            ((123_000_i64 * model_info.effective_context_window_percent) / 100)
                * phase_one::CONTEXT_WINDOW_PERCENT
                / 100,
        )
        .unwrap();
        let expected_truncated = truncate_text(
            &input,
            TruncationPolicy::Tokens(expected_rollout_token_limit),
        );
        let message = build_stage_one_input_message(
            &model_info,
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

    #[test]
    fn build_stage_one_input_message_uses_default_limit_when_model_context_window_missing() {
        let input = format!("{}{}{}", "a".repeat(700_000), "middle", "z".repeat(700_000));
        let mut model_info = model_info_from_slug("gpt-5.2-codex");
        model_info.context_window = None;
        let expected_truncated = truncate_text(
            &input,
            TruncationPolicy::Tokens(phase_one::DEFAULT_STAGE_ONE_ROLLOUT_TOKEN_LIMIT),
        );
        let message = build_stage_one_input_message(
            &model_info,
            Path::new("/tmp/rollout.jsonl"),
            Path::new("/tmp"),
            &input,
        )
        .unwrap();

        assert!(message.contains(&expected_truncated));
    }
}
