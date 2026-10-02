//! Display-only previews for recognized delegated inputs; model content is never rewritten.

use codex_protocol::items::FunctionCallOutputItem;

/// Extract the task text from a delegated input, ignoring unrelated tool outputs.
pub fn delegated_output_preview(output: &FunctionCallOutputItem) -> Option<String> {
    if !matches!(output.namespace.as_deref(), Some("codex_app" | "codex_tui"))
        || !matches!(
            output.name.as_str(),
            "create_thread" | "send_message_to_thread"
        )
    {
        return None;
    }
    let text = output.output.to_text()?;
    let preview = text
        .strip_prefix("<codex_delegation>\n  <source_thread_id>")
        .and_then(|delegation| delegation.split_once("</source_thread_id>\n  <input>"))
        .and_then(|(_, input)| input.strip_suffix("</input>\n</codex_delegation>"))
        .map(|input| {
            input
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&")
        })
        .unwrap_or(text);
    (!preview.trim().is_empty()).then_some(preview)
}

#[cfg(test)]
#[path = "delegated_preview_tests.rs"]
mod tests;
