use crate::error::CodexErr;
use crate::error::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use serde_json::json;

use super::types::StageOneOutput;

/// System prompt for stage-1 trace memory extraction.
pub(crate) const TRACE_MEMORY_PROMPT: &str =
    include_str!("../../templates/memories/stage_one_system.md");
const MAX_STAGE_ONE_TRACE_MEMORY_CHARS: usize = 300_000;
const MAX_STAGE_ONE_SUMMARY_CHARS: usize = 1_200;

static OPENAI_KEY_REGEX: Lazy<Regex> = Lazy::new(|| compile_regex(r"sk-[A-Za-z0-9]{20,}"));
static AWS_ACCESS_KEY_ID_REGEX: Lazy<Regex> = Lazy::new(|| compile_regex(r"\bAKIA[0-9A-Z]{16}\b"));
static BEARER_TOKEN_REGEX: Lazy<Regex> =
    Lazy::new(|| compile_regex(r"(?i)\bBearer\s+[A-Za-z0-9._\-]{16,}\b"));
static SECRET_ASSIGNMENT_REGEX: Lazy<Regex> = Lazy::new(|| {
    compile_regex(r#"(?i)\b(api[_-]?key|token|secret|password)\b(\s*[:=]\s*)(["']?)[^\s"']{8,}"#)
});

/// JSON schema used to constrain stage-1 model output.
pub(crate) fn stage_one_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "traceMemory": { "type": "string" },
            "summary": { "type": "string" }
        },
        "required": ["traceMemory", "summary"],
        "additionalProperties": false
    })
}

/// Parses and normalizes stage-1 model output into a typed payload.
///
/// Accepts plain JSON objects, fenced JSON, and object snippets embedded in
/// extra text, then enforces redaction and size limits.
pub(crate) fn parse_stage_one_output(raw: &str) -> Result<StageOneOutput> {
    let parsed = parse_json_object_loose(raw)?;
    let output: StageOneOutput = serde_json::from_value(parsed).map_err(|err| {
        CodexErr::InvalidRequest(format!("invalid stage-1 memory output JSON payload: {err}"))
    })?;
    normalize_stage_one_output(output)
}

fn parse_json_object_loose(raw: &str) -> Result<Value> {
    let raw = raw.trim();

    if let Ok(value) = serde_json::from_str::<Value>(raw)
        && value.is_object()
    {
        return Ok(value);
    }

    if let Some(fenced) = raw
        .strip_prefix("```json")
        .and_then(|s| s.strip_suffix("```"))
        .map(str::trim)
        && let Ok(value) = serde_json::from_str::<Value>(fenced)
        && value.is_object()
    {
        return Ok(value);
    }

    if let Some(fenced) = raw
        .strip_prefix("```")
        .and_then(|s| s.strip_suffix("```"))
        .map(str::trim)
        && let Ok(value) = serde_json::from_str::<Value>(fenced)
        && value.is_object()
    {
        return Ok(value);
    }

    if let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}'))
        && start < end
    {
        let snippet = &raw[start..=end];
        if let Ok(value) = serde_json::from_str::<Value>(snippet)
            && value.is_object()
        {
            return Ok(value);
        }
    }

    Err(CodexErr::InvalidRequest(
        "unable to parse stage-1 memory JSON output".to_string(),
    ))
}

fn prefix_at_char_boundary(input: &str, max_bytes: usize) -> &str {
    if max_bytes >= input.len() {
        return input;
    }
    let mut end = 0;
    for (idx, _) in input.char_indices() {
        if idx > max_bytes {
            break;
        }
        end = idx;
    }
    &input[..end]
}

fn suffix_at_char_boundary(input: &str, max_bytes: usize) -> &str {
    if max_bytes >= input.len() {
        return input;
    }
    let start_limit = input.len().saturating_sub(max_bytes);
    let mut start = input.len();
    for (idx, _) in input.char_indices().rev() {
        if idx < start_limit {
            break;
        }
        start = idx;
    }
    &input[start..]
}

fn normalize_stage_one_output(mut output: StageOneOutput) -> Result<StageOneOutput> {
    output.trace_memory = output.trace_memory.trim().to_string();
    output.summary = output.summary.trim().to_string();

    if output.trace_memory.is_empty() {
        return Err(CodexErr::InvalidRequest(
            "stage-1 memory output missing traceMemory".to_string(),
        ));
    }
    if output.summary.is_empty() {
        return Err(CodexErr::InvalidRequest(
            "stage-1 memory output missing summary".to_string(),
        ));
    }

    output.trace_memory = normalize_trace_memory_structure(&redact_secrets(&output.trace_memory));
    output.summary = redact_secrets(&compact_whitespace(&output.summary));

    if output.trace_memory.len() > MAX_STAGE_ONE_TRACE_MEMORY_CHARS {
        output.trace_memory = truncate_text_for_storage(
            &output.trace_memory,
            MAX_STAGE_ONE_TRACE_MEMORY_CHARS,
            "\n\n[... TRACE MEMORY TRUNCATED ...]\n\n",
        );
    }

    if output.summary.len() > MAX_STAGE_ONE_SUMMARY_CHARS {
        output.summary = truncate_text_for_storage(
            &output.summary,
            MAX_STAGE_ONE_SUMMARY_CHARS,
            " [...summary truncated...]",
        );
    }

    Ok(output)
}

fn compact_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn redact_secrets(input: &str) -> String {
    let redacted = OPENAI_KEY_REGEX.replace_all(input, "[REDACTED_SECRET]");
    let redacted = AWS_ACCESS_KEY_ID_REGEX.replace_all(&redacted, "[REDACTED_SECRET]");
    let redacted = BEARER_TOKEN_REGEX.replace_all(&redacted, "Bearer [REDACTED_SECRET]");

    SECRET_ASSIGNMENT_REGEX
        .replace_all(&redacted, "$1$2$3[REDACTED_SECRET]")
        .to_string()
}

fn normalize_trace_memory_structure(input: &str) -> String {
    if has_trace_memory_structure(input) {
        return input.to_string();
    }

    format!(
        "# Trace Summary\n\
Trace context: extracted from rollout (normalized fallback structure).\n\
User preferences: none observed\n\n\
## Task: Extracted Memory\n\
Outcome: uncertain\n\
Key steps:\n\
- Review raw notes captured below.\n\
Things that did not work / things that can be improved:\n\
- Not clearly captured in structured form.\n\
Reusable knowledge:\n\
- Re-validate critical claims against the current rollout.\n\
Pointers and references (annotate why each item matters):\n\
- Raw trace notes included below.\n\n\
### Raw trace notes\n\
{input}\n"
    )
}

fn has_trace_memory_structure(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.starts_with('#')
        && trimmed.contains("Trace context:")
        && trimmed.contains("User preferences:")
        && trimmed.contains("## Task:")
        && trimmed.contains("Outcome:")
}

fn truncate_text_for_storage(input: &str, max_bytes: usize, marker: &str) -> String {
    if input.len() <= max_bytes {
        return input.to_string();
    }

    let budget_without_marker = max_bytes.saturating_sub(marker.len());
    let head_budget = budget_without_marker / 2;
    let tail_budget = budget_without_marker.saturating_sub(head_budget);
    let head = prefix_at_char_boundary(input, head_budget);
    let tail = suffix_at_char_boundary(input, tail_budget);

    format!("{head}{marker}{tail}")
}

fn compile_regex(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(err) => panic!("invalid regex pattern `{pattern}`: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_stage_one_output_redacts_and_compacts_summary() {
        let output = StageOneOutput {
            trace_memory: "Token: sk-abcdefghijklmnopqrstuvwxyz123456\nBearer abcdefghijklmnopqrstuvwxyz012345".to_string(),
            summary: "password = mysecret123456\n\nsmall".to_string(),
        };

        let normalized = normalize_stage_one_output(output).expect("normalized");

        assert!(normalized.trace_memory.contains("[REDACTED_SECRET]"));
        assert!(!normalized.summary.contains("mysecret123456"));
        assert_eq!(normalized.summary, "password = [REDACTED_SECRET] small");
    }

    #[test]
    fn normalize_trace_memory_structure_wraps_unstructured_content() {
        let normalized = normalize_trace_memory_structure("loose notes only");
        assert!(normalized.starts_with("# Trace Summary"));
        assert!(normalized.contains("Trace context:"));
        assert!(normalized.contains("## Task:"));
        assert!(normalized.contains("Outcome: uncertain"));
        assert!(normalized.contains("loose notes only"));
    }
}
