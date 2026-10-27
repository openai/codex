//! Selects bounded catalog headings while preserving all permission and review guidance.

use super::AUTO_REVIEW_SUFFIX;
use super::ApprovalPromptContext;
use super::PermissionsInstructions;
use super::REQUEST_PERMISSIONS_TOOL;
use codex_context_fragments::ContextualUserFragment;
use codex_protocol::openai_models::ApprovalMessages;

pub(super) struct ApprovalInstructions<'a>(Option<&'a ApprovalMessages>);

impl<'a> ApprovalInstructions<'a> {
    pub(super) fn new(context: ApprovalPromptContext<'a>) -> Self {
        Self(context.messages.catalog)
    }

    pub(super) fn tool(&self) -> String {
        with_heading(
            self.0.and_then(|m| m.request_permissions.as_deref()),
            REQUEST_PERMISSIONS_TOOL,
        )
    }

    pub(super) fn auto_review(&self) -> String {
        match self
            .0
            .and_then(|m| m.auto_review_suffix.as_deref())
            .filter(|s| valid_heading(s))
        {
            Some(heading) => format!("{heading}\n\n{AUTO_REVIEW_SUFFIX}"),
            None => AUTO_REVIEW_SUFFIX.to_owned(),
        }
    }
}

fn with_heading(heading: Option<&str>, bundled: &str) -> String {
    match (
        heading.filter(|s| valid_heading(s)),
        bundled.find(['\r', '\n']),
    ) {
        (Some(heading), Some(end)) => format!("{heading}{}", &bundled[end..]),
        _ => bundled.to_owned(),
    }
}

// One single-line heading can add at most 500 UTF-8 bytes plus its paragraph separator.
fn valid_heading(text: &str) -> bool {
    let (open, close) = PermissionsInstructions::type_markers();
    text.len() <= 500
        && !text.trim().is_empty()
        && !text.chars().any(char::is_control)
        && !text.contains(['{', '}', '\u{2028}', '\u{2029}'])
        && !text.contains(open)
        && !text.contains(close)
}

#[cfg(test)]
#[path = "approval_instructions_tests.rs"]
mod tests;
