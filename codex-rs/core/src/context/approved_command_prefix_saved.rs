use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

pub(crate) const SAVED_PREFIX_KIND: &str = "permissions.approved_command_prefix_saved";

pub(crate) const APPROVED_COMMAND_PREFIX_SAVED_MESSAGE_PREFIX: &str =
    "Approved command prefix saved:";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ApprovedCommandPrefixSaved {
    text: String,
}

impl ApprovedCommandPrefixSaved {
    /// The catalog heading is capped at 500 UTF-8 bytes. Keep the bundled marker
    /// first so older clients can trim this notification during rollback. The
    /// prefix list retains its own size limit and truncation notice.
    pub(crate) fn new(prefixes: String, heading: Option<&str>) -> Self {
        let marker = APPROVED_COMMAND_PREFIX_SAVED_MESSAGE_PREFIX;
        let text = match heading {
            Some(heading) if heading != marker => format!("{marker}\n{heading}\n{prefixes}"),
            Some(_) | None => format!("{marker}\n{prefixes}"),
        };
        Self { text }
    }
}

impl ContextualUserFragment for ApprovedCommandPrefixSaved {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind(SAVED_PREFIX_KIND.to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> ContentItem {
        input_text(self.text.clone())
    }
}
