use codex_otel::MetricsClient;

use crate::MEMORY_TOOLS_NAMESPACE;

pub(crate) const MEMORIES_TOOL_CALL_METRIC: &str = "codex.memories.tool.call";

pub(crate) fn record_tool_call(
    metrics_client: Option<&MetricsClient>,
    operation: &str,
    scope: &str,
    success: bool,
    truncated: &str,
) {
    let Some(metrics_client) = metrics_client else {
        return;
    };

    let tool = format!("{MEMORY_TOOLS_NAMESPACE}{operation}");
    let _ = metrics_client.counter(
        MEMORIES_TOOL_CALL_METRIC,
        /*inc*/ 1,
        &[
            ("tool", tool.as_str()),
            ("operation", operation),
            ("scope", scope),
            ("status", status_tag(success)),
            ("truncated", truncated),
        ],
    );
}

pub(crate) fn scope_from_path(path: &str) -> &'static str {
    let path = path.trim_matches('/');
    let path = path.strip_prefix("./").unwrap_or(path);

    if path.is_empty() {
        "root"
    } else if path == "MEMORY.md" {
        "memory_md"
    } else if path == "memory_summary.md" {
        "memory_summary"
    } else if path == "raw_memories.md" {
        "raw_memories"
    } else if path == "rollout_summaries" || path.starts_with("rollout_summaries/") {
        "rollout_summaries"
    } else if path == "skills" || path.starts_with("skills/") {
        "skills"
    } else if path == "extensions/ad_hoc/notes" || path.starts_with("extensions/ad_hoc/notes/") {
        "ad_hoc_notes"
    } else {
        "other"
    }
}

pub(crate) fn scope_from_optional_path(path: Option<&str>, default: &'static str) -> &'static str {
    path.map_or(default, scope_from_path)
}

pub(crate) fn truncated_tag(truncated: Option<bool>) -> &'static str {
    match truncated {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}

fn status_tag(success: bool) -> &'static str {
    if success { "succeeded" } else { "failed" }
}
