//! Fits newly composed evidence into the remaining complete-request allowance.
//! Required action evidence is never truncated. Optional evidence leaves first;
//! hosts may shorten historical instructions after compaction cannot make room.
//! Every reduction reserves an omission notice and preserves source order.
//! JSON recovery shortens only the body; line mode shortens the whole rendered entry.

use std::collections::HashSet;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use codex_protocol::protocol::TruncationPolicy;

use crate::ComposedContext;
use crate::RequestBudget;
use crate::SectionError;
use crate::TruncationObservation;
use crate::budget::content_framing_tokens;
use crate::budget::section_content_tokens;
use crate::budget::section_tokens;
use crate::composition::SectionContent;
use crate::composition::SectionDelivery;
use crate::composition::SectionOutput;

/// Eviction priority within a profile; older items at the same priority go first.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum BudgetPriority {
    ToolDescription,
    Commentary,
    Tool,
    Image,
}

/// Content and its selection policy move together through rendering and admission.
/// Consumers may add required content; optional priorities stay crate-owned.
#[derive(Clone, PartialEq)]
pub struct Budgeted<T> {
    pub content: T,
    pub(crate) source: Option<codex_history::RetainedSource>,
    pub(crate) retention: Retention,
}

impl<T> Budgeted<T> {
    pub fn required(content: T) -> Self {
        Self {
            content,
            source: None,
            retention: Retention::Required,
        }
    }

    pub(crate) fn historical(content: T) -> Self {
        Self {
            content,
            source: None,
            retention: Retention::Historical,
        }
    }

    pub(crate) fn optional(content: T, priority: BudgetPriority) -> Self {
        Self {
            content,
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
                    let original_bytes = match &item.content {
                        SectionContent::Transcript(record) => record.rendered().text_bytes,
                        SectionContent::Other(ContentItem::InputText { text }) => text.len(),
                        _ => continue,
                    };
                    let Some(original_body) = item.content.replace_text(String::new()) else {
                        continue;
                    };
                    let target = original_tokens.saturating_sub(required_tokens - remaining);
                    let mut upper = TruncationPolicy::Bytes(original_body.len()).token_budget();
                    let mut lower = 32.min(upper);
                    let mut selected = lower;
                    while lower < upper {
                        let mid = lower + (upper - lower).div_ceil(2);
                        let _ = item
                            .content
                            .replace_text(crate::truncate_text(&original_body, mid));
                        if section_content_tokens(&item.content) <= target {
                            lower = mid;
                            selected = mid;
                        } else {
                            upper = mid - 1;
                        }
                    }
                    let _ = item
                        .content
                        .replace_text(crate::truncate_text(&original_body, selected));
                    let saved =
                        original_tokens.saturating_sub(section_content_tokens(&item.content));
                    if saved == 0 {
                        let _ = item.content.replace_text(original_body);
                        continue;
                    }
                    self.truncations.push(TruncationObservation {
                        component: section.id,
                        original_bytes,
                        retained_bytes: match &item.content {
                            SectionContent::Transcript(record) => record.rendered().text_bytes,
                            SectionContent::Other(ContentItem::InputText { text }) => text.len(),
                            _ => 0,
                        },
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
        Ok(self)
    }
}

fn retain_content(
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
            let original_bytes = match &item.content {
                SectionContent::Transcript(record) => record.rendered().text_bytes,
                SectionContent::Other(
                    ContentItem::InputText { text } | ContentItem::OutputText { text },
                ) => text.len(),
                SectionContent::Other(ContentItem::InputImage {
                    image: ImageReference::Inline { image_url },
                    ..
                }) => image_url.len(),
                SectionContent::Other(ContentItem::InputImage {
                    image: ImageReference::File { .. },
                    ..
                }) => 0,
                SectionContent::Other(ContentItem::InputAudio { audio_url }) => audio_url.len(),
            };
            truncations.push(TruncationObservation {
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
