//! Prepares nested tool definitions for execution and search.
//! Cached definitions stay borrowed; rendered schemas are dropped after augmentation.
//! Description order follows the session-static feature setting.

use crate::tools::registry::CoreToolRuntime;
use codex_code_mode::ToolDefinition;
use codex_tools::ToolSpec;
use std::borrow::Cow;

pub(crate) fn prepare_code_mode_tool_definitions<'a>(
    runtime: Option<&'a dyn CoreToolRuntime>,
    spec: impl FnOnce() -> Cow<'a, ToolSpec>,
    code_mode_input_schema_max_bytes: Option<usize>,
    tool_description_first: bool,
) -> Cow<'a, [ToolDefinition]> {
    if let Some(definitions) = runtime.and_then(|runtime| {
        runtime
            .cached_code_mode_definitions(code_mode_input_schema_max_bytes, tool_description_first)
    }) {
        return Cow::Borrowed(definitions);
    }

    let spec = spec();
    let mut definitions = codex_tools::collect_code_mode_tool_definitions(
        std::iter::once(spec.as_ref()),
        code_mode_input_schema_max_bytes,
        tool_description_first,
    );
    for definition in &mut definitions {
        definition.input_schema = None;
        definition.output_schema = None;
    }
    Cow::Owned(definitions)
}
