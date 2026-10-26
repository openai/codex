//! Stateless selection of original host-verified question/answer records.
//! Omit a record atomically when it cannot fit; never truncate a restriction out of a grant.

use codex_history::RetainedContext;
use codex_history::RetainedContextEntry;
use codex_history::VerifiedAnswer;

use crate::GuardianRootMessage;
use crate::retained_instructions::source_order_labels;

/// Request-bounded answers and whether they are complete enough for cached fast approval.
pub struct RenderedVerifiedAnswers {
    pub fragments: Vec<String>,
    pub complete: bool,
}

/// Renders each response with its original source roles and the existing approximate per-answer budget.
pub fn render_verified_answers(context: &RetainedContext) -> RenderedVerifiedAnswers {
    let mut complete = context.verified_answers_complete();
    let mut fragments = Vec::new();
    for (order, entry) in source_order_labels(context) {
        let RetainedContextEntry::VerifiedAnswer(answer) = entry else {
            continue;
        };
        if let Some(text) = render_verified_answer(answer) {
            // The bounded order label is framing, not part of the existing answer budget.
            fragments.push(format!("Retained source order: {order}\n{text}"));
        } else {
            complete = false;
        }
    }
    if !complete {
        fragments.insert(
            /*index*/ 0,
            GuardianRootMessage::IncompleteVerifiedAnswers.render(),
        );
    }
    RenderedVerifiedAnswers {
        fragments,
        complete,
    }
}

/// Selects one complete response, preserving both sides of every question/answer pair.
/// Returning None means the caller must account for missing evidence, not partial permission.
pub fn render_verified_answer(answer: &VerifiedAnswer) -> Option<String> {
    let text = render_original_verified_answer(answer);
    answer_fits(text.len()).then_some(text)
}

/// Renders original question/answer bodies without making a request-selection decision.
pub fn render_original_verified_answer(answer: &VerifiedAnswer) -> String {
    answer
        .questions
        .iter()
        .map(|pair| {
            format!(
                "{}{}",
                GuardianRootMessage::Assistant(pair.question.clone()).render(),
                GuardianRootMessage::User(pair.answer.clone()).render()
            )
        })
        .collect::<String>()
}

/// One role-labeled answer or host framing, with original answer cost when not yet admitted.
#[derive(Clone, PartialEq)]
pub struct TrustedUserAnswer {
    pub(crate) text: String,
    pub(crate) answer_bytes: Option<usize>,
}

impl std::fmt::Debug for TrustedUserAnswer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TrustedUserAnswer")
            .field("answer_bytes", &self.answer_bytes)
            .finish_non_exhaustive()
    }
}

impl From<String> for TrustedUserAnswer {
    fn from(text: String) -> Self {
        Self {
            text,
            answer_bytes: None,
        }
    }
}

impl From<&str> for TrustedUserAnswer {
    fn from(text: &str) -> Self {
        text.to_owned().into()
    }
}

/// Collects originals for sync preparation without applying the per-answer request limit.
pub fn collect_verified_answer_candidates(context: &RetainedContext) -> Vec<TrustedUserAnswer> {
    let mut items = Vec::new();
    if !context.verified_answers_complete() {
        items.push(
            GuardianRootMessage::IncompleteVerifiedAnswers
                .render()
                .into(),
        );
    }
    for (order, entry) in source_order_labels(context) {
        if let RetainedContextEntry::VerifiedAnswer(answer) = entry {
            let text = render_original_verified_answer(answer);
            items.push(TrustedUserAnswer {
                answer_bytes: Some(text.len()),
                text: format!("Retained source order: {order}\n{text}"),
            });
        }
    }
    items
}

pub(crate) use crate::preparation::answer_fits;
