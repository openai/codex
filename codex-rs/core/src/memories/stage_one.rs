use crate::error::CodexErr;
use crate::error::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use serde_json::json;

use super::StageOneOutput;
use super::text::compact_whitespace;
use super::text::truncate_text_for_storage;

/// System prompt for stage-1 raw memory extraction.
pub(super) const RAW_MEMORY_PROMPT: &str =
    include_str!("../../templates/memories/stage_one_system.md");
const MAX_STAGE_ONE_RAW_MEMORY_CHARS: usize = 300_000;
const MAX_STAGE_ONE_SUMMARY_CHARS: usize = 1_200;

static OPENAI_KEY_REGEX: Lazy<Regex> = Lazy::new(|| compile_regex(r"sk-[A-Za-z0-9]{20,}"));
static AWS_ACCESS_KEY_ID_REGEX: Lazy<Regex> = Lazy::new(|| compile_regex(r"\bAKIA[0-9A-Z]{16}\b"));
static BEARER_TOKEN_REGEX: Lazy<Regex> =
    Lazy::new(|| compile_regex(r"(?i)\bBearer\s+[A-Za-z0-9._\-]{16,}\b"));
static SECRET_ASSIGNMENT_REGEX: Lazy<Regex> = Lazy::new(|| {
    compile_regex(r#"(?i)\b(api[_-]?key|token|secret|password)\b(\s*[:=]\s*)(["']?)[^\s"']{8,}"#)
});

/// JSON schema used to constrain stage-1 model output.
pub(super) fn stage_one_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "rollout_summary": { "type": "string" },
            "raw_memory": { "type": "string" }
        },
        "required": ["rollout_summary", "raw_memory"],
        "additionalProperties": false
    })
}

/// Parses and normalizes stage-1 model output into a typed payload.
///
/// Accepts plain JSON objects, fenced JSON, and object snippets embedded in
/// extra text, then enforces redaction and size limits.
pub(super) fn parse_stage_one_output(raw: &str) -> Result<StageOneOutput> {
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

fn normalize_stage_one_output(mut output: StageOneOutput) -> Result<StageOneOutput> {
    output.raw_memory = output.raw_memory.trim().to_string();
    output.rollout_summary = output.rollout_summary.trim().to_string();

    if output.raw_memory.is_empty() {
        return Err(CodexErr::InvalidRequest(
            "stage-1 memory output missing raw_memory".to_string(),
        ));
    }
    if output.rollout_summary.is_empty() {
        return Err(CodexErr::InvalidRequest(
            "stage-1 memory output missing rollout_summary".to_string(),
        ));
    }

    output.raw_memory = normalize_raw_memory_structure(&redact_secrets(&output.raw_memory));
    output.rollout_summary = redact_secrets(&compact_whitespace(&output.rollout_summary));

    if output.raw_memory.len() > MAX_STAGE_ONE_RAW_MEMORY_CHARS {
        output.raw_memory = truncate_text_for_storage(
            &output.raw_memory,
            MAX_STAGE_ONE_RAW_MEMORY_CHARS,
            "\n\n[... RAW MEMORY TRUNCATED ...]\n\n",
        );
    }

    if output.rollout_summary.len() > MAX_STAGE_ONE_SUMMARY_CHARS {
        output.rollout_summary = truncate_text_for_storage(
            &output.rollout_summary,
            MAX_STAGE_ONE_SUMMARY_CHARS,
            " [...summary truncated...]",
        );
    }

    Ok(output)
}

fn redact_secrets(input: &str) -> String {
    let redacted = OPENAI_KEY_REGEX.replace_all(input, "[REDACTED_SECRET]");
    let redacted = AWS_ACCESS_KEY_ID_REGEX.replace_all(&redacted, "[REDACTED_SECRET]");
    let redacted = BEARER_TOKEN_REGEX.replace_all(&redacted, "Bearer [REDACTED_SECRET]");

    SECRET_ASSIGNMENT_REGEX
        .replace_all(&redacted, "$1$2$3[REDACTED_SECRET]")
        .to_string()
}

fn normalize_raw_memory_structure(input: &str) -> String {
    if has_raw_memory_structure(input) {
        return input.to_string();
    }

    format!(
        "# Raw Memory\n\
Memory context: extracted from rollout (normalized fallback structure).\n\
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
- Raw memory notes included below.\n\n\
### Raw memory notes\n\
{input}\n"
    )
}

fn has_raw_memory_structure(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.starts_with('#')
        && (trimmed.contains("Memory context:") || trimmed.contains("Trace context:"))
        && trimmed.contains("User preferences:")
        && trimmed.contains("## Task:")
        && trimmed.contains("Outcome:")
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
            raw_memory: "Token: sk-abcdefghijklmnopqrstuvwxyz123456\nBearer abcdefghijklmnopqrstuvwxyz012345".to_string(),
            rollout_summary: "password = mysecret123456\n\nsmall".to_string(),
        };

        let normalized = normalize_stage_one_output(output).expect("normalized");

        assert!(normalized.raw_memory.contains("[REDACTED_SECRET]"));
        assert!(!normalized.rollout_summary.contains("mysecret123456"));
        assert_eq!(
            normalized.rollout_summary,
            "password = [REDACTED_SECRET] small"
        );
    }

    #[test]
    fn normalize_raw_memory_structure_wraps_unstructured_content() {
        let normalized = normalize_raw_memory_structure("loose notes only");
        assert!(normalized.starts_with("# Raw Memory"));
        assert!(normalized.contains("Memory context:"));
        assert!(normalized.contains("## Task:"));
        assert!(normalized.contains("Outcome: uncertain"));
        assert!(normalized.contains("loose notes only"));
    }
}
