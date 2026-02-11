use crate::error::CodexErr;
use crate::error::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;

/// System prompt for stage-1 raw memory extraction.
pub(super) const RAW_MEMORY_PROMPT: &str =
    include_str!("../../templates/memories/stage_one_system.md");

static OPENAI_KEY_REGEX: Lazy<Regex> = Lazy::new(|| compile_regex(r"sk-[A-Za-z0-9]{20,}"));
static AWS_ACCESS_KEY_ID_REGEX: Lazy<Regex> = Lazy::new(|| compile_regex(r"\bAKIA[0-9A-Z]{16}\b"));
static BEARER_TOKEN_REGEX: Lazy<Regex> =
    Lazy::new(|| compile_regex(r"(?i)\bBearer\s+[A-Za-z0-9._\-]{16,}\b"));
static SECRET_ASSIGNMENT_REGEX: Lazy<Regex> = Lazy::new(|| {
    compile_regex(r#"(?i)\b(api[_-]?key|token|secret|password)\b(\s*[:=]\s*)(["']?)[^\s"']{8,}"#)
});

/// Parsed stage-1 model output payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StageOneOutput {
    /// Detailed markdown raw memory for a single rollout.
    #[serde(rename = "raw_memory")]
    pub(crate) raw_memory: String,
    /// Compact summary line used for routing and indexing.
    #[serde(rename = "rollout_summary")]
    pub(crate) rollout_summary: String,
    /// Optional slug accepted from stage-1 output for forward compatibility.
    ///
    /// This is currently ignored by downstream storage and naming, which remain
    /// thread-id based.
    #[serde(default, rename = "rollout_slug")]
    pub(crate) _rollout_slug: Option<String>,
}

/// JSON schema used to constrain stage-1 model output.
pub(super) fn stage_one_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "rollout_summary": { "type": "string" },
            "rollout_slug": { "type": "string" },
            "raw_memory": { "type": "string" }
        },
        "required": ["rollout_summary", "rollout_slug", "raw_memory"],
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
    output._rollout_slug = output
        ._rollout_slug
        .map(|slug| slug.trim().to_string())
        .filter(|slug| !slug.is_empty());

    if output.raw_memory.is_empty() && output.rollout_summary.is_empty() {
        // Empty pair is a deliberate "no meaningful signal" sentinel.
        return Ok(output);
    }

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

    output.raw_memory = redact_secrets(&output.raw_memory);
    output.rollout_summary = redact_secrets(&output.rollout_summary);

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

fn compile_regex(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(regex) => regex,
        // Panic is ok thanks to `load_regex` test.
        Err(err) => panic!("invalid regex pattern `{pattern}`: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_regex() {
        // The goal of this test is just to compile all the regex to prevent the panic
        let _ = redact_secrets("secret");
    }

    #[test]
    fn normalize_stage_one_output_redacts_summary() {
        let output = StageOneOutput {
            raw_memory: "Token: sk-abcdefghijklmnopqrstuvwxyz123456\nBearer abcdefghijklmnopqrstuvwxyz012345".to_string(),
            rollout_summary: "password = mysecret123456\n\nsmall".to_string(),
            _rollout_slug: None,
        };

        let normalized = normalize_stage_one_output(output).expect("normalized");

        assert!(normalized.raw_memory.contains("[REDACTED_SECRET]"));
        assert!(!normalized.rollout_summary.contains("mysecret123456"));
        assert_eq!(
            normalized.rollout_summary,
            "password = [REDACTED_SECRET]\n\nsmall"
        );
    }

    #[test]
    fn normalize_stage_one_output_allows_empty_pair_for_skip() {
        let output = StageOneOutput {
            raw_memory: String::new(),
            rollout_summary: String::new(),
            _rollout_slug: None,
        };

        let normalized = normalize_stage_one_output(output).expect("normalized");
        assert_eq!(normalized.raw_memory, "");
        assert_eq!(normalized.rollout_summary, "");
    }

    #[test]
    fn normalize_stage_one_output_rejects_partial_empty_values() {
        let output = StageOneOutput {
            raw_memory: String::new(),
            rollout_summary: "summary".to_string(),
            _rollout_slug: None,
        };

        let err = normalize_stage_one_output(output).expect_err("should reject");
        assert_eq!(err.to_string(), "stage-1 memory output missing raw_memory");
    }
}
