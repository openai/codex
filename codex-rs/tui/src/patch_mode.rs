// src/patch_mode.rs
//
// Patch-first Builder wiring: prompts, bundle builder and strict output enforcement.
// Code in English; fully commented; no network or remote git involved.

use std::fs;
use std::path::Path;

use color_eyre::eyre::{bail, eyre, Result};
use std::borrow::Cow;

use crate::git_guard::{self, DiffEnvelope};

/// High-authority addendum injected into the Builder's system/preamble.
/// Forces the model to emit ONLY a Diff Envelope (no prose).
pub fn builder_system_addendum() -> String {
    r#"ROLE: You are the Builder in PATCH-FIRST mode.
OUTPUT POLICY:
- Your ONLY output must be a Diff Envelope matching the exact format below.
- Do NOT include explanations, markdown fences, JSON, or any prose.
- Keep changes minimal and within the Change Contract constraints.

DIFF ENVELOPE FORMAT (exactly, plain text; no backticks):
<diff_envelope>
base_ref: {BASE_REF}        # from input bundle
task_id: {TASK_ID}          # from input bundle/PRD
rationale: "{ONE_SENTENCE_REASON}"
diff_format: "unified"
---BEGIN DIFF---
... unified diff here ...
---END DIFF---
</diff_envelope>

HARD RULES:
- Respect allowed_paths / deny_paths / budgets. No renames/deletes unless allowed.
- If the task truly cannot be done within constraints, produce the smallest viable diff and keep within scope.
- No file I/O commands, no shell, no test logs. Only the envelope above.
"#
    .to_string()
}

/// Build the Builder payload with PRD tasks, the Change Contract and a tiny protocol crib.
pub fn build_builder_bundle(
    prd_path: &Path,
    change_contract_yaml: &str,
    base_ref: &str,
    task_id: &str,
    user_prompt: Option<&str>,
    plan_text: Option<&str>,
) -> String {
    let prd_snapshot = read_truncated_utf8(prd_path, 48 * 1024);
    let prd_tasks = extract_tasks_snapshot(&prd_snapshot);

    let mut out = String::new();

    out.push_str("<context>\n");
    out.push_str("This is a patch-first build. Produce ONLY the Diff Envelope.\n");
    out.push_str("</context>\n\n");

    out.push_str("<prd_tasks>\n");
    out.push_str(&prd_tasks);
    out.push_str("\n</prd_tasks>\n\n");

    out.push_str("<change_contract>\n");
    out.push_str(change_contract_yaml.trim());
    out.push_str("\n</change_contract>\n\n");

    if let Some(p) = plan_text {
        out.push_str("<plan>\n");
        out.push_str(p.trim());
        out.push_str("\n</plan>\n\n");
    }

    if let Some(hint) = user_prompt {
        out.push_str("<hint>\n");
        out.push_str(hint.trim());
        out.push_str("\n</hint>\n\n");
    }

    out.push_str("<patch_protocol>\n");
    out.push_str("Emit exactly the following envelope (no markdown fences):\n");
    out.push_str("<diff_envelope>\n");
    out.push_str(&format!("base_ref: {base_ref}\n"));
    out.push_str(&format!("task_id: {task_id}\n"));
    out.push_str("rationale: \"One sentence reason\"\n");
    out.push_str("diff_format: \"unified\"\n");
    out.push_str("---BEGIN DIFF---\n");
    out.push_str("... unified diff here ...\n");
    out.push_str("---END DIFF---\n");
    out.push_str("</diff_envelope>\n");
    out.push_str("</patch_protocol>\n");

    out
}

/// Strictly enforce that the Builder produced ONLY a Diff Envelope.
/// Also returns a parsed `DiffEnvelope` for PatchGate.
pub fn enforce_diff_envelope_or_err(output: &str) -> Result<DiffEnvelope> {
    // Basic size sanity to avoid pathological payloads.
    const MAX_BYTES: usize = 2 * 1024 * 1024; // 2 MiB
    if output.len() > MAX_BYTES {
        bail!("builder output too large (>2MiB)");
    }

    // Trim common noise early and unwrap simple Markdown fences (``` or ~~~)
    let mut text_cow: Cow<'_, str> = Cow::Borrowed(output.trim());
    if let Some(unwrapped) = try_unwrap_fences(&text_cow) {
        text_cow = Cow::Owned(unwrapped);
    }
    let text: &str = text_cow.as_ref();

    // Must contain our envelope markers once.
    let start_tag = "<diff_envelope>";
    let end_tag = "</diff_envelope>";
    let start = text
        .find(start_tag)
        .ok_or_else(|| eyre!("missing <diff_envelope>"))?;
    let end = text
        .find(end_tag)
        .ok_or_else(|| eyre!("missing </diff_envelope>"))?
        + end_tag.len();

    // Before/after the envelope must be empty and fence-free.
    let before = &text[..start].trim();
    let after = &text[end..].trim();
    if before.contains("```") || after.contains("```") || before.contains("~~~") || after.contains("~~~") {
        bail!("markdown fences detected; envelope must be plain text");
    }
    if !before.is_empty() || !after.is_empty() {
        bail!("output contains extra content outside the envelope");
    }

    // Delegate structured extraction (base_ref/task_id/rationale/diff).
    let env = git_guard::parse_diff_envelope(&text[start..end])?;

    // Minimal envelope-shape allowlist for fast feedback (no deep validation):
    // - unified diffs (have "diff --git ") OR
    // - rename/copy-only envelopes ("similarity index" with rename/copy from/to)
    let d = env.diff.as_str();
    let is_unified = d.contains("diff --git ");
    let is_rename = d.contains("similarity index") && d.contains("rename from ") && d.contains("rename to ");
    let is_copy = d.contains("similarity index") && d.contains("copy from ") && d.contains("copy to ");
    if !(is_unified || is_rename || is_copy) {
        bail!("envelope diff must be unified (diff --git) or rename/copy-only with similarity index and from/to");
    }
    Ok(env)
}

/* ---------- helpers (UTF-8 safe) ---------- */

fn read_truncated_utf8(path: &Path, max_bytes: usize) -> String {
    match fs::read_to_string(path) {
        Ok(s) => truncate_middle_utf8(&s, max_bytes),
        Err(_) => "(PRD not found)".to_string(),
    }
}

fn truncate_middle_utf8(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let marker = "\n[... omitted ...]\n";
    let mb = marker.len();
    let head_budget = (max_bytes.saturating_sub(mb)) / 2;
    let head = safe_prefix_bytes(s, head_budget);
    let tail_budget = max_bytes.saturating_sub(mb + head.len());
    let tail = safe_suffix_bytes(s, tail_budget);
    format!("{head}{marker}{tail}")
}

fn safe_prefix_bytes(s: &str, budget: usize) -> &str {
    if s.len() <= budget {
        return s;
    }
    let mut end = 0usize;
    for (idx, _) in s.char_indices() {
        if idx <= budget {
            end = idx;
        } else {
            break;
        }
    }
    &s[..end]
}

fn safe_suffix_bytes(s: &str, budget: usize) -> &str {
    if s.len() <= budget {
        return s;
    }
    let mut start = s.len();
    for (idx, _) in s.char_indices().rev() {
        if s.len().saturating_sub(idx) <= budget {
            start = idx;
        } else {
            break;
        }
    }
    &s[start..]
}

/// Extract a light "Tasks" snapshot from PRD.md; fallback to bullets/checkboxes.
fn extract_tasks_snapshot(prd: &str) -> String {
    let lines = prd.lines().collect::<Vec<_>>();

    // 1) Try explicit "Tasks" section.
    let mut start: Option<usize> = None;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        if (t.starts_with('#') || t.starts_with("##")) && t.to_ascii_lowercase().contains("tasks") {
            start = Some(i + 1);
            break;
        }
    }
    if let Some(s) = start {
        let mut end = lines.len();
        for (i, line) in lines.iter().enumerate().skip(s) {
            let t = line.trim();
            if (t.starts_with('#') || t.starts_with("##")) && !t.to_ascii_lowercase().contains("tasks") {
                end = i;
                break;
            }
        }
        let body = lines[s..end].join("\n").trim().to_string();
        if !body.is_empty() {
            return body;
        }
    }

    // 2) Fallback: collect bullets / checkboxes.
    let mut acc = String::new();
    for l in lines {
        let t = l.trim_start();
        if t.starts_with("- [") || t.starts_with("- ") || t.starts_with("* ") {
            acc.push_str(l);
            acc.push('\n');
        }
    }
    let out = acc.trim_end().to_string();
    if out.is_empty() {
        "(no tasks detected)".into()
    } else {
        out
    }
}

/// Try to unwrap a single top-level Markdown code fence block (``` or ~~~).
/// Returns Some(unwrapped) if the entire text is a single fenced block; otherwise None.
fn try_unwrap_fences(s: &str) -> Option<String> {
    let t = s.trim();
    if !(t.starts_with("```") || t.starts_with("~~~")) {
        return None;
    }
    let fence = if t.starts_with("```") { "```" } else { "~~~" };
    // Find the first newline after opening fence
    let mut lines = t.lines();
    let first = lines.next()?; // opening fence line (may contain language)
    if !first.starts_with(fence) {
        return None;
    }
    // Collect remaining lines to find the last closing fence line
    let rest: Vec<&str> = lines.collect();
    if rest.is_empty() {
        return None;
    }
    // Find last index where line equals the closing fence (exact token)
    let mut last_idx: Option<usize> = None;
    for (i, line) in rest.iter().enumerate().rev() {
        if line.trim() == fence {
            last_idx = Some(i);
            break;
        }
    }
    let idx = last_idx?;
    // Ensure there is no trailing content after the closing fence
    if rest[idx + 1..].iter().any(|l| !l.trim().is_empty()) {
        return None;
    }
    // Join lines between (exclusive) into a new string
    let body = rest[..idx].join("\n");
    Some(body)
}
