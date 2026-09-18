//! Async answers use the desktop's existing reply envelope and stable question identity.
//! Model-authored framing is bounded; oversized identities use the previous plain-text format.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

/// Identifies an answered question without repeating an unbounded model-authored prompt.
pub struct AnsweredQuestion<'a> {
    question_id: Option<&'a str>,
    question: String,
    answer: &'a str,
}

impl<'a> AnsweredQuestion<'a> {
    pub fn new(question_id: &'a str, question: &str, answer: &'a str) -> Self {
        let end = question.floor_char_boundary(question.len().min(512));
        Self {
            question_id: (question_id.len() <= 512).then_some(question_id),
            question: question[..end].replace(['\n', '\r'], " "),
            answer,
        }
    }
}

impl ContextualUserFragment for AnsweredQuestion<'_> {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("user.answered_question".into())
    }
    fn role(&self) -> &'static str {
        "user"
    }
    fn markers(&self) -> (&'static str, &'static str) {
        if self.question_id.is_some() {
            Self::type_markers()
        } else {
            ("", "")
        }
    }
    fn type_markers() -> (&'static str, &'static str) {
        (
            "<send_user_message_question_reply>",
            "</send_user_message_question_reply>",
        )
    }
    fn body(&self) -> String {
        let Some(question_id) = self.question_id else {
            return format!("> {}\n\n{}", self.question, self.answer);
        };
        let replies = serde_json::json!([{
            "answer": self.answer,
            "question": self.question,
            "questionItemId": question_id,
        }]);
        format!("\n{replies}\n")
    }
}

#[cfg(test)]
#[path = "answered_question_tests.rs"]
mod tests;
