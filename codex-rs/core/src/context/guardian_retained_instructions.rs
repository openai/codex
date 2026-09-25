//! Classifies bounded, already-framed Guardian evidence restored after compaction.
//! Preserve its text boundaries so subsequent reviews can recognize admitted originals.

use super::ContextualUserFragment;
use codex_protocol::error::CodexErr;
use codex_protocol::models::ContentItemKind;
use codex_protocol::protocol::TruncationPolicy;

pub(crate) struct GuardianRetainedInstructions(String);

impl GuardianRetainedInstructions {
    pub(crate) const KIND: &str = "guardian.retained_instructions";
}

impl TryFrom<String> for GuardianRetainedInstructions {
    type Error = CodexErr;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        if text.len() > TruncationPolicy::Tokens(900).byte_budget() {
            return Err(CodexErr::InvalidRequest(
                "restored Guardian evidence exceeds its per-record limit".to_owned(),
            ));
        }
        Ok(Self(text))
    }
}

impl ContextualUserFragment for GuardianRetainedInstructions {
    fn role(&self) -> &'static str {
        "user"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind(Self::KIND.to_owned())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        self.0.clone()
    }
}
