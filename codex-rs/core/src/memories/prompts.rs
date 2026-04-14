use crate::memories::extensions::{RemovedExtensionResource, EXTENSION_RESOURCE_RETENTION_DAYS};
use crate::memories::memory_extensions_root;
use crate::memories::memory_root;
use crate::memories::phase_one;
use crate::memories::storage::rollout_summary_file_stem_from_parts;
use codex_protocol::openai_models::ModelInfo;
use codex_state::Phase2InputSelection;
use codex_state::Stage1Output;
use codex_state::Stage1OutputRef;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;
use codex_utils_template::Template;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::LazyLock;
use tokio::fs;
use tracing::warn;

static CONSOLIDATION_PROMPT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../../templates/memories/consolidation.md"),
        "memories/consolidation.md",
    )
});
static STAGE_ONE_INPUT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../../templates/memories/stage_one_input.md"),
        "memories/stage_one_input.md",
    )
});
static MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../../templates/memories/read_path.md"),
        "memories/read_path.md",
    )
});
static MEMORY_EXTENSIONS_FOLDER_STRUCTURE_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        MEMORY_EXTENSIONS_FOLDER_STRUCTURE,
        "memories/extensions_folder_structure.md",
    )
});
static MEMORY_EXTENSIONS_PRIMARY_INPUTS_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        MEMORY_EXTENSIONS_PRIMARY_INPUTS,
        "memories/extensions_primary_inputs.md",
    )
});

fn parse_embedded_template(source: &'static str, template_name: &str) -> Template {
    match Template::parse(source) {
        Ok(template) => template,
        Err(err) => panic!("embedded template {template_name} is invalid: {err}"),
    }
}

const MEMORY_EXTENSIONS_FOLDER_STRUCTURE: &str = r#"
Memory extensions (under {{ memory_extensions_root }}/):

- <extension_name>/instructions.md
  - Source-specific guidance for interpreting additional memory signals. If an
    extension folder exists, you must read its instructions.md to determine how to use this memory
    source.

If the user has any memory extensions, you MUST read the instructions for each extension to
determine how to use the memory source. If the Phase 2 diff lists removed memory extension
resources, use that extension-specific deletion diff to remove stale memories derived only from
those resources. If it has no extension folders, continue with the standard memory inputs only.
"#;

const MEMORY_EXTENSIONS_PRIMARY_INPUTS: &str = r#"
Optional source-specific inputs:
Under `{{ memory_extensions_root }}/`:

- `<extension_name>/instructions.md`
  - If extension folders exist, read each instructions.md first and follow it when interpreting
    that extension's memory source.

If the Phase 2 diff lists removed memory extension resources, use that extension-specific deletion
diff to remove stale memories derived only from those resources.
"#;

/// Builds the consolidation subagent prompt for a specific memory root.
pub(super) fn build_consolidation_prompt(
    memory_root: &Path,
    selection: &Phase2InputSelection,
    removed_extension_resources: &[RemovedExtensionResource],
) -> String {
    let memory_extensions_root = memory_extensions_root(memory_root);
    let memory_extensions_exist = memory_extensions_root.is_dir();
    let memory_root = memory_root.display().to_string();
    let memory_extensions_root = memory_extensions_root.display().to_string();
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
    let phase2_input_selection =
        render_phase2_input_selection(selection, removed_extension_resources);
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
            ("phase2_input_selection", phase2_input_selection.as_str()),
        ])
        .unwrap_or_else(|err| {
            warn!("failed to render memories consolidation prompt template: {err}");
            format!(
                "## Memory Phase 2 (Consolidation)\nConsolidate Codex memories in: {memory_root}\n\n{phase2_input_selection}"
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

fn render_phase2_input_selection(
    selection: &Phase2InputSelection,
    removed_extension_resources: &[RemovedExtensionResource],
) -> String {
    let retained = selection.retained_thread_ids.len();
    let added = selection.selected.len().saturating_sub(retained);
    let selected = if selection.selected.is_empty() {
        "- none".to_string()
    } else {
        selection
            .selected
            .iter()
            .map(|item| {
                render_selected_input_line(
                    item,
                    selection.retained_thread_ids.contains(&item.thread_id),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let removed = if selection.removed.is_empty() {
        "- none".to_string()
    } else {
        selection
            .removed
            .iter()
            .map(render_removed_input_line)
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mut rendered = format!(
        "- selected inputs this run: {}\n- newly added since the last successful Phase 2 run: {added}\n- retained from the last successful Phase 2 run: {retained}\n- removed from the last successful Phase 2 run: {}\n\nCurrent selected Phase 1 inputs:\n{selected}\n\nRemoved from the last successful Phase 2 selection:\n{removed}\n",
        selection.selected.len(),
        selection.removed.len(),
    );

    if !removed_extension_resources.is_empty() {
        rendered.push_str("\nMemory extension resources removed by retention pruning:\n");
        let _ = writeln!(
            rendered,
            "- retention window: {EXTENSION_RESOURCE_RETENTION_DAYS} days"
        );
        let mut current_extension = "";
        for removed_resource in removed_extension_resources {
            if removed_resource.extension != current_extension {
                current_extension = &removed_resource.extension;
                let _ = writeln!(rendered, "- extension: {current_extension}");
            }
            let _ = writeln!(rendered, "  - {}", removed_resource.resource_path);
        }
    }

    rendered
}

fn render_selected_input_line(item: &Stage1Output, retained: bool) -> String {
    let status = if retained { "retained" } else { "added" };
    let rollout_summary_file = format!(
        "rollout_summaries/{}.md",
        rollout_summary_file_stem_from_parts(
            item.thread_id,
            item.source_updated_at,
            item.rollout_slug.as_deref(),
        )
    );
    format!(
        "- [{status}] thread_id={}, rollout_summary_file={rollout_summary_file}",
        item.thread_id
    )
}

fn render_removed_input_line(item: &Stage1OutputRef) -> String {
    let rollout_summary_file = format!(
        "rollout_summaries/{}.md",
        rollout_summary_file_stem_from_parts(
            item.thread_id,
            item.source_updated_at,
            item.rollout_slug.as_deref(),
        )
    );
    format!(
        "- thread_id={}, rollout_summary_file={rollout_summary_file}",
        item.thread_id
    )
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
    Ok(STAGE_ONE_INPUT_TEMPLATE.render([
        ("rollout_path", rollout_path.as_str()),
        ("rollout_cwd", rollout_cwd.as_str()),
        ("rollout_contents", truncated_rollout_contents.as_str()),
    ])?)
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
    MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_TEMPLATE
        .render([
            ("base_path", base_path.as_str()),
            ("memory_summary", memory_summary.as_str()),
        ])
        .ok()
}

#[cfg(test)]
#[path = "prompts_tests.rs"]
mod tests;
