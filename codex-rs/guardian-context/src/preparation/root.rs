//! Fixed root request selection, including legacy source matching and omission notices.
//! Collection and persistent storage keep their existing bounds and source ownership.
//! Assistant candidates borrow one immutable host snapshot until synchronous preparation ends.

use crate::GuardianRootMessage;
use codex_history::RetainedContextOrder;
use codex_history::RetainedUserMessage;
use codex_protocol::models::ContentItem;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::VecDeque;

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

/// Unselected evidence from the same snapshot as the root authorization metadata.
#[derive(Clone, PartialEq)]
pub struct RootConversationCandidates<'a> {
    pub user_messages: Vec<RootMessageCandidate>,
    pub assistant_messages: Vec<RootAssistantCandidate<'a>>,
    pub retained_assistants: Vec<RootRetainedAssistant<'a>>,
    /// Newest first, preserving host-confirmed delivery lookup order.
    pub delivered_messages: Vec<RootDeliveredMessage<'a>>,
    /// Retained identities used for legacy matching after request-size preparation.
    pub retained_user_sources: Vec<RetainedUserMessage>,
    pub handoff_boundaries: Vec<RetainedContextOrder>,
    pub latest_user_turn_id: Option<String>,
    pub missing_root_instructions: bool,
    pub missing_assistant_context: bool,
    pub missing_verified_answers: bool,
}

impl std::fmt::Debug for RootConversationCandidates<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RootConversationCandidates(..)")
    }
}

/// An immutable host snapshot that lends sources only during synchronous preparation.
/// Implementations must preserve capture-time provenance and avoid reading live session state.
pub trait RootConversationSource: Send + Sync {
    fn prepare(&self) -> PreparedRootContext;
}

/// Shared ownership keeps captured source payloads alive without copying them.
/// Equality identifies the same capture, rather than comparing potentially sensitive payloads.
#[derive(Clone)]
pub struct RootConversationInput(std::sync::Arc<dyn RootConversationSource>);

impl RootConversationInput {
    pub fn new(source: impl RootConversationSource + 'static) -> Self {
        Self(std::sync::Arc::new(source))
    }

    pub fn prepare(&self) -> PreparedRootContext {
        self.0.prepare()
    }
}

impl std::fmt::Debug for RootConversationInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RootConversationInput(..)")
    }
}

impl PartialEq for RootConversationInput {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

const ROOT_CONTEXT_WINDOW: usize = 3;

/// Preserves the current handoff windows, latest context and all user restrictions.
fn select_root_handoff_indices(
    messages: &[RootMessageCandidate],
    boundaries: &[RetainedContextOrder],
) -> Option<BTreeSet<usize>> {
    let mut selected = BTreeSet::new();
    for boundary in boundaries {
        selected.extend(
            messages
                .iter()
                .enumerate()
                .rev()
                .filter_map(|(index, message)| {
                    if matches!(message.message, GuardianRootMessage::UserInput(_)) {
                        return None;
                    }
                    message
                        .order
                        .filter(|order| order <= boundary)
                        .map(|_| index)
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
                .filter_map(|(index, message)| {
                    (message.order.is_some()
                        && !matches!(message.message, GuardianRootMessage::UserInput(_)))
                    .then_some(index)
                })
                .take(ROOT_CONTEXT_WINDOW),
        );
        // Later user inputs can restrict an earlier handoff even when they are not recent.
        // Leave all user inputs, verified answers, and unordered legacy evidence to the shared cap.
        selected.extend(messages.iter().enumerate().filter_map(|(index, message)| {
            (message.order.is_none()
                || matches!(
                    message.message,
                    GuardianRootMessage::User(_) | GuardianRootMessage::UserInput(_)
                ))
            .then_some(index)
        }));
    }
    (!selected.is_empty()).then_some(selected)
}

const MAX_ROOT_MESSAGE_TOKENS: usize = 900;

const MAX_ROOT_MESSAGES: usize = 16;

/// Root assistant evidence after delivery lookup, original admission and live fallback.
struct PreparedRootAssistants {
    messages: Vec<crate::RootMessageCandidate>,
    missing_context: bool,
}

struct LiveRootMessage {
    source_id: Option<String>,
    candidate: RootMessageCandidate,
}

/// Prepares assistant evidence without changing which sources were captured.
fn prepare_root_assistant_messages(
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

/// Bounded root view plus the existing cache and skill-scope selection metadata.
pub struct PreparedRootContext {
    pub messages: Vec<GuardianRootMessage>,
    pub missing_root_instructions: bool,
    pub handoff_selection: Option<BTreeSet<usize>>,
    pub latest_user_turn_id: Option<String>,
}

/// Applies the fixed root request policy without discarding the captured sources.
pub fn prepare_root_context(
    input: &RootConversationCandidates<'_>,
    legacy_candidates: impl IntoIterator<Item = RetainedUserMessage>,
) -> PreparedRootContext {
    let mut missing_root_instructions = input.missing_root_instructions;
    let mut missing_verified_answers = input.missing_verified_answers;
    // Verified answers stay whole; ordinary root user text keeps its existing cap.
    let mut messages = input
        .user_messages
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
        .collect::<Vec<_>>();
    missing_verified_answers |= messages.len() != input.user_messages.len();
    let legacy_capacity = MAX_ROOT_MESSAGES.saturating_sub(messages.len());
    let mut legacy_messages = VecDeque::new();
    let mut latest_legacy_turn_id = None;
    if missing_root_instructions && legacy_capacity > 0 {
        for candidate in legacy_candidates {
            let text = crate::truncate_text(&candidate.text, MAX_ROOT_MESSAGE_TOKENS);
            // Legacy records without IDs historically match after the request-size cap.
            if input.retained_user_sources.iter().any(|source| {
                if let Some(id) = &source.message_id {
                    candidate.message_id.as_ref() == Some(id)
                } else {
                    candidate.turn_id == source.turn_id && text == source.text
                }
            }) {
                continue;
            }
            latest_legacy_turn_id =
                (!candidate.turn_id.is_empty()).then(|| candidate.turn_id.clone());
            legacy_messages.push_back(GuardianRootMessage::User(text));
            if legacy_messages.len() > legacy_capacity {
                legacy_messages.pop_front();
            }
        }
    }
    let assistants = prepare_root_assistant_messages(
        &input.assistant_messages,
        &input.retained_assistants,
        &input.delivered_messages,
        input.missing_assistant_context,
    );
    let mut missing_assistant_context = assistants.missing_context;
    messages.extend(assistants.messages);
    messages.sort_by_key(|message| message.order);
    let handoff_selection = select_root_handoff_indices(&messages, &input.handoff_boundaries);
    if let Some(selected) = &handoff_selection {
        messages = messages
            .into_iter()
            .enumerate()
            .filter_map(|(index, message)| {
                if selected.contains(&index) {
                    Some(message)
                } else {
                    missing_assistant_context |= matches!(
                        message.message,
                        GuardianRootMessage::Assistant(_)
                            | GuardianRootMessage::UnorderedAssistant(_)
                    );
                    None
                }
            })
            .collect();
    }
    let removed = messages.len().saturating_sub(MAX_ROOT_MESSAGES);
    for message in &messages[..removed] {
        missing_root_instructions |= matches!(
            message.message,
            GuardianRootMessage::User(_) | GuardianRootMessage::UserInput(_)
        );
        missing_assistant_context |= matches!(
            message.message,
            GuardianRootMessage::Assistant(_) | GuardianRootMessage::UnorderedAssistant(_)
        );
    }
    messages.drain(..removed);
    messages.sort_by_key(|message| (message.order.is_none(), message.order));
    legacy_messages.drain(
        ..legacy_messages
            .len()
            .saturating_sub(MAX_ROOT_MESSAGES - messages.len()),
    );
    let mut messages = messages
        .into_iter()
        .map(|message| message.message)
        .collect::<Vec<_>>();
    if missing_assistant_context {
        messages.insert(
            /*index*/ 0,
            GuardianRootMessage::IncompleteAssistantContext,
        );
    }
    if missing_verified_answers {
        messages.insert(
            /*index*/ 0,
            GuardianRootMessage::IncompleteVerifiedAnswers,
        );
    }
    if missing_root_instructions {
        messages.insert(
            /*index*/ 0,
            GuardianRootMessage::IncompleteRootInstructions,
        );
    }
    messages.insert(/*index*/ 0, GuardianRootMessage::RetainedContextScope);
    if !legacy_messages.is_empty() {
        legacy_messages.push_front(GuardianRootMessage::LegacyContextScope);
        legacy_messages.extend(messages);
        messages = legacy_messages.into_iter().collect();
    }
    PreparedRootContext {
        messages,
        missing_root_instructions,
        handoff_selection,
        latest_user_turn_id: input.latest_user_turn_id.clone().or(latest_legacy_turn_id),
    }
}
