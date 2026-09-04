//! Bounded question framing accompanying an explicitly submitted user answer.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

/// Identifies an answered question without repeating an unbounded model-authored prompt.
pub struct AnsweredQuestion {
    question: String,
}

impl AnsweredQuestion {
    pub fn new(question: &str) -> Self {
        let end = question.floor_char_boundary(question.len().min(512));
        Self {
            question: question[..end].to_string(),
        }
    }
}

impl ContextualUserFragment for AnsweredQuestion {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("user.answered_question".into())
    }
    fn role(&self) -> &'static str {
        "user"
    }
    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }
    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }
    fn body(&self) -> String {
        format!("> {}\n\n", self.question.replace(['\n', '\r'], " "))
    }
}

#[cfg(test)]
#[path = "answered_question_tests.rs"]
mod tests;
