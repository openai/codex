//! Fixed root request selection, including legacy source matching and omission notices.
//! Collection and persistent storage keep their existing bounds and source ownership.
//! Assistant candidates borrow one immutable host snapshot until synchronous preparation ends.

use crate::GuardianRootMessage;
use codex_history::RetainedContextOrder;
use codex_history::RetainedUserMessage;
use codex_protocol::models::ContentItem;
use std::collections::BTreeSet;
use std::collections::HashMap;

#[derive(Clone, PartialEq)]
pub struct RootMessageCandidate {
    pub order: Option<RetainedContextOrder>,
    pub message: GuardianRootMessage,
}

#[derive(Clone, PartialEq)]
pub enum RootAssistantContent<'a> {
    Text(&'a [ContentItem]),
    DeliveredCall { call_id: String },
}

#[derive(Clone, PartialEq)]
pub struct RootAssistantCandidate<'a> {
    pub message_id: Option<String>,
    pub order: Option<RetainedContextOrder>,
    pub content: RootAssistantContent<'a>,
}

#[derive(Clone, PartialEq)]
pub struct RootRetainedAssistant<'a> {
    pub order: RetainedContextOrder,
    pub message: &'a RetainedUserMessage,
    /// Host checks exclude commentary from standalone context, before request selection.
    pub standalone_eligible: bool,
}

#[derive(Clone, PartialEq)]
pub struct RootDeliveredMessage<'a> {
    pub call_id: &'a str,
    pub text: &'a str,
}

const ROOT_CONTEXT_WINDOW: usize = 3;

/// Preserves the current handoff windows, latest context and all user restrictions.
pub fn select_root_handoff_indices(
    messages: &[(Option<RetainedContextOrder>, GuardianRootMessage)],
    boundaries: &[RetainedContextOrder],
) -> Option<BTreeSet<usize>> {
    let mut selected = BTreeSet::new();
    for boundary in boundaries {
        selected.extend(
            messages
                .iter()
                .enumerate()
                .rev()
                .filter_map(|(index, (order, message))| {
                    if matches!(message, GuardianRootMessage::UserInput(_)) {
                        return None;
                    }
                    order.filter(|order| order <= boundary).map(|_| index)
                })
                .take(ROOT_CONTEXT_WINDOW),
        );
    }
    if !selected.is_empty() {
        // Include the latest ordinary messages even without a new handoff to this branch.
        selected.extend(
            messages
                .iter()
                .enumerate()
                .rev()
                .filter_map(|(index, (order, message))| {
                    (order.is_some() && !matches!(message, GuardianRootMessage::UserInput(_)))
                        .then_some(index)
                })
                .take(ROOT_CONTEXT_WINDOW),
        );
        // Later user inputs can restrict an earlier handoff even when they are not recent.
        // Leave all user inputs, verified answers, and unordered legacy evidence to the shared cap.
        selected.extend(
            messages
                .iter()
                .enumerate()
                .filter_map(|(index, (order, message))| {
                    (order.is_none()
                        || matches!(
                            message,
                            GuardianRootMessage::User(_) | GuardianRootMessage::UserInput(_)
                        ))
                    .then_some(index)
                }),
        );
    }
    (!selected.is_empty()).then_some(selected)
}

const MAX_ROOT_MESSAGE_TOKENS: usize = 900;

/// Bounds root user text and admits verified answers only as whole records.
pub fn prepare_root_user_messages(
    input: &[crate::RootMessageCandidate],
) -> Vec<crate::RootMessageCandidate> {
    input
        .iter()
        .filter_map(|candidate| {
            if let GuardianRootMessage::UserInput(text) = &candidate.message
                && !crate::verified_answers::answer_fits(text.len())
            {
                return None;
            }
            let message = if let GuardianRootMessage::User(text) = &candidate.message {
                GuardianRootMessage::User(crate::truncate_text(text, MAX_ROOT_MESSAGE_TOKENS))
            } else {
                candidate.message.clone()
            };
            Some(crate::RootMessageCandidate {
                order: candidate.order,
                message,
            })
        })
        .collect()
}

const MAX_ROOT_MESSAGES: usize = 16;

/// Root assistant evidence after delivery lookup, original admission and live fallback.
pub struct PreparedRootAssistants {
    pub messages: Vec<crate::RootMessageCandidate>,
    pub missing_context: bool,
}

struct LiveRootMessage {
    source_id: Option<String>,
    candidate: RootMessageCandidate,
}

/// Prepares assistant evidence without changing which sources were captured.
pub fn prepare_root_assistant_messages(
    assistant_messages: &[crate::RootAssistantCandidate<'_>],
    retained_assistants: &[crate::RootRetainedAssistant<'_>],
    delivered_messages: &[crate::RootDeliveredMessage<'_>],
    mut missing_assistant_context: bool,
) -> PreparedRootAssistants {
    let delivered = delivered_messages
        .iter()
        .take(MAX_ROOT_MESSAGES)
        .map(|message| (message.call_id, message.text))
        .collect::<HashMap<_, _>>();
    let live = assistant_messages
        .iter()
        .filter_map(|source| {
            let text = match &source.content {
                RootAssistantContent::Text(content) => content
                    .iter()
                    .filter_map(|item| match item {
                        ContentItem::InputText { text } | ContentItem::OutputText { text, .. } => {
                            Some(text.as_str())
                        }
                        _ => None,
                    })
                    .collect::<String>(),
                RootAssistantContent::DeliveredCall { call_id } => {
                    delivered.get(call_id.as_str())?.to_string()
                }
            };
            let retained = retained_assistants.iter().find(|retained| {
                source
                    .message_id
                    .as_ref()
                    .is_some_and(|id| retained.message.message_id.as_ref() == Some(id))
            });
            let order = retained.map(|retained| retained.order).or(source.order);
            let text = retained
                .and_then(|retained| root_retained_assistant(retained.message))
                .unwrap_or_else(|| crate::truncate_text(&text, MAX_ROOT_MESSAGE_TOKENS));
            let message = if order.is_some() {
                GuardianRootMessage::Assistant(text)
            } else {
                GuardianRootMessage::UnorderedAssistant(text)
            };
            Some(LiveRootMessage {
                source_id: source.message_id.clone(),
                candidate: RootMessageCandidate { order, message },
            })
        })
        .collect::<Vec<_>>();
    let mut messages = retained_assistants
        .iter()
        .filter_map(|source| {
            if !source.standalone_eligible
                || source.message.message_id.as_ref().is_some_and(|id| {
                    live.iter()
                        .any(|message| message.source_id.as_ref() == Some(id))
                })
            {
                return None;
            }
            let text = root_retained_assistant(source.message);
            missing_assistant_context |= text.is_none();
            text.map(|text| RootMessageCandidate {
                order: Some(source.order),
                message: GuardianRootMessage::Assistant(text),
            })
        })
        .collect::<Vec<_>>();
    messages.extend(live.into_iter().map(|message| message.candidate));
    PreparedRootAssistants {
        messages,
        missing_context: missing_assistant_context,
    }
}

fn root_retained_assistant(message: &codex_history::RetainedUserMessage) -> Option<String> {
    crate::retained_assistant_message(message).map(|_| message.text.clone())
}
