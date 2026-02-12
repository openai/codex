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
struct ContextSnapshotPrefixCapture {
    prefix: String,
    marker_prefix: String,
    marker_suffix: String,
}

#[derive(Debug, Clone)]
pub struct ContextSnapshotOptions {
    render_mode: ContextSnapshotRenderMode,
    normalize_environment_context: bool,
    text_exact_replacements: Vec<(String, String)>,
    text_prefix_replacements: Vec<(String, String)>,
    text_contains_replacements: Vec<(String, String)>,
    text_prefix_captures: Vec<ContextSnapshotPrefixCapture>,
    cwd_contains_replacements: Vec<(String, String)>,
    tool_name_replacements: Vec<(String, String)>,
}

impl Default for ContextSnapshotOptions {
    fn default() -> Self {
        Self {
            render_mode: ContextSnapshotRenderMode::RedactedText,
            normalize_environment_context: true,
            text_exact_replacements: Vec::new(),
            text_prefix_replacements: vec![(
                "# AGENTS.md instructions for ".to_string(),
                "<AGENTS_MD>".to_string(),
            )],
            text_contains_replacements: vec![(
                "<permissions instructions>".to_string(),
                "<PERMISSIONS_INSTRUCTIONS>".to_string(),
            )],
            text_prefix_captures: Vec::new(),
            cwd_contains_replacements: Vec::new(),
            tool_name_replacements: Vec::new(),
        }
    }
}

impl ContextSnapshotOptions {
    pub fn render_mode(mut self, render_mode: ContextSnapshotRenderMode) -> Self {
        self.render_mode = render_mode;
        self
    }

    pub fn normalize_environment_context(mut self, enabled: bool) -> Self {
        self.normalize_environment_context = enabled;
        self
    }

    pub fn replace_exact_text(
        mut self,
        text: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        self.text_exact_replacements
            .push((text.into(), replacement.into()));
        self
    }

    pub fn replace_text_with_prefix(
        mut self,
        prefix: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        self.text_prefix_replacements
            .push((prefix.into(), replacement.into()));
        self
    }

    pub fn replace_text_containing(
        mut self,
        needle: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        self.text_contains_replacements
            .push((needle.into(), replacement.into()));
        self
    }

    pub fn capture_text_suffix_after_prefix(
        mut self,
        prefix: impl Into<String>,
        marker_prefix: impl Into<String>,
        marker_suffix: impl Into<String>,
    ) -> Self {
        self.text_prefix_captures
            .push(ContextSnapshotPrefixCapture {
                prefix: prefix.into(),
                marker_prefix: marker_prefix.into(),
                marker_suffix: marker_suffix.into(),
            });
        self
    }

    pub fn replace_cwd_when_contains(
        mut self,
        cwd_substring: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        self.cwd_contains_replacements
            .push((cwd_substring.into(), replacement.into()));
        self
    }

    pub fn replace_tool_name(
        mut self,
        original_name: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        self.tool_name_replacements
            .push((original_name.into(), replacement.into()));
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
                    let normalized_name = options
                        .tool_name_replacements
                        .iter()
                        .find_map(|(original_name, replacement)| {
                            (original_name == name).then_some(replacement.as_str())
                        })
                        .unwrap_or(name);
                    format!("{idx:02}:function_call/{normalized_name}")
                }
                "function_call_output" => {
                    let output = item
                        .get("output")
                        .and_then(Value::as_str)
                        .map(|output| match options.render_mode {
                            ContextSnapshotRenderMode::RedactedText => {
                                if output.starts_with("unsupported call: ")
                                    || output.starts_with("unsupported custom tool call: ")
                                {
                                    "<TOOL_ERROR_OUTPUT>".to_string()
                                } else {
                                    normalize_shape_text(output, options)
                                }
                            }
                            ContextSnapshotRenderMode::FullText => output.replace('\n', "\\n"),
                            ContextSnapshotRenderMode::KindOnly => unreachable!(),
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
    if options.render_mode == ContextSnapshotRenderMode::FullText {
        return text.replace('\n', "\\n");
    }

    if let Some((_, replacement)) = options
        .text_exact_replacements
        .iter()
        .find(|(target, _)| target == text)
    {
        return replacement.clone();
    }

    if let Some((_, replacement)) = options
        .text_prefix_replacements
        .iter()
        .find(|(prefix, _)| text.starts_with(prefix.as_str()))
    {
        return replacement.clone();
    }

    if let Some(capture) = options
        .text_prefix_captures
        .iter()
        .find(|capture| text.starts_with(capture.prefix.as_str()))
    {
        let suffix = text
            .strip_prefix(capture.prefix.as_str())
            .unwrap_or_default();
        return format!(
            "{}{}{}",
            capture.marker_prefix, suffix, capture.marker_suffix
        );
    }

    if options.normalize_environment_context && text.starts_with("<environment_context>") {
        let cwd = text.lines().find_map(|line| {
            let trimmed = line.trim();
            let cwd = trimmed.strip_prefix("<cwd>")?.strip_suffix("</cwd>")?;
            if let Some((_, replacement)) = options
                .cwd_contains_replacements
                .iter()
                .find(|(needle, _)| cwd.contains(needle.as_str()))
            {
                return Some(replacement.clone());
            }
            Some("<CWD>".to_string())
        });
        return match cwd {
            Some(cwd) => format!("<ENVIRONMENT_CONTEXT:cwd={cwd}>"),
            None => "<ENVIRONMENT_CONTEXT:cwd=<NONE>>".to_string(),
        };
    }

    if let Some((_, replacement)) = options
        .text_contains_replacements
        .iter()
        .find(|(needle, _)| text.contains(needle.as_str()))
    {
        return replacement.clone();
    }

    text.replace('\n', "\\n")
}
