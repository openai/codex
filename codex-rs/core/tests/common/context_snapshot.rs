//! Readable snapshots of captured model context.
//!
//! Capturing and grouping requests only decides which input items are new. All entry points
//! render items with the same formatter, which then normalizes volatile text or replaces routine
//! context blocks according to the caller's options.

use crate::responses::ResponsesRequest;
use crate::responses::strip_metadata_from_json;
use crate::responses::strip_response_item_ids_from_json;
use normalize::Normalizer;
use normalize::fingerprint;
use normalize::fnv1a;
use normalize::normalize_json;
use normalize::normalize_line_endings;
use normalize::normalize_stable_text;
use normalize::render_text;
use normalize::shorten;
use serde_json::Value;

mod normalize;
#[cfg(test)]
#[path = "context_snapshot/context_snapshot_tests.rs"]
mod tests;

const MAX_SNAPSHOT_LINE_CHARS: usize = 160;

#[derive(Debug, Clone, Default)]
pub struct ContextSnapshotOptions {
    rewrite_known_segments: bool,
    include_request_settings: bool,
}

impl ContextSnapshotOptions {
    /// Replace routine Apps, Skills, Plugins, AGENTS.md, permissions, environment,
    /// model switches, personality, Guardian, and compaction guidance with named markers.
    /// Message parts remain visible.
    pub fn rewrite_known_segments(mut self) -> Self {
        self.rewrite_known_segments = true;
        self
    }

    /// Render model, instructions, tools, and other settings at window boundaries.
    /// Settings still participate in grouping when this is disabled.
    pub fn include_request_settings(mut self) -> Self {
        self.include_request_settings = true;
        self
    }
}

// Capture and group: compare complete, normalized request inputs and request settings.
// The first request in a window retains all its input; later requests retain their suffix index.
#[derive(Clone, Copy)]
enum SnapshotSource<'a> {
    Captured(&'a ResponsesRequest),
    Body(&'a Value),
    Items(&'a [Value]),
}

pub struct SnapshotEntry<'a> {
    label: Option<&'a str>,
    source: SnapshotSource<'a>,
}

impl<'a> SnapshotEntry<'a> {
    pub fn captured(request: &'a ResponsesRequest) -> Self {
        Self {
            label: None,
            source: SnapshotSource::Captured(request),
        }
    }

    pub fn body(body: &'a Value) -> Self {
        Self {
            label: None,
            source: SnapshotSource::Body(body),
        }
    }

    pub fn items(items: &'a [Value]) -> Self {
        Self {
            label: None,
            source: SnapshotSource::Items(items),
        }
    }

    pub fn labeled(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }
}

struct CapturedRequest {
    number: usize,
    kind: String,
    label: Option<String>,
    input: Vec<Value>,
    settings: Option<Value>,
}

struct Window {
    settings: Option<Value>,
    boundary: Option<InputBoundary>,
    requests: Vec<WindowRequest>,
}

struct WindowRequest {
    number: usize,
    kind: String,
    label: Option<String>,
    input: Vec<Value>,
    suffix_start: usize,
}

enum InputBoundary {
    Truncated(usize),
    Diverged(usize),
    Repeated,
    SettingsUnavailable,
}

fn capture_request(number: usize, entry: &SnapshotEntry<'_>) -> CapturedRequest {
    let (kind, input, settings) = match entry.source {
        SnapshotSource::Captured(request) => {
            let kind = request
                .header("x-codex-turn-metadata")
                .and_then(|header| serde_json::from_str::<Value>(&header).ok())
                .and_then(|metadata| metadata["request_kind"].as_str().map(str::to_owned))
                .unwrap_or_else(|| "request".to_string());
            let (input, settings) = capture_body(request.body_json());
            (kind, input, Some(settings))
        }
        SnapshotSource::Body(body) => {
            let (input, settings) = capture_body(body.clone());
            ("request".to_string(), input, Some(settings))
        }
        SnapshotSource::Items(items) => ("items".to_string(), Value::Array(items.to_vec()), None),
    };
    let input = strip_metadata_from_json(strip_response_item_ids_from_json(input));
    CapturedRequest {
        number,
        kind,
        label: entry.label.map(str::to_owned),
        input: input
            .as_array()
            .expect("request input should be an array")
            .clone(),
        settings,
    }
}

fn capture_body(mut body: Value) -> (Value, Value) {
    let settings = body
        .as_object_mut()
        .expect("request body should be an object");
    let input = settings
        .remove("input")
        .expect("request should contain input");
    // Telemetry does not affect request context. Keep the cache key in settings so
    // a change to it starts a new window.
    settings.remove("client_metadata");
    (input, body)
}

fn group_requests(requests: impl IntoIterator<Item = CapturedRequest>) -> Vec<Window> {
    let mut windows: Vec<Window> = Vec::new();
    for request in requests {
        let previous = windows.last().and_then(|window| window.requests.last());
        let shared = previous
            .map(|previous| {
                previous
                    .input
                    .iter()
                    .zip(&request.input)
                    .take_while(|(left, right)| left == right)
                    .count()
            })
            .unwrap_or(0);
        let rewritten = previous.is_some_and(|previous| shared != previous.input.len());
        let repeated = previous.is_some_and(|previous| shared == previous.input.len())
            && shared == request.input.len();
        let settings_changed = windows
            .last()
            .is_some_and(|window| window.settings != request.settings);
        let unknown_settings = request.settings.is_none();
        let new_window =
            previous.is_none() || rewritten || repeated || settings_changed || unknown_settings;
        if new_window {
            let boundary = if rewritten {
                Some(if shared == request.input.len() {
                    InputBoundary::Truncated(shared)
                } else {
                    InputBoundary::Diverged(shared)
                })
            } else if repeated {
                Some(InputBoundary::Repeated)
            } else if unknown_settings && previous.is_some() {
                Some(InputBoundary::SettingsUnavailable)
            } else {
                None
            };
            windows.push(Window {
                settings: request.settings,
                boundary,
                requests: Vec::new(),
            });
        }
        windows
            .last_mut()
            .expect("first request starts a window")
            .requests
            .push(WindowRequest {
                number: request.number,
                kind: request.kind,
                label: request.label,
                input: request.input,
                suffix_start: if new_window { 0 } else { shared },
            });
    }
    windows
}

// Render: labeled snapshots and complete histories feed the same item renderer.
pub fn format_labeled_requests_snapshot(
    scenario: &str,
    sections: &[(&str, &ResponsesRequest)],
    options: &ContextSnapshotOptions,
) -> String {
    let entries = sections
        .iter()
        .map(|(title, request)| SnapshotEntry::captured(request).labeled(title))
        .collect::<Vec<_>>();
    format_context_snapshot(scenario, &entries, options)
}

/// Show every captured `/responses` request. A new window starts when a request no longer
/// extends its predecessor's input or changes request settings. No server window IDs are
/// used to infer boundaries.
pub fn format_request_history_snapshot(
    scenario: &str,
    requests: &[ResponsesRequest],
    options: &ContextSnapshotOptions,
) -> String {
    let entries = requests
        .iter()
        .map(SnapshotEntry::captured)
        .collect::<Vec<_>>();
    format_context_snapshot(scenario, &entries, options)
}

pub fn format_context_snapshot(
    scenario: &str,
    entries: &[SnapshotEntry<'_>],
    options: &ContextSnapshotOptions,
) -> String {
    let windows = group_requests(
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| capture_request(index + 1, entry)),
    );
    let mut normalizer = Normalizer::default();
    let mut result = format!("Scenario: {scenario}");
    for (index, window) in windows.iter().enumerate() {
        result.push_str(&format!("\n\n## Window {}", index + 1));
        if let Some(previous) = index.checked_sub(1) {
            result.push_str(&format!(
                " (after request {}",
                window.requests[0].number - 1
            ));
            if let Some(boundary) = &window.boundary {
                result.push_str(&match boundary {
                    InputBoundary::Truncated(at) => format!(": input truncated at item {at:02}"),
                    InputBoundary::Diverged(at) => format!(": input diverged at item {at:02}"),
                    InputBoundary::Repeated => ": input repeated".to_string(),
                    InputBoundary::SettingsUnavailable => ": settings unavailable".to_string(),
                });
            }
            if !options.include_request_settings {
                let changed = windows[previous]
                    .settings
                    .as_ref()
                    .zip(window.settings.as_ref())
                    .filter(|(old, new)| old != new)
                    .map(|(old, new)| changed_setting_keys(new, Some(old)).join(", "));
                if let Some(changed) = changed {
                    let separator = if window.boundary.is_some() {
                        ", "
                    } else {
                        ": "
                    };
                    result.push_str(&format!("{separator}settings changed ({changed})"));
                }
            }
            result.push(')');
        }
        if options.include_request_settings {
            render_window_settings(&mut result, &windows, index, &mut normalizer);
        }
        for request in &window.requests {
            let label = request
                .label
                .as_deref()
                .map(|label| format!("; {label}"))
                .unwrap_or_default();
            result.push_str(&format!(
                "\n-- request {} ({}{label}) --",
                request.number, request.kind
            ));
            let suffix = &request.input[request.suffix_start..];
            if !suffix.is_empty() {
                result.push('\n');
                result.push_str(&render_items(
                    suffix,
                    request.suffix_start,
                    options,
                    &mut normalizer,
                ));
            } else if request.number == window.requests[0].number {
                result.push_str("\n<EMPTY_INPUT>");
            }
        }
    }
    result
}

fn render_window_settings(
    result: &mut String,
    windows: &[Window],
    index: usize,
    normalizer: &mut Normalizer,
) {
    let Some(settings) = &windows[index].settings else {
        result.push_str("\nSettings: unavailable (items only)");
        return;
    };
    let Some(previous) = index.checked_sub(1) else {
        result.push_str("\nSettings:");
        result.push_str(&render_settings(
            settings, /*previous*/ None, normalizer,
        ));
        return;
    };
    if let Some(same) = windows[..index]
        .iter()
        .position(|earlier| earlier.settings.as_ref() == Some(settings))
    {
        result.push_str(&format!("\nSettings: same as window {}", same + 1));
        return;
    }
    if windows[previous].settings.is_some() {
        result.push_str(&format!("\nSettings: relative to window {}", previous + 1));
    } else {
        result.push_str("\nSettings:");
    }
    result.push_str(&render_settings(
        settings,
        windows[previous].settings.as_ref(),
        normalizer,
    ));
}

fn render_settings(
    settings: &Value,
    previous: Option<&Value>,
    normalizer: &mut Normalizer,
) -> String {
    let mut lines = Vec::new();
    let fields = settings.as_object().expect("request settings object");
    for key in changed_setting_keys(settings, previous) {
        let Some(value) = fields.get(&key) else {
            lines.push(format!("  {key}: <removed>"));
            continue;
        };
        match (key.as_str(), value) {
            ("prompt_cache_key", Value::String(key)) => {
                lines.push(format!(
                    "  prompt_cache_key: \"{}\"",
                    normalizer.prompt_cache_key(key)
                ));
            }
            ("instructions", Value::String(text)) => {
                let text = normalize_line_endings(text);
                let first = text.lines().next().unwrap_or_default();
                lines.push(format!(
                    "  instructions ({} lines, {} chars, hash={}): {}",
                    text.lines().count(),
                    text.chars().count(),
                    fingerprint(value),
                    shorten(
                        first, /*limit*/ 120, /*keep_tail*/ false, /*hash*/ false
                    )
                ));
            }
            ("tools", Value::Array(tools)) => {
                let prior = previous
                    .and_then(|old| old.get("tools"))
                    .and_then(Value::as_array)
                    .map(Vec::as_slice);
                lines.push(format!(
                    "  tools ({}; hash={}):",
                    tools.len(),
                    fingerprint(&Value::Array(
                        tools.iter().map(portable_tool_schema).collect()
                    ))
                ));
                lines.extend(render_tools(tools, prior));
            }
            _ => {
                let mut normalized = value.clone();
                normalize_json(&mut normalized, &mut |text| {
                    normalize_stable_text(&normalizer.normalize_uuids(text))
                });
                lines.push(format!("  {key}: {normalized}"));
            }
        }
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("\n{}", lines.join("\n"))
    }
}

fn changed_setting_keys(settings: &Value, previous: Option<&Value>) -> Vec<String> {
    let fields = settings.as_object().expect("request settings object");
    let mut keys = fields
        .keys()
        .chain(
            previous
                .into_iter()
                .flat_map(|old| old.as_object().expect("previous settings object").keys()),
        )
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    keys.retain(|key| previous.is_none_or(|old| old.get(key) != fields.get(key)));
    keys
}

fn render_tools(tools: &[Value], previous: Option<&[Value]>) -> Vec<String> {
    let Some(previous) = previous else {
        return tools.iter().map(|tool| render_tool(tool, '-')).collect();
    };
    let mut labels = std::collections::HashSet::new();
    if !previous.iter().all(|tool| labels.insert(tool_label(tool))) {
        return vec!["    - inventory changed (ambiguous tool names)".to_string()];
    }
    labels.clear();
    if !tools.iter().all(|tool| labels.insert(tool_label(tool))) {
        return vec!["    - inventory changed (ambiguous tool names)".to_string()];
    }
    let mut changes = Vec::new();
    for old in previous {
        if !tools.iter().any(|tool| tool_label(tool) == tool_label(old)) {
            changes.push(format!("    - removed {}", tool_label(old)));
        }
    }
    for tool in tools {
        match previous
            .iter()
            .find(|old| tool_label(old) == tool_label(tool))
        {
            None => changes.push(render_tool(tool, '+')),
            Some(old) if old != tool => {
                changes.push(render_tool(tool, '~'));
            }
            _ => {}
        }
    }
    if changes.is_empty() {
        changes.push("    - order changed".to_string());
    }
    changes
}

fn render_tool(tool: &Value, marker: char) -> String {
    let stable = portable_tool_schema(tool);
    let tool = &stable;
    let label = tool_label(tool);
    let definition = tool.get("function").unwrap_or(tool);
    let description = definition
        .get("description")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(|text| {
            format!(
                ": {}",
                shorten(
                    &text.replace('\n', " "),
                    /*limit*/ 100,
                    /*keep_tail*/ false,
                    /*hash*/ false,
                )
            )
        })
        .unwrap_or_default();
    let args = definition
        .get("parameters")
        .and_then(|parameters| parameters.get("properties"))
        .and_then(Value::as_object)
        .map(|properties| {
            let mut names = properties.keys().cloned().collect::<Vec<_>>();
            names.sort();
            format!("; args=[{}]", names.join(", "))
        })
        .unwrap_or_default();
    let mut rendered = format!(
        "    {marker} {label}{description}{args}; hash={}",
        fingerprint(tool)
    );
    if let Some(members) = definition.get("tools").and_then(Value::as_array) {
        for member in members {
            rendered.push_str(&format!("\n      - {}", tool_label(member)));
        }
    }
    rendered
}

// Shell guidance and its yield-time wording intentionally differ on Windows. Compare all other
// tool schema fields, while keeping the context snapshots identical across operating systems.
fn portable_tool_schema(tool: &Value) -> Value {
    const BASE: &str =
        "Runs a command in a PTY, returning output or a session ID for ongoing interaction.";
    // Exact current Windows-only suffix. A change to its wording must be reviewed explicitly.
    const WINDOWS_SAFETY_SUFFIX_HASH: u64 = 0x6de3_23e4_7060_6128;
    const UNIX_WAIT: &str =
        "Wait before yielding output. Defaults to 10000 ms; effective range is 250-30000 ms.";
    const WINDOWS_WAIT: &str = "Maximum time to wait before returning a session ID for a still-running command. Commands that finish sooner return immediately. For ordinary commands, omit this parameter to use the 10000 ms default. Effective range on Windows is 10000-30000 ms.";
    let mut stable = tool.clone();
    if let Some(Value::Array(members)) = stable.get_mut("tools") {
        for member in members {
            *member = portable_tool_schema(member);
        }
    }
    let definition = if stable.get("function").is_some() {
        stable
            .get_mut("function")
            .expect("function tool definition")
    } else {
        &mut stable
    };
    if definition.get("name").and_then(Value::as_str) == Some("exec") {
        if let Some(Value::String(description)) = definition.get_mut("description") {
            // Code mode embeds the exec_command description and its TypeScript declaration
            // inside the exec tool. Apply the same narrow platform normalization there.
            if let Some((start, section)) = description.split_once("### `exec_command`\n")
                && let Some(rest) = section.strip_prefix(BASE)
                && let Some((suffix, _)) = rest.split_once("\n\nexec tool declaration:")
                && suffix.starts_with("\n\nWindows safety rules:")
                && fnv1a(normalize_line_endings(suffix).as_bytes()) == WINDOWS_SAFETY_SUFFIX_HASH
            {
                *description =
                    format!("{start}### `exec_command`\n{BASE}{}", &rest[suffix.len()..]);
            }
            for platform_wait in [UNIX_WAIT, WINDOWS_WAIT] {
                *description = description.replace(
                    &format!("  // {platform_wait}"),
                    "  // <PLATFORM_WAIT_GUIDANCE>",
                );
            }
        }
        return stable;
    }
    if definition.get("name").and_then(Value::as_str) != Some("exec_command") {
        return stable;
    }
    if let Some(Value::String(description)) = definition.get_mut("description")
        && description.strip_prefix(BASE).is_some_and(|rest| {
            rest.starts_with("\n\nWindows safety rules:")
                && fnv1a(normalize_line_endings(rest).as_bytes()) == WINDOWS_SAFETY_SUFFIX_HASH
        })
    {
        *description = BASE.to_string();
    }
    if let Some(Value::String(wait)) =
        definition.pointer_mut("/parameters/properties/yield_time_ms/description")
        && (wait == UNIX_WAIT || wait == WINDOWS_WAIT)
    {
        *wait = "<PLATFORM_WAIT_GUIDANCE>".to_string();
    }
    stable
}

fn tool_label(tool: &Value) -> String {
    let kind = tool
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let definition = tool.get("function").unwrap_or(tool);
    let name = definition
        .get("name")
        .or_else(|| tool.get("server_label"))
        .and_then(Value::as_str);
    name.map_or_else(|| kind.to_string(), |name| format!("{kind}/{name}"))
}

fn render_items(
    items: &[Value],
    start_index: usize,
    options: &ContextSnapshotOptions,
    normalizer: &mut Normalizer,
) -> String {
    items
        .iter()
        .enumerate()
        .map(|(offset, item)| {
            let rendered = render_item(start_index + offset, item, options, normalizer)
                .replace('\n', "\n    ");
            rendered
                .split('\n')
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_item(
    index: usize,
    item: &Value,
    options: &ContextSnapshotOptions,
    normalizer: &mut Normalizer,
) -> String {
    let Some(kind) = item.get("type").and_then(Value::as_str) else {
        return format!("{index:02}:<MISSING_TYPE>");
    };
    match kind {
        "message" => render_message(index, item, options, normalizer),
        "additional_tools" => {
            let Some(tools) = item.get("tools").and_then(Value::as_array) else {
                return format!("{index:02}:additional_tools:<MISSING_TOOLS>");
            };
            let portable = tools.iter().map(portable_tool_schema).collect();
            let mut lines = vec![format!(
                "{index:02}:additional_tools/{} ({}; hash={}):",
                item.get("role")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                tools.len(),
                fingerprint(&Value::Array(portable))
            )];
            lines.extend(render_tools(tools, /*previous*/ None));
            lines.join("\n")
        }
        "function_call" => {
            let name = call_name(item);
            let args = item
                .get("arguments")
                .and_then(Value::as_str)
                .map(|text| {
                    let normalized = serde_json::from_str::<Value>(text)
                        .map(|mut value| {
                            normalize_json(&mut value, &mut |text| {
                                normalize_stable_text(&normalizer.normalize_uuids(text))
                            });
                            value.to_string()
                        })
                        .unwrap_or_else(|_| text.to_string());
                    render_text(&normalized, options, normalizer)
                })
                .unwrap_or_else(|| "<NO_ARGUMENTS>".to_string());
            format!("{index:02}:function_call/{name}:{args}")
        }
        "custom_tool_call" => {
            let name = call_name(item);
            let input = item
                .get("input")
                .and_then(Value::as_str)
                .map(|input| render_text(input, options, normalizer))
                .unwrap_or_else(|| "<NO_INPUT>".to_string());
            format!("{index:02}:custom_tool_call/{name}:{input}")
        }
        "function_call_output" | "custom_tool_call_output" => {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .map(|_| format!("/{}", call_name(item)))
                .or_else(|| {
                    item.get("namespace")
                        .and_then(Value::as_str)
                        .map(|namespace| format!("[namespace={namespace}]"))
                })
                .unwrap_or_default();
            let output = item
                .get("output")
                .map(|output| match output {
                    Value::String(text) => render_text(text, options, normalizer),
                    Value::Array(parts) => parts
                        .iter()
                        .map(|part| {
                            part.get("text")
                                .and_then(Value::as_str)
                                .map(|text| render_text(text, options, normalizer))
                                .unwrap_or_else(|| format!("<{}>", tool_label(part)))
                        })
                        .collect::<Vec<_>>()
                        .join(" | "),
                    Value::Object(fields) => {
                        let content = fields
                            .get("content")
                            .and_then(Value::as_str)
                            .map(|text| render_text(text, options, normalizer))
                            .unwrap_or_else(|| "<NO_TEXT>".to_string());
                        match fields.get("success").and_then(Value::as_bool) {
                            Some(success) => format!("success={success}:{content}"),
                            None => content,
                        }
                    }
                    _ => "<NON_TEXT_OUTPUT>".to_string(),
                })
                .unwrap_or_else(|| "<NO_OUTPUT>".to_string());
            format!("{index:02}:{kind}{name}:{output}")
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
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .filter(|text| !text.is_empty())
                .map(|text| render_text(&text, options, normalizer))
                .unwrap_or_else(|| "<NO_COMMAND>".to_string());
            format!("{index:02}:local_shell_call:{command}")
        }
        "reasoning" => {
            let summary = item
                .get("summary")
                .and_then(Value::as_array)
                .and_then(|entries| entries.first())
                .and_then(|entry| entry.get("text"))
                .and_then(Value::as_str)
                .map(|text| render_text(text, options, normalizer))
                .unwrap_or_else(|| "<NO_SUMMARY>".to_string());
            let encrypted = has_encrypted_content(item);
            format!("{index:02}:reasoning:summary={summary}:encrypted={encrypted}")
        }
        "compaction" => {
            let encrypted = item
                .get("encrypted_content")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty());
            match encrypted {
                Some(content) => format!(
                    "{index:02}:compaction:encrypted=true; chars={}; hash={}",
                    content.chars().count(),
                    fingerprint(&Value::String(content.to_owned()))
                ),
                None => format!("{index:02}:compaction:encrypted=false"),
            }
        }
        other => format!("{index:02}:{other}"),
    }
}

fn call_name(item: &Value) -> String {
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match item.get("namespace").and_then(Value::as_str) {
        Some(namespace) => format!("{namespace}.{name}"),
        None => name.to_string(),
    }
}

fn has_encrypted_content(item: &Value) -> bool {
    item.get("encrypted_content")
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty())
}

fn render_message(
    index: usize,
    item: &Value,
    options: &ContextSnapshotOptions,
    normalizer: &mut Normalizer,
) -> String {
    let role = item
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let parts = item
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|part| {
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                return render_text(text, options, normalizer);
            }
            let Some(kind) = part.get("type").and_then(Value::as_str) else {
                return "<UNKNOWN_CONTENT_ITEM>".to_string();
            };
            let mut keys = part
                .as_object()
                .into_iter()
                .flat_map(|part| part.keys())
                .filter(|key| *key != "type" && *key != "text")
                .cloned()
                .collect::<Vec<_>>();
            keys.sort();
            if keys.is_empty() {
                format!("<{kind}>")
            } else {
                format!("<{kind}:{}>", keys.join(","))
            }
        })
        .collect::<Vec<_>>();
    let role = if parts.len() > 1 {
        format!("{role}[{}]", parts.len())
    } else {
        role.to_string()
    };
    match parts.as_slice() {
        [] => format!("{index:02}:message/{role}:\n<NO_TEXT>"),
        [part] => format!("{index:02}:message/{role}:\n{part}"),
        _ => format!(
            "{index:02}:message/{role}:\n{}",
            parts
                .iter()
                .enumerate()
                .map(|(number, part)| format!(
                    "[{:02}] {}",
                    number + 1,
                    part.replace('\n', "\n    ")
                ))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    }
}
