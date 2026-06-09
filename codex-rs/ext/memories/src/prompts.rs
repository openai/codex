use crate::MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_SUMMARY_TOKEN_LIMIT;
use crate::prompt_source::MemoryPromptSource;
use crate::prompt_source::MemoryPromptSummary;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;
use codex_utils_template::Template;
use std::sync::LazyLock;

static MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../templates/memories/read_path.md"),
        "memories/read_path.md",
    )
});

fn parse_embedded_template(source: &'static str, template_name: &str) -> Template {
    match Template::parse(source) {
        Ok(template) => template,
        Err(err) => panic!("embedded template {template_name} is invalid: {err}"),
    }
}

/// Build the memory read-path prompt that is added to developer instructions.
///
/// Large `memory_summary.md` files are truncated at
/// [MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_SUMMARY_TOKEN_LIMIT].
pub(crate) async fn build_memory_tool_developer_instructions<S>(source: &S) -> Option<String>
where
    S: MemoryPromptSource,
{
    let MemoryPromptSummary { base_path, content } = source.read_summary().await?;
    let memory_summary = content.trim().to_string();
    let memory_summary = truncate_text(
        &memory_summary,
        TruncationPolicy::Tokens(MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_SUMMARY_TOKEN_LIMIT),
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
