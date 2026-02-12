use serde_json::Value;

use crate::responses::ResponsesRequest;

#[derive(Debug, Clone, Default)]
pub struct ContextSnapshotOptions {
    summary_prefix: Option<String>,
    summarization_prompt: Option<String>,
    cwd_marker: Option<String>,
    tool_call_name: Option<String>,
}

impl ContextSnapshotOptions {
    pub fn summary_prefix(mut self, summary_prefix: impl Into<String>) -> Self {
        self.summary_prefix = Some(summary_prefix.into());
        self
    }

    pub fn summarization_prompt(mut self, summarization_prompt: impl Into<String>) -> Self {
        self.summarization_prompt = Some(summarization_prompt.into());
        self
    }

    pub fn cwd_marker(mut self, cwd_marker: impl Into<String>) -> Self {
        self.cwd_marker = Some(cwd_marker.into());
        self
    }

    pub fn tool_call_name(mut self, tool_call_name: impl Into<String>) -> Self {
        self.tool_call_name = Some(tool_call_name.into());
        self
    }
}

pub fn request_input_shape(request: &ResponsesRequest, options: &ContextSnapshotOptions) -> String {
    request
        .input()
        .into_iter()
        .enumerate()
        .map(|(idx, item)| {
            let Some(item_type) = item.get("type").and_then(Value::as_str) else {
                return format!("{idx:02}:<MISSING_TYPE>");
            };
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
                    let normalized_name = if options.tool_call_name.as_deref() == Some(name) {
                        "<TOOL_CALL>"
                    } else {
                        name
                    };
                    format!("{idx:02}:function_call/{normalized_name}")
                }
                "function_call_output" => {
                    let output = item
                        .get("output")
                        .and_then(Value::as_str)
                        .map(|output| {
                            if output.starts_with("unsupported call: ")
                                || output.starts_with("unsupported custom tool call: ")
                            {
                                "<TOOL_ERROR_OUTPUT>".to_string()
                            } else {
                                normalize_shape_text(output, options)
                            }
                        })
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

fn normalize_shape_text(text: &str, options: &ContextSnapshotOptions) -> String {
    if options.summarization_prompt.as_deref() == Some(text) {
        return "<SUMMARIZATION_PROMPT>".to_string();
    }
    if let Some(summary_prefix) = options.summary_prefix.as_deref() {
        let summary_prefix_line = format!("{summary_prefix}\n");
        if let Some(summary) = text.strip_prefix(summary_prefix_line.as_str()) {
            return format!("<SUMMARY:{summary}>");
        }
    }
    if text.starts_with("# AGENTS.md instructions for ") {
        return "<AGENTS_MD>".to_string();
    }
    if text.starts_with("<environment_context>") {
        let cwd = text.lines().find_map(|line| {
            let trimmed = line.trim();
            let cwd = trimmed.strip_prefix("<cwd>")?.strip_suffix("</cwd>")?;
            if options
                .cwd_marker
                .as_deref()
                .is_some_and(|marker| cwd.contains(marker))
            {
                return options.cwd_marker.clone();
            }
            Some("<CWD>".to_string())
        });
        return match cwd {
            Some(cwd) => format!("<ENVIRONMENT_CONTEXT:cwd={cwd}>"),
            None => "<ENVIRONMENT_CONTEXT:cwd=<NONE>>".to_string(),
        };
    }
    if text.contains("<permissions instructions>") {
        return "<PERMISSIONS_INSTRUCTIONS>".to_string();
    }

    text.replace('\n', "\\n")
}
