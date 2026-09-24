//! Read-only recovery of retained instructions from eligible live messages.
//! Missing retained instructions use persisted acceptance order; unknown ordering stays incomplete.
//! Recovering surviving sources cannot clear a checkpoint's record of missing instructions.
//! Legacy candidates use the same source matching without inventing an acceptance order.
//! Heartbeat versions coalesce after ordering; the latest input turn remains independent.

use std::collections::HashMap;
use std::collections::HashSet;

use crate::RetainedContext;
use crate::RetainedContextEntry;
use crate::RetainedContextOrder;
use crate::RetainedUserMessage;

/// Retained evidence supplemented by surviving user messages without changing the checkpoint.
pub struct ReconciledRetainedContext<'a> {
    retained_context: Option<&'a RetainedContext>,
    recovered: Vec<(RetainedContextOrder, RetainedUserMessage)>,
    /// Latest ordered input turn, including invocations omitted from instruction evidence.
    pub latest_user_turn_id: Option<String>,
    /// Instructions were already missing, or a recovered source could not be ordered.
    pub missing_user_messages: bool,
}

impl<'a> ReconciledRetainedContext<'a> {
    /// Reconciles eligible live user messages with retained sources in acceptance order.
    /// The caller supplies original source identities and filters out non-user context.
    /// Inherited sources must supply no local order; retained identities match before ordering.
    /// Live messages also identify the latest input turn when retained instructions coalesce.
    pub fn new(
        retained_context: Option<&'a RetainedContext>,
        live_messages: impl IntoIterator<Item = (Option<u64>, RetainedUserMessage)>,
    ) -> Self {
        let mut missing_user_messages =
            retained_context.is_none_or(RetainedContext::has_missing_user_messages);
        let retained_entries = retained_context
            .into_iter()
            .flat_map(RetainedContext::ordered_entries)
            .collect::<Vec<_>>();
        let mut recovered = Vec::new();
        let mut latest_user_input =
            retained_entries
                .iter()
                .rev()
                .find_map(|(order, entry)| match entry {
                    RetainedContextEntry::UserMessage(message) => {
                        Some((*order, message.turn_id.clone()))
                    }
                    RetainedContextEntry::AssistantMessage(_)
                    | RetainedContextEntry::VerifiedAnswer(_) => None,
                });
        let recover = retained_context.is_none_or(|context| !context.user_messages_complete());
        let mut matched_entries = HashSet::new();
        let mut used_orders = retained_entries
            .iter()
            .map(|(order, _)| *order)
            .collect::<HashSet<_>>();
        for (order, message) in live_messages {
            if let Some(order) = order.map(RetainedContextOrder::Local)
                && latest_user_input
                    .as_ref()
                    .is_none_or(|(latest, _)| order > *latest)
            {
                latest_user_input = Some((order, message.turn_id.clone()));
            }
            if !recover {
                continue;
            }
            if retained_entries
                .iter()
                .enumerate()
                .any(|(index, (_, entry))| {
                    let RetainedContextEntry::UserMessage(retained) = entry else {
                        return false;
                    };
                    same_user_message_source(retained, &message) && matched_entries.insert(index)
                })
            {
                continue;
            }
            // Queued steering can enter raw history after a later-accepted answer.
            // Compare their persisted sequence numbers, including checkpoint-only answers.
            // Parent counters cannot establish an adopted prefix position.
            let Some(order) = order
                .map(RetainedContextOrder::Local)
                .filter(|order| used_orders.insert(*order))
            else {
                missing_user_messages = true;
                continue;
            };
            recovered.push((order, message));
        }
        Self {
            retained_context,
            recovered,
            latest_user_turn_id: latest_user_input.map(|(_, turn_id)| turn_id),
            missing_user_messages,
        }
    }

    /// Returns retained and recovered evidence in persisted acceptance order.
    pub fn ordered_entries(
        &self,
    ) -> impl DoubleEndedIterator<Item = (RetainedContextOrder, RetainedContextEntry<'_>)> {
        let mut entries = self
            .retained_context
            .into_iter()
            .flat_map(RetainedContext::ordered_entries)
            .collect::<Vec<_>>();
        entries.extend(
            self.recovered
                .iter()
                .map(|(order, message)| (*order, RetainedContextEntry::UserMessage(message))),
        );
        entries.sort_by_key(|(order, _)| *order);
        let mut heartbeat_versions = HashMap::new();
        let mut previous_scope = None;
        entries.retain(|(order, entry)| {
            let inherited = matches!(order, RetainedContextOrder::Inherited(_));
            if previous_scope != Some(inherited) {
                heartbeat_versions.clear();
                previous_scope = Some(inherited);
            }
            let RetainedContextEntry::UserMessage(message) = entry else {
                return true;
            };
            if message.origin != crate::UserInputOrigin::Heartbeat {
                return true;
            }
            let Some(heartbeat) = crate::Heartbeat::parse(&message.text) else {
                heartbeat_versions.clear();
                return true;
            };
            // Checkpoint recovery cannot prove completeness, but an exact bounded envelope
            // can still identify a replay. Keep the checkpoint's missing-evidence warning.
            heartbeat_versions.insert(heartbeat.automation_id, heartbeat.instructions)
                != Some(heartbeat.instructions)
        });
        entries.into_iter()
    }

    /// Filters legacy candidates already represented by retained or recovered instructions.
    /// Unmatched sources keep their original text and identity; this does not assign an order.
    pub fn unmatched_user_messages<'b>(
        &'b self,
        messages: impl Iterator<Item = RetainedUserMessage> + 'b,
    ) -> impl Iterator<Item = RetainedUserMessage> + 'b {
        let sources = self
            .ordered_entries()
            .filter_map(|(_, entry)| match entry {
                RetainedContextEntry::UserMessage(message) => Some(message),
                RetainedContextEntry::AssistantMessage(_)
                | RetainedContextEntry::VerifiedAnswer(_) => None,
            })
            .collect::<Vec<_>>();
        messages.filter(move |message| {
            !sources
                .iter()
                .any(|source| same_user_message_source(source, message))
        })
    }
}

fn same_user_message_source(
    retained: &RetainedUserMessage,
    candidate: &RetainedUserMessage,
) -> bool {
    if let Some(id) = &retained.message_id {
        candidate.message_id.as_ref() == Some(id)
    } else {
        candidate.turn_id == retained.turn_id && candidate.text == retained.text
    }
}

#[cfg(test)]
#[path = "reconciled_retained_context_tests.rs"]
mod tests;
