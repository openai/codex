//! Bounds catalog presentation while retaining harness-owned denial and omission rules.
//! Callers bound paths first; catalog wording never changes the path selection budget.

use super::MAX_PERMISSION_PATH_BYTES;
use super::PermissionsInstructions;
use crate::model_messages::permissions::DENIED_READS_TEMPLATE;
use crate::model_messages::permissions::ResolvedPermissionMessages;
use codex_context_fragments::ContextualUserFragment;

pub(super) const OMITTED_PERMISSION_PATHS: &str = "Additional permission paths/globs are omitted. All restrictions still apply; do not use escalation or additional permissions to bypass omitted read denials.";
const DENIED_READS_POLICY: &str = "The active permission profile denies reading these paths/globs. Do not request escalation or additional permissions to read them; these denials are policy restrictions.";
const DENIED_READS_PLACEHOLDER: &str = "{{ denied_reads }}";
// Together the two overrides introduce at most 1,000 UTF-8 bytes of presentation.
const MAX_PRESENTATION_BYTES: usize = 500;

pub(super) fn omitted_paths_text(messages: ResolvedPermissionMessages<'_>) -> String {
    match messages.omitted_paths.catalog_override() {
        Some(text) if valid_presentation(text) && !text.contains(['{', '}']) => {
            format!("{text}\n{OMITTED_PERMISSION_PATHS}")
        }
        _ => OMITTED_PERMISSION_PATHS.to_string(),
    }
}

pub(super) fn denied_reads_text(
    paths: &[String],
    globs: &[String],
    messages: ResolvedPermissionMessages<'_>,
) -> Option<String> {
    if paths.is_empty() && globs.is_empty() {
        return None;
    }
    let entries = paths
        .iter()
        .map(|path| format!("- path `{path}`"))
        .chain(globs.iter().map(|glob| format!("- glob `{glob}`")))
        .collect::<Vec<_>>()
        .join("\n");
    let facts = format!("{DENIED_READS_POLICY}\n{entries}");
    let (before, after) = messages
        .denied_reads
        .catalog_override()
        .filter(|template| valid_presentation(template))
        .and_then(|template| template.split_once(DENIED_READS_PLACEHOLDER))
        .filter(|(before, after)| !before.contains(['{', '}']) && !after.contains(['{', '}']))
        .filter(|(before, after)| {
            before.len() + facts.len() + after.len()
                <= MAX_PERMISSION_PATH_BYTES
                    + DENIED_READS_POLICY.len()
                    + 1
                    + MAX_PRESENTATION_BYTES
        })
        .or_else(|| DENIED_READS_TEMPLATE.split_once(DENIED_READS_PLACEHOLDER))
        .unwrap_or_default();
    Some(format!("{before}{facts}{after}"))
}

fn valid_presentation(text: &str) -> bool {
    let (open, close) = PermissionsInstructions::type_markers();
    !text.trim().is_empty()
        && text.len() <= MAX_PRESENTATION_BYTES
        && !text
            .chars()
            .any(|c| (c.is_control() && c != '\n') || matches!(c, '\u{2028}' | '\u{2029}'))
        && !text.contains(open)
        && !text.contains(close)
}
