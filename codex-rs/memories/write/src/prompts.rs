use crate::memory_extensions_root;
use codex_protocol::openai_models::ModelInfo;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;
use codex_utils_template::Template;
use std::path::Path;
use std::sync::LazyLock;
use tracing::warn;

static CONSOLIDATION_PROMPT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../templates/memories/consolidation.md"),
        "memories/consolidation.md",
    )
});
static STAGE_ONE_INPUT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../templates/memories/stage_one_input.md"),
        "memories/stage_one_input.md",
    )
});
static MEMORY_EXTENSIONS_FOLDER_STRUCTURE_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        crate::prompt_blocks::EXTENSIONS_FOLDER_STRUCTURE,
        "memories/extensions_folder_structure.md",
    )
});
static MEMORY_EXTENSIONS_PRIMARY_INPUTS_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        crate::prompt_blocks::EXTENSIONS_PRIMARY_INPUTS,
        "memories/extensions_primary_inputs.md",
    )
});

fn parse_embedded_template(source: &'static str, template_name: &str) -> Template {
    match Template::parse(source) {
        Ok(template) => template,
        Err(err) => panic!("embedded template {template_name} is invalid: {err}"),
    }
}

/// Builds the consolidation subagent prompt for a specific memory root.
pub fn build_consolidation_prompt(memory_root: &Path) -> String {
    let memory_extensions_root = memory_extensions_root(memory_root);
    let memory_extensions_exist = memory_extensions_root.is_dir();
    let memory_root = memory_root.display().to_string();
    let memory_extensions_root = memory_extensions_root.display().to_string();
    let phase2_workspace_diff_file = crate::workspace_diff::FILENAME.to_string();
    let memory_extensions_folder_structure = if memory_extensions_exist {
        render_memory_extensions_block(
            &MEMORY_EXTENSIONS_FOLDER_STRUCTURE_TEMPLATE,
            &memory_extensions_root,
        )
    } else {
        String::new()
    };
    let memory_extensions_primary_inputs = if memory_extensions_exist {
        render_memory_extensions_block(
            &MEMORY_EXTENSIONS_PRIMARY_INPUTS_TEMPLATE,
            &memory_extensions_root,
        )
    } else {
        String::new()
    };
    CONSOLIDATION_PROMPT_TEMPLATE
        .render([
            ("memory_root", memory_root.as_str()),
            (
                "memory_extensions_folder_structure",
                memory_extensions_folder_structure.as_str(),
            ),
            (
                "memory_extensions_primary_inputs",
                memory_extensions_primary_inputs.as_str(),
            ),
            (
                "phase2_workspace_diff_file",
                phase2_workspace_diff_file.as_str(),
            ),
        ])
        .unwrap_or_else(|err| {
            warn!("failed to render memories consolidation prompt template: {err}");
            format!(
                "## Memory Phase 2 (Consolidation)\nConsolidate Codex memories in: {memory_root}\n\nRead {phase2_workspace_diff_file} first."
            )
        })
}

fn render_memory_extensions_block(template: &Template, memory_extensions_root: &str) -> String {
    template
        .render([("memory_extensions_root", memory_extensions_root)])
        .unwrap_or_else(|err| {
            warn!("failed to render memories extension prompt block: {err}");
            String::new()
        })
}

/// Builds the stage-1 user message containing rollout metadata and content.
///
/// Large rollout payloads are truncated to 70% of the active model's effective
/// input window token budget while keeping both head and tail context.
pub fn build_stage_one_input_message(
    model_info: &ModelInfo,
    rollout_path: &Path,
    rollout_cwd: &Path,
    rollout_contents: &str,
) -> anyhow::Result<String> {
    let rollout_token_limit = model_info
        .resolved_context_window()
        .and_then(|limit| (limit > 0).then_some(limit))
        .map(|limit| limit.saturating_mul(model_info.effective_context_window_percent) / 100)
        .map(|limit| (limit.saturating_mul(crate::stage_one::CONTEXT_WINDOW_PERCENT) / 100).max(1))
        .and_then(|limit| usize::try_from(limit).ok())
        .unwrap_or(crate::stage_one::DEFAULT_ROLLOUT_TOKEN_LIMIT);
    let truncated_rollout_contents = truncate_text(
        rollout_contents,
        TruncationPolicy::Tokens(rollout_token_limit),
    );

    let rollout_path = rollout_path.display().to_string();
    let rollout_cwd = rollout_cwd.display().to_string();
    Ok(STAGE_ONE_INPUT_TEMPLATE.render([
        ("rollout_path", rollout_path.as_str()),
        ("rollout_cwd", rollout_cwd.as_str()),
        ("rollout_contents", truncated_rollout_contents.as_str()),
    ])?)
}

#[cfg(test)]
#[path = "prompts_tests.rs"]
mod tests;
