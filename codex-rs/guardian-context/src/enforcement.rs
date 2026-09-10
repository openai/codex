//! Fits newly composed evidence into the remaining complete-request allowance.
//! Required action evidence is never truncated. Optional evidence leaves first;
//! hosts may shorten historical instructions after compaction cannot make room.
//! Every reduction reserves an omission notice and preserves source order.

use std::collections::HashSet;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::protocol::TruncationPolicy;

use crate::ComposedContext;
use crate::RequestBudget;
use crate::SectionError;
use crate::TruncationObservation;
use crate::budget::content_framing_tokens;
use crate::budget::content_tokens;
use crate::budget::section_tokens;
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
    pub(crate) retention: Retention,
}

impl<T> Budgeted<T> {
    pub fn required(content: T) -> Self {
        Self {
            content,
            retention: Retention::Required,
        }
    }

    pub(crate) fn historical(content: T) -> Self {
        Self {
            content,
            retention: Retention::Historical,
        }
    }

    pub(crate) fn optional(content: T, priority: BudgetPriority) -> Self {
        Self {
            content,
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
    pub fn retain_images(&mut self, mut admit: impl FnMut(&str, &mut Option<ImageDetail>) -> bool) {
        for section in &mut self.sections {
            retain_content(section, &mut self.truncations, |_, item| match item {
                ContentItem::InputImage { image_url, detail } => admit(image_url, detail),
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
            delivery: SectionDelivery::UserContent(vec![Budgeted::required(
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
            if let SectionDelivery::UserContent(content) = &section.delivery {
                let mut required_items = content.len();
                for (index, item) in content.iter().enumerate() {
                    let Retention::Optional(priority) = item.retention else {
                        continue;
                    };
                    required_items -= 1;
                    let tokens = content_tokens(&item.content);
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
                    let original_tokens = content_tokens(&item.content);
                    let ContentItem::InputText { text } = &mut item.content else {
                        continue;
                    };
                    let target = original_tokens.saturating_sub(required_tokens - remaining);
                    let mut upper = TruncationPolicy::Bytes(text.len()).token_budget();
                    let mut lower = 32.min(upper);
                    let mut shortened = crate::truncate_text(text, lower);
                    while lower < upper {
                        let mid = lower + (upper - lower).div_ceil(2);
                        let candidate = crate::truncate_text(text, mid);
                        if content_tokens(&ContentItem::InputText {
                            text: candidate.clone(),
                        }) <= target
                        {
                            lower = mid;
                            shortened = candidate;
                        } else {
                            upper = mid - 1;
                        }
                    }
                    let saved =
                        original_tokens.saturating_sub(content_tokens(&ContentItem::InputText {
                            text: shortened.clone(),
                        }));
                    if saved == 0 {
                        continue;
                    }
                    self.truncations.push(TruncationObservation {
                        component: section.id,
                        original_bytes: text.len(),
                        retained_bytes: shortened.len(),
                    });
                    *text = shortened;
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
    mut retain: impl FnMut(usize, &mut ContentItem) -> bool,
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
                ContentItem::InputText { text } | ContentItem::OutputText { text } => text.len(),
                ContentItem::InputImage { image_url, .. } => image_url.len(),
                ContentItem::InputAudio { audio_url } => audio_url.len(),
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
