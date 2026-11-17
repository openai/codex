//! Fits newly composed evidence into the remaining complete-request allowance.
//! Required action evidence is never truncated. Optional evidence leaves first;
//! hosts may shorten historical instructions after compaction cannot make room.
//! Every reduction reserves an omission notice and preserves source order.
//! JSON recovery shortens only the body; line mode shortens the whole rendered entry.
//! Native deduplication prefers required and protected copies before optional eviction.

use std::collections::HashMap;
use std::collections::HashSet;

use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseItem;

use crate::ComposedContext;
use crate::ElasticSectionLimits;
use crate::RequestBudget;
use crate::SectionError;
use crate::TruncationObservation;
use crate::budget::content_framing_tokens;
use crate::budget::section_content_tokens;
use crate::budget::section_tokens;
use crate::composition::PendingEnclosingCall;
use crate::composition::SectionContent;
use crate::composition::SectionDelivery;
use crate::composition::SectionOutput;
use crate::truncation::truncate_content_to_fit;

const MIN_HISTORICAL_TEXT_TOKENS: usize = 32;

/// Eviction priority within a profile; older items at the same priority go first.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum BudgetPriority {
    ToolDescription,
    Commentary,
    Tool,
    SelectedExecResult,
    Image,
}

/// Content and its selection policy move together through rendering and admission.
/// Required support has priority within Elastic section allocations. Protected host framing
/// and exact actions stay complete; Fixed retention remains unchanged.
#[derive(Clone, PartialEq)]
pub struct Budgeted<T> {
    pub content: T,
    /// Host framing and exact actions use global space, outside section quotas.
    pub(crate) protected: bool,
    /// Host-verified question/answer records must be kept or omitted whole.
    pub(crate) verified_answer: bool,
    pub(crate) source: Option<codex_history::RetainedSource>,
    pub(crate) retention: Retention,
}

impl<T> Budgeted<T> {
    pub fn required(content: T) -> Self {
        Self {
            content,
            protected: false,
            verified_answer: false,
            source: None,
            retention: Retention::Required,
        }
    }

    pub(crate) fn protected(content: T) -> Self {
        Self {
            protected: true,
            ..Self::required(content)
        }
    }

    pub(crate) fn verified_answer(content: T) -> Self {
        Self {
            verified_answer: true,
            ..Self::required(content)
        }
    }

    pub(crate) fn map<U>(self, transform: impl FnOnce(T) -> U) -> Budgeted<U> {
        Budgeted {
            content: transform(self.content),
            protected: self.protected,
            verified_answer: self.verified_answer,
            source: self.source,
            retention: self.retention,
        }
    }

    pub(crate) fn historical(content: T) -> Self {
        Self {
            content,
            protected: false,
            verified_answer: false,
            source: None,
            retention: Retention::Historical,
        }
    }

    pub(crate) fn optional(content: T, priority: BudgetPriority) -> Self {
        Self {
            content,
            protected: false,
            verified_answer: false,
            source: None,
            retention: Retention::Optional(priority),
        }
    }
}

impl<T> std::fmt::Debug for Budgeted<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Budgeted")
            .field("retention", &self.retention)
            .field("protected", &self.protected)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Retention {
    Required,
    Historical,
    Optional(BudgetPriority),
}

/// Whether the host has reached the final attempt to fit historical evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryTruncation {
    /// Preserve original instructions while the host can still compact history.
    Preserve,
    /// Shorten older historical entries only after optional evidence is exhausted.
    Allow,
}

impl ComposedContext {
    fn planned_action_content(&mut self) -> Option<&mut Vec<Budgeted<SectionContent>>> {
        self.sections.iter_mut().find_map(|section| {
            if section.id == "planned_action"
                && let SectionDelivery::UserContent(items) = &mut section.delivery
            {
                Some(items)
            } else {
                None
            }
        })
    }

    /// The optional parent can use spare space without displacing ordinary
    /// transcript evidence or changing whether the approval fits.
    pub(crate) fn admit_enclosing_call(
        &mut self,
        remaining: usize,
        limits: Option<ElasticSectionLimits>,
    ) {
        let Some(PendingEnclosingCall {
            call:
                crate::EnclosingCall {
                    arguments,
                    tool,
                    max_bytes,
                },
            fixed_image_budget,
        }) = self.enclosing_call.take()
        else {
            return;
        };
        let call = serde_json::json!({ "arguments": arguments, "tool": tool });
        let text = format!("Enclosing call for this review: {call}\n");
        if text.len() > max_bytes {
            return;
        }
        let item = Budgeted::optional(SectionContent::from(text), BudgetPriority::ToolDescription);
        let available = remaining.saturating_sub(self.estimated_tokens());
        let Some(items) = self.planned_action_content() else {
            return;
        };
        let tokens = section_content_tokens(&item.content);
        let framing = |count| content_framing_tokens(count + 1) - content_framing_tokens(count);
        if tokens.saturating_add(framing(items.len())) <= available
            && limits.is_none_or(|limits| {
                let supporting = items.iter().filter(|item| !item.protected);
                let supporting_tokens = supporting
                    .clone()
                    .map(|item| section_content_tokens(&item.content))
                    .sum::<usize>();
                let supporting_count = supporting.count();
                // Elastic allocation charges supporting rows from a shared wrapper.
                let combined = supporting_tokens
                    .saturating_add(tokens)
                    .saturating_add(content_framing_tokens(supporting_count + 2))
                    .saturating_sub(content_framing_tokens(/*item_count*/ 1));
                tokens.saturating_add(content_framing_tokens(/*item_count*/ 1))
                    <= limits.max_section_entry_tokens()
                    && limits
                        .max_total_section_tokens()
                        .is_none_or(|maximum| combined <= maximum)
            }) && let Some(index) = items.iter().position(|item| {
            item.protected
                && matches!(&item.content, SectionContent::Other(ContentItem::InputText { text })
                        if text == crate::action::APPROVAL_REQUEST_START)
        }) {
            items.insert(index + 1, item);
            // Fixed image admission uses the host estimator, independently of the
            // aggregate budget. Check the delivered prompt after ordinary selection.
            if let Some(budget) = fixed_image_budget
                && self.section_costs().any(|(_, cost)| cost.image_count > 0)
                && self
                    .clone()
                    .into_messages()
                    .iter()
                    .map(budget.estimate_tokens)
                    .fold(0i64, i64::saturating_add)
                    > budget.max_prompt_tokens
                && let Some(items) = self.planned_action_content()
            {
                items.remove(index + 1);
            }
        }
    }

    /// Omits native messages and tool calls already present in the effective reviewer history
    /// or another pending copy, preferring required and protected copies.
    /// Hosts repeat this check after compaction; missing evidence may be delivered again.
    /// Identity excludes host annotations.
    /// Conflicting payloads under one ID reject the input without changing it.
    pub fn retain_new_native_messages(
        &mut self,
        reviewer_history: &[ResponseItemEnvelope],
    ) -> Result<(), SectionError> {
        if !self.sections.iter().any(|section| {
            matches!(
                &section.delivery,
                SectionDelivery::Message(message)
                    if matches!(message.content.item,
                        ResponseItem::AgentMessage { .. } | ResponseItem::FunctionCall { .. })
            )
        }) {
            return Ok(());
        }
        let mut seen_messages = HashMap::new();
        let mut seen_calls = HashMap::new();
        let mut duplicates = HashSet::new();
        let mut pending = self
            .sections
            .iter()
            .enumerate()
            .filter_map(|(index, section)| match &section.delivery {
                SectionDelivery::Message(message) => Some((index, message)),
                SectionDelivery::UserContent(_) => None,
            })
            .collect::<Vec<_>>();
        pending.sort_by_key(|(_, message)| {
            (
                matches!(message.retention, Retention::Optional(_)),
                !message.protected,
            )
        });
        let pending = pending
            .into_iter()
            .map(|(index, message)| (Some(index), &message.content.item));
        for (index, item) in reviewer_history
            .iter()
            .map(|item| (None, &item.item))
            .chain(pending)
        {
            let duplicate = match item {
                ResponseItem::AgentMessage {
                    id: Some(id),
                    author,
                    recipient,
                    content,
                    ..
                } if !id.is_empty() => {
                    let payload = (author, recipient, content);
                    seen_messages
                        .insert(id, payload)
                        .map(|previous| previous == payload)
                }
                ResponseItem::FunctionCall {
                    call_id,
                    name,
                    namespace,
                    arguments,
                    encrypted_content,
                    encrypted_function_args,
                    ..
                } => {
                    let payload = (
                        name,
                        namespace,
                        arguments,
                        encrypted_content,
                        encrypted_function_args,
                    );
                    seen_calls
                        .insert(call_id, payload)
                        .map(|previous| previous == payload)
                }
                _ => None,
            };
            match duplicate {
                Some(false) => return Err(SectionError::ConflictingNativeMessage),
                Some(true) => {
                    if let Some(index) = index {
                        duplicates.insert(index);
                    }
                }
                None => {}
            }
        }
        let mut index = 0;
        self.sections.retain(|_| {
            let retain = !duplicates.contains(&index);
            index += 1;
            retain
        });
        Ok(())
    }

    /// Applies host image admission before aggregate selection, preserving section
    /// identity and each retained item's selection policy.
    pub fn retain_images(
        &mut self,
        mut admit: impl FnMut(&ImageReference, &mut Option<ImageDetail>) -> bool,
    ) {
        for section in &mut self.sections {
            retain_content(section, &mut self.truncations, |_, item| match item {
                SectionContent::Other(ContentItem::InputImage { image, detail }) => {
                    admit(image, detail)
                }
                _ => true,
            });
        }
    }

    /// Fits evidence according to the host recovery phase, or returns no context.
    /// The host owns the bounded omission fragment and existing reviewer history.
    pub fn enforce_budget(
        mut self,
        budget: RequestBudget,
        omission_notice: String,
        history_truncation: HistoryTruncation,
    ) -> Result<Self, SectionError> {
        let remaining = budget
            .max_input_tokens
            .checked_sub(budget.existing_context_tokens)
            .ok_or(SectionError::EvidenceLimitExceeded {
                section: "request_budget",
            })?;
        let current = self.estimated_tokens();
        if current <= remaining {
            self.admit_enclosing_call(remaining, /*limits*/ None);
            return Ok(self);
        }
        let notice = SectionOutput {
            id: "budget_omission",
            delivery: SectionDelivery::user_content(vec![Budgeted::required(
                ContentItem::InputText {
                    text: omission_notice,
                },
            )]),
        };
        let mut required_tokens = current.saturating_add(section_tokens(&notice));
        let mut needed = required_tokens.saturating_sub(remaining);
        let mut candidates = Vec::new();
        let mut remaining_items = vec![0; self.sections.len()];
        let mut candidate_framing = vec![0; self.sections.len()];
        for (section_index, section) in self.sections.iter().enumerate() {
            if let SectionDelivery::Message(message) = &section.delivery
                && let Retention::Optional(priority) = message.retention
            {
                let tokens = section_tokens(section);
                required_tokens = required_tokens.saturating_sub(tokens);
                candidates.push((priority, section_index, 0, tokens));
                remaining_items[section_index] = 1;
            }
            if let SectionDelivery::UserContent(content) = &section.delivery {
                let mut required_items = content.len();
                for (index, item) in content.iter().enumerate() {
                    let Retention::Optional(priority) = item.retention else {
                        continue;
                    };
                    required_items -= 1;
                    let tokens = section_content_tokens(&item.content);
                    required_tokens = required_tokens.saturating_sub(tokens);
                    candidates.push((priority, section_index, index, tokens));
                }
                // Recompute framing with only required items, then charge each
                // candidate for the framing needed to add it back on its own.
                let required_framing = content_framing_tokens(required_items);
                required_tokens = required_tokens.saturating_sub(
                    content_framing_tokens(content.len()).saturating_sub(required_framing),
                );
                candidate_framing[section_index] =
                    content_framing_tokens(required_items + 1).saturating_sub(required_framing);
                remaining_items[section_index] = content.len();
            }
        }
        let mut history_truncated = false;
        if history_truncation == HistoryTruncation::Allow && required_tokens > remaining {
            // Source order makes older evidence yield first. Keep each source's
            // label, both ends and the standard marker; later restrictions stay
            // complete whenever older entries can supply the needed space.
            'history: for section in &mut self.sections {
                let SectionDelivery::UserContent(content) = &mut section.delivery else {
                    continue;
                };
                for item in content {
                    if item.retention != Retention::Historical {
                        continue;
                    }
                    let original_tokens = section_content_tokens(&item.content);
                    let original_bytes = item.content.evidence_bytes();
                    let target = original_tokens.saturating_sub(required_tokens - remaining);
                    truncate_content_to_fit(&mut item.content, target, MIN_HISTORICAL_TEXT_TOKENS);
                    let saved =
                        original_tokens.saturating_sub(section_content_tokens(&item.content));
                    if saved == 0 {
                        continue;
                    }
                    self.truncations.push(TruncationObservation {
                        source: None,
                        component: section.id,
                        original_bytes,
                        retained_bytes: item.content.evidence_bytes(),
                    });
                    history_truncated = true;
                    required_tokens = required_tokens.saturating_sub(saved);
                    needed = needed.saturating_sub(saved);
                    if required_tokens <= remaining {
                        break 'history;
                    }
                }
            }
        }
        // Evidence that cannot fit beside the required content and notice must
        // leave first, without evicting useful smaller entries on its behalf.
        let optional_allowance =
            remaining
                .checked_sub(required_tokens)
                .ok_or(SectionError::EvidenceLimitExceeded {
                    section: "request_budget",
                })?;
        let mut removed = HashSet::new();
        let mut remove = |section_index: usize, index: usize, tokens: usize| {
            if !removed.insert((section_index, index)) {
                return 0;
            }
            if matches!(
                self.sections[section_index].delivery,
                SectionDelivery::Message(_)
            ) {
                return tokens;
            }
            let count = &mut remaining_items[section_index];
            let framing = content_framing_tokens(*count);
            *count -= 1;
            tokens.saturating_add(framing.saturating_sub(content_framing_tokens(*count)))
        };
        candidates.retain(|&(_, section_index, index, tokens)| {
            if tokens.saturating_add(candidate_framing[section_index]) <= optional_allowance {
                return true;
            }
            needed = needed.saturating_sub(remove(section_index, index, tokens));
            false
        });
        candidates.sort_unstable();
        for (_, section_index, index, tokens) in candidates {
            if needed == 0 {
                break;
            }
            needed = needed.saturating_sub(remove(section_index, index, tokens));
        }
        if removed.is_empty() && !history_truncated {
            return Err(SectionError::EvidenceLimitExceeded {
                section: "request_budget",
            });
        }
        for (section_index, section) in self.sections.iter_mut().enumerate() {
            if let SectionDelivery::Message(message) = &section.delivery
                && removed.contains(&(section_index, 0))
            {
                self.truncations.push(TruncationObservation {
                    source: None,
                    component: section.id,
                    original_bytes: serde_json::to_vec(&message.content.item)
                        .map_or(usize::MAX, |bytes| bytes.len()),
                    retained_bytes: 0,
                });
                section.delivery = SectionDelivery::UserContent(Vec::new());
            }
            retain_content(section, &mut self.truncations, |index, _| {
                !removed.contains(&(section_index, index))
            });
        }
        self.sections.push(notice);
        if self.estimated_tokens() > remaining {
            return Err(SectionError::EvidenceLimitExceeded {
                section: "request_budget",
            });
        }
        self.admit_enclosing_call(remaining, /*limits*/ None);
        Ok(self)
    }
}

pub(crate) fn retain_content(
    section: &mut SectionOutput,
    truncations: &mut Vec<TruncationObservation>,
    mut retain: impl FnMut(usize, &mut SectionContent) -> bool,
) {
    let SectionDelivery::UserContent(content) = &mut section.delivery else {
        return;
    };
    let mut index = 0;
    content.retain_mut(|item| {
        let keep = retain(index, &mut item.content);
        index += 1;
        if !keep {
            let original_bytes = item.content.evidence_bytes();
            truncations.push(TruncationObservation {
                source: None,
                component: section.id,
                original_bytes,
                retained_bytes: 0,
            });
        }
        keep
    });
}

#[cfg(test)]
#[path = "enforcement_tests.rs"]
mod tests;
