//! Renders bounded catalog wording around an already bounded list of writable roots.
//! Validate template text before inserting paths once; unusable overrides retain legacy bytes.

use super::MAX_PERMISSION_PATH_BYTES;
use super::PermissionsInstructions;
use crate::model_messages::permissions::ResolvedPermissionMessages;
use crate::model_messages::permissions::WRITABLE_ROOT_TEMPLATE;
use crate::model_messages::permissions::WRITABLE_ROOTS_TEMPLATE;
use codex_context_fragments::ContextualUserFragment;

const ROOTS_PLACEHOLDER: &str = "{{ roots }}";
// Replacing the 11-byte marker leaves at most 989 bytes of catalog wording,
// bounding it below 1K tokens even with one byte-fallback token per byte.
const MAX_ROOTS_TEMPLATE_BYTES: usize = 1_000;
const MAX_ROOTS_SENTENCE_BYTES: usize = MAX_PERMISSION_PATH_BYTES + MAX_ROOTS_TEMPLATE_BYTES;

// The caller bounds paths at whole-entry boundaries before reaching this renderer.
pub(super) fn writable_roots_text(
    roots: &[String],
    messages: ResolvedPermissionMessages<'_>,
) -> Option<String> {
    let (selected, fallback) = match roots.len() {
        0 => return None,
        1 => (messages.writable_root, WRITABLE_ROOT_TEMPLATE),
        _ => (messages.writable_roots, WRITABLE_ROOTS_TEMPLATE),
    };
    let roots_list = roots
        .iter()
        .map(|root| format!("`{root}`"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(
        selected
            .catalog_override()
            .and_then(|template| render_roots_override(template, &roots_list))
            .unwrap_or_else(|| fallback.replace(ROOTS_PLACEHOLDER, &roots_list)),
    )
}

fn render_roots_override(template: &str, roots: &str) -> Option<String> {
    if template.len() > MAX_ROOTS_TEMPLATE_BYTES
        || template
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
    {
        return None;
    }
    let (before, after) = template.split_once(ROOTS_PLACEHOLDER)?;
    // Reject duplicate, unknown and malformed placeholders, without parsing runtime paths.
    if [before, after]
        .iter()
        .any(|part| part.chars().any(|c| matches!(c, '{' | '}')))
    {
        return None;
    }
    let (open, close) = PermissionsInstructions::type_markers();
    if template.contains(open) || template.contains(close) {
        return None;
    }
    let rendered_len = before
        .len()
        .checked_add(roots.len())?
        .checked_add(after.len())?;
    if rendered_len > MAX_ROOTS_SENTENCE_BYTES {
        return None;
    }
    Some(format!("{before}{roots}{after}"))
}

#[cfg(test)]
#[path = "writable_roots_tests.rs"]
mod tests;
