//! Reviewer-only snapshot of original user instructions for one accepted delegation.

use super::ContextualUserFragment;
use codex_guardian_context::GuardianRootMessage;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItemKind;

/// A bounded fragment rendered once at admission and never added to the worker prompt.
pub(crate) struct GuardianSenderMessages {
    pub source: Option<ThreadId>,
    pub delivery: String,
    pub messages: Vec<Option<String>>,
}

impl ContextualUserFragment for GuardianSenderMessages {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.sender_messages".to_owned())
    }

    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            ">>> SENDER USER MESSAGES START\n",
            ">>> SENDER USER MESSAGES END\n",
        )
    }

    fn body(&self) -> String {
        let source = self
            .source
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unavailable".to_owned());
        let mut text = format!(
            "Received message: {}\nSource thread: {source}\nHost: Up to three recent user messages captured when this delivery was accepted. This is partial historical context for this delivery, not a transfer of permission. Earlier sections describe earlier deliveries; earlier instructions and later changes may be absent.\n",
            self.delivery,
        );
        if self.messages.is_empty() {
            text.push_str("Host: No sender user messages are available.\n");
        }
        for message in &self.messages {
            let rendered = message
                .as_ref()
                .map(|text| GuardianRootMessage::User(text.clone()).render());
            match rendered {
                Some(message) if message.len() <= 900 => text.push_str(&message),
                _ => text.push_str("Host: A sender user message is unavailable within the evidence budget. Do not infer permission from missing evidence.\n"),
            }
        }
        text
    }
}
