//! Allocates whole optional entries with shared section framing, preserving content and order.

use std::collections::BTreeMap;
use std::collections::HashSet;

use crate::BudgetPriority;
use crate::Budgeted;
use crate::ComposedContext;
use crate::ElasticBudgetConfig;
use crate::ElasticSection;
use crate::RequestBudget;
use crate::Retention;
use crate::SectionError;
use crate::TruncationObservation;
use crate::budget::ByteCount;
use crate::budget::content_framing_tokens;
use crate::budget::section_content_tokens;
use crate::budget::section_tokens;
use crate::composition::SectionDelivery;
use crate::composition::SectionOutput;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ContentItem;

#[derive(Clone, Copy)]
struct Candidate {
    section_index: usize,
    item_index: usize,
    cost: usize,
}

impl Candidate {
    fn cost(self, content_counts: &[Option<usize>]) -> usize {
        self.cost
            .saturating_add(content_counts[self.section_index].map_or(0, |count| {
                content_framing_tokens(count + 1).saturating_sub(content_framing_tokens(count))
            }))
    }

    fn select(self, content_counts: &mut [Option<usize>]) -> usize {
        let cost = self.cost(content_counts);
        if let Some(count) = &mut content_counts[self.section_index] {
            *count += 1;
        }
        cost
    }
}

#[derive(Default)]
struct GroupDemand {
    pending_items: Vec<Candidate>,
    total_cost: usize,
    quota: usize,
    used: usize,
}

pub(crate) fn allocation_section(
    section: &'static str,
    retention: Retention,
) -> Result<ElasticSection, SectionError> {
    Ok(match section {
        "conversation_transcript" if retention == Retention::Optional(BudgetPriority::Tool) => {
            ElasticSection::TranscriptTools
        }
        "conversation_transcript" => ElasticSection::TranscriptMessages,
        "root_conversation" => ElasticSection::RootConversationMessages,
        "retained_user_instructions" => ElasticSection::RetainedUserInstructions,
        "trusted_user_answers" => ElasticSection::TrustedUserAnswers,
        "sender_user_messages" => ElasticSection::SenderUserMessages,
        "planned_action" => ElasticSection::PlannedAction,
        "previous_reviews" => ElasticSection::PreviousReviews,
        "trusted_tool" => ElasticSection::TrustedTools,
        "trusted_skills" => ElasticSection::TrustedSkills,
        "transcript_images" => ElasticSection::TranscriptImages,
        "node_repl_evidence" => ElasticSection::NodeReplEvidence,
        "permissions" => ElasticSection::PermissionContext,
        _ => return Err(SectionError::UnsupportedDelivery { section }),
    })
}

/// Prepared section text with a borrowed immutable cap policy.
#[derive(Clone)]
pub struct PreparedElasticContext<'a> {
    pub(crate) context: ComposedContext,
    pub(crate) config: &'a ElasticBudgetConfig,
}

impl PreparedElasticContext<'_> {
    /// Deduplicates using the completeness of the capped representations.
    pub fn prepare_sync_delivery(
        mut self,
        history: &[ResponseItemEnvelope],
    ) -> Result<Self, SectionError> {
        crate::prepare_sync_delivery(&mut self.context, history)?;
        Ok(self)
    }

    /// Includes capped evidence and section framing before optional selection.
    pub fn estimated_tokens(&self) -> usize {
        self.context.estimated_tokens()
    }

    /// Reserves required evidence and history, then selects whole optional entries.
    pub fn enforce_budget(
        self,
        budget: RequestBudget,
        omission_notice: String,
    ) -> Result<ComposedContext, SectionError> {
        let mut context = self.context;
        context
            .sections
            .retain(|section| section.id != "budget_omission");
        let remaining = budget
            .max_input_tokens
            .checked_sub(budget.existing_context_tokens)
            .ok_or(SectionError::EvidenceLimitExceeded {
                section: "request_budget",
            })?;
        let demand = collect_demand(&context)?;
        let context_tokens = context.estimated_tokens();
        if context_tokens <= remaining && context.truncations.is_empty() {
            return Ok(context);
        }
        let notice = SectionOutput {
            id: "budget_omission",
            delivery: SectionDelivery::user_content(vec![Budgeted::required(
                ContentItem::InputText {
                    text: omission_notice,
                },
            )]),
        };
        let notice_tokens = section_tokens(&notice);
        if context_tokens.saturating_add(notice_tokens) <= remaining {
            context.sections.push(notice);
            return Ok(context);
        }
        let allowance = remaining
            .checked_sub(demand.required_tokens.saturating_add(notice_tokens))
            .ok_or(SectionError::EvidenceLimitExceeded {
                section: "request_budget",
            })?;
        let selected = allocate_items(demand, self.config, allowance);
        retain_selected(&mut context, &selected);
        context.sections.push(notice);
        if context.estimated_tokens() > remaining {
            return Err(SectionError::EvidenceLimitExceeded {
                section: "request_budget",
            });
        }
        Ok(context)
    }
}

/// Required reservation and optional group demand for one pending request.
struct ContextDemand {
    required_tokens: usize,
    groups: BTreeMap<ElasticSection, GroupDemand>,
    /// User-content counts for shared framing; native messages have no wrapper.
    content_counts: Vec<Option<usize>>,
}

fn collect_demand(context: &ComposedContext) -> Result<ContextDemand, SectionError> {
    let mut required_tokens = 0usize;
    let mut demands = BTreeMap::<ElasticSection, GroupDemand>::new();
    let mut content_counts = Vec::with_capacity(context.sections.len());
    for (section_index, section) in context.sections.iter().enumerate() {
        let required_count = match &section.delivery {
            SectionDelivery::UserContent(items) => Some(
                items
                    .iter()
                    .filter(|item| !matches!(item.retention, Retention::Optional(_)))
                    .count(),
            ),
            SectionDelivery::Message(_) => None,
        };
        content_counts.push(required_count);
        required_tokens =
            required_tokens.saturating_add(required_count.map_or(0, content_framing_tokens));
        let mut count = required_count.unwrap_or(0);
        let mut add = |item_index, priority, cost: usize| {
            let group = allocation_section(section.id, Retention::Optional(priority))?;
            let framing = required_count.map_or(0, |_| {
                content_framing_tokens(count + 1).saturating_sub(content_framing_tokens(count))
            });
            count += 1;
            let demand = demands.entry(group).or_default();
            demand.total_cost = demand
                .total_cost
                .saturating_add(cost.saturating_add(framing));
            demand.pending_items.push(Candidate {
                section_index,
                item_index,
                cost,
            });
            Ok(())
        };
        match &section.delivery {
            SectionDelivery::Message(message) => {
                let cost = section_tokens(section);
                if let Retention::Optional(priority) = message.retention {
                    add(/*item_index*/ 0, priority, cost)?;
                } else {
                    required_tokens = required_tokens.saturating_add(cost);
                }
            }
            SectionDelivery::UserContent(items) => {
                for (index, item) in items.iter().enumerate() {
                    let cost = section_content_tokens(&item.content);
                    if let Retention::Optional(priority) = item.retention {
                        add(index, priority, cost)?;
                    } else {
                        required_tokens = required_tokens.saturating_add(cost);
                    }
                }
            }
        }
    }
    Ok(ContextDemand {
        required_tokens,
        groups: demands,
        content_counts,
    })
}

fn allocate_items(
    demand: ContextDemand,
    config: &ElasticBudgetConfig,
    allowance: usize,
) -> HashSet<(usize, usize)> {
    let mut demands = demand.groups;
    let mut content_counts = demand.content_counts;
    for (group, demand) in demands.iter_mut() {
        let limits = config.section_limits(*group);
        // Prefer one whole entry per group when the remaining space permits it.
        let smallest = demand
            .pending_items
            .iter()
            .map(|item| item.cost(&content_counts))
            .min()
            .unwrap_or(0);
        demand.quota = limits
            .min_total_section_tokens()
            .max(smallest)
            .min(demand.total_cost);
    }
    let minimum: u128 = demands.values().map(|demand| demand.quota as u128).sum();
    if minimum > allowance as u128 {
        for demand in demands.values_mut() {
            demand.quota = (demand.quota as u128 * allowance as u128 / minimum) as usize;
        }
    }
    let mut available = allowance - demands.values().map(|demand| demand.quota).sum::<usize>();
    loop {
        let active = demands
            .values()
            .filter(|demand| demand.total_cost > demand.quota)
            .count();
        if active == 0 || available / active == 0 {
            break;
        }
        let share = available / active;
        for demand in demands.values_mut() {
            let granted = share.min(demand.total_cost - demand.quota);
            demand.quota += granted;
            available -= granted;
        }
    }
    let mut selected = HashSet::new();
    let mut remaining = allowance;
    for demand in demands.values_mut() {
        let mut deferred = Vec::new();
        for candidate in demand.pending_items.drain(..).rev() {
            if candidate.cost(&content_counts) <= demand.quota - demand.used {
                let cost = candidate.select(&mut content_counts);
                selected.insert((candidate.section_index, candidate.item_index));
                demand.used += cost;
                remaining -= cost;
            } else {
                deferred.push(candidate);
            }
        }
        deferred.reverse();
        demand.pending_items = deferred;
    }
    // Return packing gaps to the least-served group that can still use them.
    while remaining > 0 {
        let Some(demand) = demands
            .values_mut()
            .filter(|demand| !demand.pending_items.is_empty())
            .min_by_key(|demand| demand.used)
        else {
            break;
        };
        let Some(candidate) = demand.pending_items.pop() else {
            continue;
        };
        if candidate.cost(&content_counts) <= remaining {
            let cost = candidate.select(&mut content_counts);
            selected.insert((candidate.section_index, candidate.item_index));
            demand.used += cost;
            remaining -= cost;
        }
    }
    selected
}

fn retain_selected(context: &mut ComposedContext, selected: &HashSet<(usize, usize)>) {
    for (section_index, section) in context.sections.iter_mut().enumerate() {
        match &section.delivery {
            SectionDelivery::Message(message)
                if matches!(message.retention, Retention::Optional(_))
                    && !selected.contains(&(section_index, 0)) =>
            {
                context.truncations.push(TruncationObservation {
                    component: section.id,
                    original_bytes: ByteCount::item(&message.content.item),
                    retained_bytes: 0,
                });
                section.delivery = SectionDelivery::UserContent(Vec::new());
            }
            SectionDelivery::UserContent(items) => {
                let keep = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        !matches!(item.retention, Retention::Optional(_))
                            || selected.contains(&(section_index, index))
                    })
                    .collect::<Vec<_>>();
                crate::enforcement::retain_content(
                    section,
                    &mut context.truncations,
                    |index, _| keep[index],
                );
            }
            SectionDelivery::Message(_) => {}
        }
    }
}

#[cfg(test)]
#[path = "allocation_tests.rs"]
mod tests;
