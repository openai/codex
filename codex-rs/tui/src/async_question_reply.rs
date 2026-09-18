//! Recognize the desktop's existing async-question reply envelope for display and dismissal.
//! Only complete envelopes, optionally following the standard IDE context prefix, are interpreted.

use codex_app_server_protocol::UserInput;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AsyncQuestionReply {
    pub(crate) question_item_id: String,
    question: String,
    answer: String,
}

pub(crate) fn parse(text: &str) -> Option<Vec<AsyncQuestionReply>> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Replies {
        Many(Vec<AsyncQuestionReply>),
        One(AsyncQuestionReply),
    }
    let text = text.trim();
    // JSON strings escape newlines, so this cannot match a delimiter inside an answer.
    let text = if text.starts_with("# Context from my IDE setup:\n") {
        text.rsplit_once("\n## My request for Codex:\n")?.1.trim()
    } else {
        text
    };
    let json = text
        .strip_prefix("<send_user_message_question_reply>")?
        .strip_suffix("</send_user_message_question_reply>")?;
    let replies = match serde_json::from_str::<Replies>(json).ok()? {
        Replies::Many(replies) => replies,
        Replies::One(reply) => vec![reply],
    };
    (!replies.is_empty()).then_some(replies)
}

pub(crate) fn display_text(text: &str) -> Option<String> {
    Some(
        parse(text)?
            .iter()
            .map(|reply| format!("> {}\n\n{}", reply.question, reply.answer))
            .collect::<Vec<_>>()
            .join("\n\n"),
    )
}

pub(crate) fn parse_input(input: &[UserInput]) -> Option<Vec<AsyncQuestionReply>> {
    let mut content = input
        .iter()
        .filter(|item| !matches!(item, UserInput::Skill { .. } | UserInput::Mention { .. }));
    let UserInput::Text { text, .. } = content.next()? else {
        return None;
    };
    if content.next().is_some() {
        return None;
    }
    parse(text)
}

#[cfg(test)]
#[path = "async_question_reply_tests.rs"]
mod tests;
