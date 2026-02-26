use serde_json::Value;

use crate::responses::ResponsesRequest;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContextSnapshotRenderMode {
    #[default]
    RedactedText,
    FullText,
    KindOnly,
    KindWithTextPrefix {
        max_chars: usize,
    },
}

#[derive(Debug, Clone)]
pub struct ContextSnapshotOptions {
    render_mode: ContextSnapshotRenderMode,
}

impl Default for ContextSnapshotOptions {
    fn default() -> Self {
        Self {
            render_mode: ContextSnapshotRenderMode::RedactedText,
        }
    }
}

impl ContextSnapshotOptions {
    pub fn render_mode(mut self, render_mode: ContextSnapshotRenderMode) -> Self {
        self.render_mode = render_mode;
        self
    }
}

pub fn format_request_input_snapshot(
    request: &ResponsesRequest,
    options: &ContextSnapshotOptions,
) -> String {
    let items = request.input();
    format_response_items_snapshot(items.as_slice(), options)
}

pub fn format_response_items_snapshot(items: &[Value], options: &ContextSnapshotOptions) -> String {
    items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let Some(item_type) = item.get("type").and_then(Value::as_str) else {
                return format!("{idx:02}:<MISSING_TYPE>");
            };

            if options.render_mode == ContextSnapshotRenderMode::KindOnly {
                return if item_type == "message" {
                    let role = item.get("role").and_then(Value::as_str).unwrap_or("unknown");
                    format!("{idx:02}:message/{role}")
                } else {
                    format!("{idx:02}:{item_type}")
                };
            }

            match item_type {
                "message" => {
                    let role = item.get("role").and_then(Value::as_str).unwrap_or("unknown");
                    let rendered_parts = item
                        .get("content")
                        .and_then(Value::as_array)
                        .map(|content| {
                            content
                                .iter()
                                .map(|entry| {
                                    if let Some(text) = entry.get("text").and_then(Value::as_str) {
                                        return format_snapshot_text(text, options);
                                    }
                                    let Some(content_type) =
                                        entry.get("type").and_then(Value::as_str)
                                    else {
                                        return "<UNKNOWN_CONTENT_ITEM>".to_string();
                                    };
                                    let Some(content_object) = entry.as_object() else {
                                        return format!("<{content_type}>");
                                    };
                                    let mut extra_keys = content_object
                                        .keys()
                                        .filter(|key| *key != "type" && *key != "text")
                                        .cloned()
                                        .collect::<Vec<String>>();
                                    extra_keys.sort();
                                    if extra_keys.is_empty() {
                                        format!("<{content_type}>")
                                    } else {
                                        format!("<{content_type}:{}>", extra_keys.join(","))
                                    }
                                })
                                .collect::<Vec<String>>()
                        })
                        .unwrap_or_default();
                    let role = if rendered_parts.len() > 1 {
                        format!("{role}[{}]", rendered_parts.len())
                    } else {
                        role.to_string()
                    };
                    if rendered_parts.is_empty() {
                        return format!("{idx:02}:message/{role}:<NO_TEXT>");
                    }
                    if rendered_parts.len() == 1 {
                        return format!("{idx:02}:message/{role}:{}", rendered_parts[0]);
                    }

                    let parts = rendered_parts
                        .iter()
                        .enumerate()
                        .map(|(part_idx, part)| format!("    [{:02}] {part}", part_idx + 1))
                        .collect::<Vec<String>>()
                        .join("\n");
                    format!("{idx:02}:message/{role}:\n{parts}")
                }
                "function_call" => {
                    let name = item.get("name").and_then(Value::as_str).unwrap_or("unknown");
                    format!("{idx:02}:function_call/{name}")
                }
                "function_call_output" => {
                    let output = item
                        .get("output")
                        .and_then(Value::as_str)
                        .map(|output| format_snapshot_text(output, options))
                        .unwrap_or_else(|| "<NON_STRING_OUTPUT>".to_string());
                    format!("{idx:02}:function_call_output:{output}")
                }
                "local_shell_call" => {
                    let command = item
                        .get("action")
                        .and_then(|action| action.get("command"))
                        .and_then(Value::as_array)
                        .map(|parts| {
                            parts
                                .iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<&str>>()
                                .join(" ")
                        })
                        .map(|command| format_snapshot_text(&command, options))
                        .filter(|cmd| !cmd.is_empty())
                        .unwrap_or_else(|| "<NO_COMMAND>".to_string());
                    format!("{idx:02}:local_shell_call:{command}")
                }
                "reasoning" => {
                    let summary_text = item
                        .get("summary")
                        .and_then(Value::as_array)
                        .and_then(|summary| summary.first())
                        .and_then(|entry| entry.get("text"))
                        .and_then(Value::as_str)
                        .map(|text| format_snapshot_text(text, options))
                        .unwrap_or_else(|| "<NO_SUMMARY>".to_string());
                    let has_encrypted_content = item
                        .get("encrypted_content")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.is_empty());
                    format!(
                        "{idx:02}:reasoning:summary={summary_text}:encrypted={has_encrypted_content}"
                    )
                }
                "compaction" => {
                    let has_encrypted_content = item
                        .get("encrypted_content")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.is_empty());
                    format!("{idx:02}:compaction:encrypted={has_encrypted_content}")
                }
                other => format!("{idx:02}:{other}"),
            }
        })
        .collect::<Vec<String>>()
        .join("\n")
}

pub fn format_labeled_requests_snapshot(
    scenario: &str,
    sections: &[(&str, &ResponsesRequest)],
    options: &ContextSnapshotOptions,
) -> String {
    let sections = sections
        .iter()
        .map(|(title, request)| {
            format!(
                "## {title}\n{}",
                format_request_input_snapshot(request, options)
            )
        })
        .collect::<Vec<String>>()
        .join("\n\n");
    format!("Scenario: {scenario}\n\n{sections}")
}

pub fn format_labeled_items_snapshot(
    scenario: &str,
    sections: &[(&str, &[Value])],
    options: &ContextSnapshotOptions,
) -> String {
    let sections = sections
        .iter()
        .map(|(title, items)| {
            format!(
                "## {title}\n{}",
                format_response_items_snapshot(items, options)
            )
        })
        .collect::<Vec<String>>()
        .join("\n\n");
    format!("Scenario: {scenario}\n\n{sections}")
}

fn format_snapshot_text(text: &str, options: &ContextSnapshotOptions) -> String {
    match options.render_mode {
        ContextSnapshotRenderMode::RedactedText => {
            canonicalize_snapshot_text(text).replace('\n', "\\n")
        }
        ContextSnapshotRenderMode::FullText => text.replace('\n', "\\n"),
        ContextSnapshotRenderMode::KindWithTextPrefix { max_chars } => {
            let normalized = canonicalize_snapshot_text(text).replace('\n', "\\n");
            if normalized.chars().count() <= max_chars {
                normalized
            } else {
                let prefix = normalized.chars().take(max_chars).collect::<String>();
                format!("{prefix}...")
            }
        }
        ContextSnapshotRenderMode::KindOnly => unreachable!(),
    }
}

fn canonicalize_snapshot_text(text: &str) -> String {
    if text.starts_with("<permissions instructions>") {
        return "<PERMISSIONS_INSTRUCTIONS>".to_string();
    }
    if text.starts_with("# AGENTS.md instructions for ") {
        return "<AGENTS_MD>".to_string();
    }
    if text.starts_with("<environment_context>") {
        if let (Some(cwd_start), Some(cwd_end)) = (text.find("<cwd>"), text.find("</cwd>")) {
            let cwd = &text[cwd_start + "<cwd>".len()..cwd_end];
            return if cwd.ends_with("PRETURN_CONTEXT_DIFF_CWD") {
                "<ENVIRONMENT_CONTEXT:cwd=PRETURN_CONTEXT_DIFF_CWD>".to_string()
            } else {
                "<ENVIRONMENT_CONTEXT:cwd=<CWD>>".to_string()
            };
        }
        return "<ENVIRONMENT_CONTEXT>".to_string();
    }
    if text.starts_with("You are performing a CONTEXT CHECKPOINT COMPACTION.") {
        return "<SUMMARIZATION_PROMPT>".to_string();
    }
    if text.starts_with("Another language model started to solve this problem")
        && let Some((_, summary)) = text.split_once('\n')
    {
        return format!("<COMPACTION_SUMMARY>\n{summary}");
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::ContextSnapshotOptions;
    use super::ContextSnapshotRenderMode;
    use super::format_response_items_snapshot;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn full_text_mode_preserves_unredacted_text() {
        let items = vec![json!({
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "# AGENTS.md instructions for /tmp/example\n\n<INSTRUCTIONS>\nbody\n</INSTRUCTIONS>"
            }]
        })];

        let rendered = format_response_items_snapshot(
            &items,
            &ContextSnapshotOptions::default().render_mode(ContextSnapshotRenderMode::FullText),
        );

        assert_eq!(
            rendered,
            r"00:message/user:# AGENTS.md instructions for /tmp/example\n\n<INSTRUCTIONS>\nbody\n</INSTRUCTIONS>"
        );
    }

    #[test]
    fn redacted_text_mode_keeps_canonical_placeholders() {
        let items = vec![json!({
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "# AGENTS.md instructions for /tmp/example\n\n<INSTRUCTIONS>\nbody\n</INSTRUCTIONS>"
            }]
        })];

        let rendered = format_response_items_snapshot(
            &items,
            &ContextSnapshotOptions::default().render_mode(ContextSnapshotRenderMode::RedactedText),
        );

        assert_eq!(rendered, "00:message/user:<AGENTS_MD>");
    }

    #[test]
    fn image_only_message_is_rendered_as_non_text_span() {
        let items = vec![json!({
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_image",
                "image_url": "data:image/png;base64,AAAA"
            }]
        })];

        let rendered = format_response_items_snapshot(&items, &ContextSnapshotOptions::default());

        assert_eq!(rendered, "00:message/user:<input_image:image_url>");
    }

    #[test]
    fn mixed_text_and_image_message_keeps_image_span() {
        let items = vec![json!({
            "type": "message",
            "role": "user",
            "content": [
                {
                    "type": "input_text",
                    "text": "<image>"
                },
                {
                    "type": "input_image",
                    "image_url": "data:image/png;base64,AAAA"
                },
                {
                    "type": "input_text",
                    "text": "</image>"
                }
            ]
        })];

        let rendered = format_response_items_snapshot(&items, &ContextSnapshotOptions::default());

        assert_eq!(
            rendered,
            "00:message/user[3]:\n    [01] <image>\n    [02] <input_image:image_url>\n    [03] </image>"
        );
    }
}
