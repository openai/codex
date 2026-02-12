use serde_json::Value;

use crate::responses::ResponsesRequest;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContextSnapshotRenderMode {
    #[default]
    RedactedText,
    FullText,
    KindOnly,
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

pub fn request_input_shape(request: &ResponsesRequest, options: &ContextSnapshotOptions) -> String {
    let items = request.input();
    response_items_shape(items.as_slice(), options)
}

pub fn response_items_shape(items: &[Value], options: &ContextSnapshotOptions) -> String {
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
                    let text = item
                        .get("content")
                        .and_then(Value::as_array)
                        .map(|content| {
                            content
                                .iter()
                                .filter_map(|entry| entry.get("text").and_then(Value::as_str))
                                .map(|text| normalize_shape_text(text, options))
                                .collect::<Vec<String>>()
                                .join(" | ")
                        })
                        .filter(|text| !text.is_empty())
                        .unwrap_or_else(|| "<NO_TEXT>".to_string());
                    format!("{idx:02}:message/{role}:{text}")
                }
                "function_call" => {
                    let name = item.get("name").and_then(Value::as_str).unwrap_or("unknown");
                    format!("{idx:02}:function_call/{name}")
                }
                "function_call_output" => {
                    let output = item
                        .get("output")
                        .and_then(Value::as_str)
                        .map(|output| output.replace('\n', "\\n"))
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
                        .map(|text| normalize_shape_text(text, options))
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

pub fn sectioned_request_shapes(
    scenario: &str,
    sections: &[(&str, &ResponsesRequest)],
    options: &ContextSnapshotOptions,
) -> String {
    let sections = sections
        .iter()
        .map(|(title, request)| format!("## {title}\n{}", request_input_shape(request, options)))
        .collect::<Vec<String>>()
        .join("\n\n");
    format!("Scenario: {scenario}\n\n{sections}")
}

pub fn sectioned_item_shapes(
    scenario: &str,
    sections: &[(&str, &[Value])],
    options: &ContextSnapshotOptions,
) -> String {
    let sections = sections
        .iter()
        .map(|(title, items)| format!("## {title}\n{}", response_items_shape(items, options)))
        .collect::<Vec<String>>()
        .join("\n\n");
    format!("Scenario: {scenario}\n\n{sections}")
}

fn normalize_shape_text(text: &str, options: &ContextSnapshotOptions) -> String {
    match options.render_mode {
        ContextSnapshotRenderMode::RedactedText | ContextSnapshotRenderMode::FullText => {
            text.replace('\n', "\\n")
        }
        ContextSnapshotRenderMode::KindOnly => unreachable!(),
    }
}
