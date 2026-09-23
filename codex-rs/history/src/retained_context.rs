//! Bounded host-owned facts outside the model's compaction contract.
//! Checkpoints and live recording use the same admission rules; rendering is a consumer concern.
//! Adopted parent instructions precede local facts without sharing the local acceptance counter.
//! Their parent verified answers are unavailable, so adopted authorization stays incomplete.

#[path = "retained_assistant_messages.rs"]
mod assistant_messages;

use std::collections::VecDeque;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use crate::CodexHarnessMetadata;
use crate::ResponseItemEnvelope;
use crate::SenderUserMessages;

const MAX_FAMILY_RECORDS: usize = 8;
const MAX_RECORD_BYTES: usize = 16_384;
const MAX_FAMILY_BYTES: usize = 65_536;

/// Original assistant question and host-verified user reply, never an inferred permission.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VerifiedQuestionAnswer {
    pub question: String,
    pub answer: String,
}

/// One accepted request_user_input response. Identity is local to the owning thread.
/// An omitted payload records incomplete evidence, rather than keeping a partial grant.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VerifiedAnswer {
    pub turn_id: String,
    pub call_id: String,
    pub questions: Vec<VerifiedQuestionAnswer>,
}

/// Original conversational text, retained outside model summarization for review.
/// The owning family supplies the source role; assistant text never establishes authorization.
/// Non-text input is not reconstructed as text; missing evidence remains explicit.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RetainedUserMessage {
    pub turn_id: String,
    pub message_id: Option<String>,
    pub text: String,
    pub complete: bool,
}

/// Local facts use their acceptance counter; copied parent instructions use prefix order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedInputSource {
    Local(Option<u64>),
    Inherited,
}

impl RetainedInputSource {
    /// Parent counters never establish order within the owning thread.
    pub fn acceptance_order(self) -> Option<u64> {
        match self {
            Self::Local(order) => order,
            Self::Inherited => None,
        }
    }
}

impl From<Option<&CodexHarnessMetadata>> for RetainedInputSource {
    fn from(metadata: Option<&CodexHarnessMetadata>) -> Self {
        if metadata.is_some_and(|metadata| metadata.inherited_user_message) {
            Self::Inherited
        } else {
            Self::Local(metadata.and_then(|metadata| metadata.user_input_order))
        }
    }
}

impl std::fmt::Debug for RetainedUserMessage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedUserMessage")
            .field("complete", &self.complete)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
struct Ordered<T> {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    inherited: bool,
    #[serde(default)]
    order: u64,
    #[serde(flatten)]
    value: T,
}

impl<T> Ordered<T> {
    fn key(&self) -> RetainedContextOrder {
        if self.inherited {
            RetainedContextOrder::Inherited(self.order)
        } else {
            RetainedContextOrder::Local(self.order)
        }
    }
}

/// Inherited prefix order precedes the owning thread's local acceptance order.
/// The counters have separate scopes and must not be compared without their origin.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RetainedContextOrder {
    Inherited(u64),
    Local(u64),
}

/// Borrowed host evidence across retained families.
pub enum RetainedContextEntry<'a> {
    UserMessage(&'a RetainedUserMessage),
    AssistantMessage(&'a RetainedUserMessage),
    VerifiedAnswer(&'a VerifiedAnswer),
}

impl std::fmt::Debug for VerifiedAnswer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerifiedAnswer")
            .field("questions", &self.questions.len())
            .finish_non_exhaustive()
    }
}

/// Sparse, model-invisible updates. Only the host may produce these records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RetainedContextEvent {
    VerifiedAnswer {
        #[serde(flatten)]
        answer: VerifiedAnswer,
        /// Absent in legacy events, which retain their recorded ordering.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        acceptance_order: Option<u64>,
    },
}

/// Bounded snapshot of retained families, persisted with the parent compaction checkpoint.
/// Facts live until their instruction boundary is rolled back; compaction does not expire them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RetainedContext {
    verified_answers: VecDeque<Ordered<VerifiedAnswer>>,
    /// Lost evidence cannot be treated as a complete authorization history.
    #[serde(rename = "incomplete")]
    verified_answers_incomplete: bool,
    #[serde(default)]
    user_messages: VecDeque<Ordered<RetainedUserMessage>>,
    /// Old checkpoints did not preserve user restrictions for delegated review.
    #[serde(default = "missing_message_history")]
    user_messages_incomplete: bool,
    /// Assistant context has a separate budget so it cannot evict user restrictions.
    #[serde(default)]
    assistant_messages: VecDeque<Ordered<RetainedUserMessage>>,
    /// Observed assistant storage omissions; absent legacy metadata is not evidence of loss.
    #[serde(default)]
    assistant_messages_incomplete: bool,
    #[serde(default)]
    next_order: u64,
    /// Delivery snapshots share local input order, so later steers can be undone independently.
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    sender_deliveries: VecDeque<Ordered<SenderUserMessages>>,
}

fn missing_message_history() -> bool {
    true
}

fn bound_family<T: Serialize>(items: &mut VecDeque<Ordered<T>>, incomplete: &mut bool) {
    // Queued instructions can be recorded after later-accepted answers. Evict by
    // acceptance order, not by the order in which persistence happened to finish.
    items.make_contiguous().sort_by_key(Ordered::key);
    while items.len() > MAX_FAMILY_RECORDS
        || serde_json::to_vec(items).map_or(true, |bytes| bytes.len() > MAX_FAMILY_BYTES)
    {
        items.pop_front();
        *incomplete = true;
    }
}

impl RetainedUserMessage {
    fn bound(&mut self) {
        if serde_json::to_vec(self).map_or(true, |bytes| bytes.len() > MAX_RECORD_BYTES) {
            self.text.clear();
            self.complete = false;
            self.turn_id
                .truncate(self.turn_id.floor_char_boundary(1_024));
            if let Some(id) = &mut self.message_id {
                id.truncate(id.floor_char_boundary(1_024));
            }
        }
    }
}

impl RetainedContextEvent {
    /// Bounds a persisted event before it enters the rollout or the live snapshot.
    pub fn bound(&mut self) {
        match self {
            Self::VerifiedAnswer { answer, .. } => {
                if serde_json::to_vec(answer).map_or(true, |bytes| bytes.len() > MAX_RECORD_BYTES) {
                    answer.questions.clear();
                    // IDs are correlation metadata, not model-visible authorization text.
                    answer
                        .turn_id
                        .truncate(answer.turn_id.floor_char_boundary(1_024));
                    answer
                        .call_id
                        .truncate(answer.call_id.floor_char_boundary(1_024));
                }
            }
        }
    }
}

impl RetainedContext {
    /// Context for the latest delivery still present in this task's retained history.
    pub fn sender_user_messages(&self) -> Option<&SenderUserMessages> {
        self.sender_deliveries.back().map(|entry| &entry.value)
    }

    /// Retains host metadata with the delivery's acceptance order, including during replay.
    pub fn record_sender_user_messages(&mut self, metadata: &CodexHarnessMetadata) -> bool {
        let Some(messages) = metadata.sender_user_messages.as_deref() else {
            return false;
        };
        if self
            .sender_deliveries
            .iter()
            .any(|entry| entry.value.receiver_message_id == messages.receiver_message_id)
        {
            return false;
        }
        let mut messages = messages.clone();
        messages.bound();
        let order = self.record_order(metadata.user_input_order);
        self.sender_deliveries.push_back(Ordered {
            inherited: false,
            order,
            value: messages,
        });
        bound_family(&mut self.sender_deliveries, /*incomplete*/ &mut false);
        true
    }

    /// Reserves order without retaining pending input that hooks may reject or cancel.
    pub fn reserve_order(&mut self) -> u64 {
        let order = self.next_order;
        self.next_order = self.next_order.saturating_add(1);
        order
    }

    fn record_order(&mut self, acceptance_order: Option<u64>) -> u64 {
        let order = acceptance_order.unwrap_or(self.next_order);
        self.next_order = self.next_order.max(order.saturating_add(1));
        order
    }

    pub fn verified_answers(&self) -> impl DoubleEndedIterator<Item = &VerifiedAnswer> {
        self.verified_answers.iter().map(|entry| &entry.value)
    }

    pub fn verified_answers_complete(&self) -> bool {
        !self.verified_answers_incomplete
            && self
                .verified_answers
                .iter()
                .all(|answer| !answer.value.questions.is_empty())
    }

    pub fn user_messages_complete(&self) -> bool {
        !self.has_missing_user_messages()
            && self
                .user_messages
                .iter()
                .all(|message| message.value.complete)
    }

    /// Whether instruction records were lost or omitted by legacy capture.
    /// Bounded excerpts keep their source record and do not set this marker.
    pub fn has_missing_user_messages(&self) -> bool {
        self.user_messages_incomplete
    }

    /// A skipped instruction leaves a gap that later checkpoints must preserve.
    pub fn mark_user_messages_incomplete(&mut self) {
        self.user_messages_incomplete = true;
    }

    /// Whether a root checkpoint already contains its adopted instruction prefix.
    pub fn has_inherited_user_messages(&self) -> bool {
        self.user_messages.iter().any(|entry| entry.inherited)
    }

    /// Returns retained evidence with its origin and persisted order across all families.
    /// Explicit acceptance order survives delayed recording; inherited prefix order stays separate.
    pub fn ordered_entries(
        &self,
    ) -> impl DoubleEndedIterator<Item = (RetainedContextOrder, RetainedContextEntry<'_>)> {
        let mut entries = self
            .verified_answers
            .iter()
            .map(|entry| {
                (
                    entry.key(),
                    RetainedContextEntry::VerifiedAnswer(&entry.value),
                )
            })
            .chain(
                self.user_messages
                    .iter()
                    .map(|entry| (entry.key(), RetainedContextEntry::UserMessage(&entry.value))),
            )
            .chain(self.assistant_messages.iter().map(|entry| {
                (
                    entry.key(),
                    RetainedContextEntry::AssistantMessage(&entry.value),
                )
            }))
            .collect::<Vec<_>>();
        entries.sort_by_key(|(order, _)| *order);
        entries.into_iter()
    }

    /// Records a delivered user item with its acceptance order. Legacy items without
    /// this metadata use recording order; checkpoint/suffix replay uses the same path.
    /// Inherited instructions use prefix order because their original counters belong to parents.
    pub fn record_user_message(
        &mut self,
        mut message: RetainedUserMessage,
        source: RetainedInputSource,
    ) {
        message.bound();
        let inherited = source == RetainedInputSource::Inherited;
        // Worker forks omit parent answer records. Adopting their instructions
        // cannot establish whether an omitted answer restricted an inherited grant.
        self.verified_answers_incomplete |= inherited;
        if let Some(index) = self.user_messages.iter().position(|entry| {
            message.message_id.is_some() && entry.value.message_id == message.message_id
        }) {
            if self.user_messages[index].value == message
                && self.user_messages[index].inherited == inherited
            {
                return;
            }
            self.user_messages.remove(index);
        }
        // Parent and worker counters have different scopes. Adopt the copied prefix
        // in history order without advancing the worker's local acceptance counter.
        let order = if inherited {
            self.next_inherited_order()
        } else {
            self.record_order(source.acceptance_order())
        };
        self.user_messages.push_back(Ordered {
            inherited,
            order,
            value: message,
        });
        bound_family(&mut self.user_messages, &mut self.user_messages_incomplete);
    }

    /// Recovers omitted text from an exact source without changing identity, order, or
    /// completeness. Recovered excerpts must still satisfy the retained storage limits.
    pub fn recover_user_message_excerpts(
        &mut self,
        mut excerpt_for_id: impl FnMut(&str) -> Option<String>,
    ) {
        for entry in &mut self.user_messages {
            let message = &mut entry.value;
            if message.text.is_empty()
                && !message.complete
                && let Some(text) = message.message_id.as_deref().and_then(&mut excerpt_for_id)
            {
                message.text = text;
                message.bound();
            }
        }
        bound_family(&mut self.user_messages, &mut self.user_messages_incomplete);
    }

    /// Same-event delivery is idempotent; changed contents replace that source's record.
    pub fn record(&mut self, event: &RetainedContextEvent) -> bool {
        let mut event = event.clone();
        event.bound();
        match event {
            RetainedContextEvent::VerifiedAnswer {
                answer,
                acceptance_order,
            } => {
                if let Some(index) = self.verified_answers.iter().position(|existing| {
                    existing.value.turn_id == answer.turn_id
                        && existing.value.call_id == answer.call_id
                }) {
                    if self.verified_answers[index].value == answer {
                        return false;
                    }
                    self.verified_answers.remove(index);
                }
                let order = self.record_order(acceptance_order);
                self.verified_answers.push_back(Ordered {
                    inherited: false,
                    order,
                    value: answer,
                });
                bound_family(
                    &mut self.verified_answers,
                    &mut self.verified_answers_incomplete,
                );
            }
        }
        true
    }

    /// Restoring a saved thread must not bypass the live storage limits.
    /// A missing checkpoint cannot establish complete historical user instructions.
    /// Surviving local input metadata also advances the counter when its retained record is missing.
    pub fn restore(&mut self, checkpoint: Option<&Self>, surviving_items: &[ResponseItemEnvelope]) {
        *self = checkpoint.cloned().unwrap_or_else(|| Self {
            user_messages_incomplete: true,
            ..Self::default()
        });
        for entry in &mut self.verified_answers {
            let mut event = RetainedContextEvent::VerifiedAnswer {
                answer: entry.value.clone(),
                acceptance_order: Some(entry.order),
            };
            event.bound();
            let RetainedContextEvent::VerifiedAnswer { answer, .. } = event;
            entry.value = answer;
            self.next_order = self.next_order.max(entry.order.saturating_add(1));
        }
        for entry in self
            .user_messages
            .iter_mut()
            .chain(&mut self.assistant_messages)
        {
            entry.value.bound();
            // Also repair checkpoints written before adoption recorded the answer gap.
            self.verified_answers_incomplete |= entry.inherited;
            if !entry.inherited {
                self.next_order = self.next_order.max(entry.order.saturating_add(1));
            }
        }
        for entry in &mut self.sender_deliveries {
            entry.value.bound();
            self.next_order = self.next_order.max(entry.order.saturating_add(1));
        }
        bound_family(&mut self.sender_deliveries, /*incomplete*/ &mut false);
        for metadata in surviving_items
            .iter()
            .filter_map(|item| item.metadata.as_ref())
        {
            self.record_sender_user_messages(metadata);
        }
        for order in surviving_items
            .iter()
            .filter_map(|item| item.metadata.as_ref())
            .filter(|metadata| !metadata.inherited_user_message)
            .filter_map(|metadata| metadata.user_input_order)
        {
            self.next_order = self.next_order.max(order.saturating_add(1));
        }
        bound_family(
            &mut self.verified_answers,
            &mut self.verified_answers_incomplete,
        );
        bound_family(&mut self.user_messages, &mut self.user_messages_incomplete);
        bound_family(
            &mut self.assistant_messages,
            &mut self.assistant_messages_incomplete,
        );
    }

    /// Keeps legacy answers whose source calls survive when no retained instruction boundary exists.
    pub fn retain_answers(&mut self, mut keep: impl FnMut(&VerifiedAnswer) -> bool) {
        self.verified_answers.retain(|answer| keep(&answer.value));
    }

    /// Rolls back at the original user-message boundary, including later-accepted facts.
    /// The explicit order also covers checkpoints made before a queued message was delivered.
    /// Steering can share a turn ID. Legacy sources without message identity fall back to
    /// source-turn removal and cannot establish complete retained user instructions.
    pub fn rollback(
        &mut self,
        turn_ids: &[&str],
        first_removed_message_id: Option<&str>,
        source: RetainedInputSource,
    ) {
        let boundary = source.acceptance_order().map(RetainedContextOrder::Local);
        if let Some(boundary) = boundary.or_else(|| {
            first_removed_message_id.and_then(|id| {
                self.user_messages
                    .iter()
                    .find(|message| message.value.message_id.as_deref() == Some(id))
                    .map(Ordered::key)
            })
        }) {
            self.sender_deliveries
                .retain(|entry| entry.key() < boundary);
            self.verified_answers.retain(|entry| entry.key() < boundary);
            self.user_messages.retain(|entry| entry.key() < boundary);
            self.assistant_messages
                .retain(|entry| entry.key() < boundary);
            return;
        }
        self.user_messages_incomplete |= first_removed_message_id.is_some()
            || self
                .user_messages
                .iter()
                .any(|message| turn_ids.contains(&message.value.turn_id.as_str()));
        self.verified_answers.retain(|answer| {
            source != RetainedInputSource::Inherited
                && !turn_ids.contains(&answer.value.turn_id.as_str())
        });
        self.sender_deliveries.retain(|entry| {
            source != RetainedInputSource::Inherited
                && !turn_ids.contains(&entry.value.receiver_turn_id.as_str())
        });
        self.assistant_messages.retain(|message| {
            (source != RetainedInputSource::Inherited || message.inherited)
                && !turn_ids.contains(&message.value.turn_id.as_str())
        });
        self.user_messages.retain(|message| {
            (source != RetainedInputSource::Inherited || message.inherited)
                && !turn_ids.contains(&message.value.turn_id.as_str())
        });
    }
}

#[cfg(test)]
#[path = "retained_context_tests.rs"]
mod tests;
