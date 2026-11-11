//! Funds fixed section minimums and scales only the headroom up to section maxima.
//! Required records precede Optional records within each section.
//! Protected framing and exact actions use global space. Selection preserves prepared bytes and order.

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

const REQUEST_BUDGET_EXCEEDED: SectionError = SectionError::EvidenceLimitExceeded {
    section: "request_budget",
};

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

// Quota counts start after the global wrapper, reserving ceil(parts / 4)
// separator tokens independently of selections in other allocation groups.
fn quota_counts(counts: &[Option<usize>]) -> Vec<Option<usize>> {
    counts.iter().map(|count| count.map(|_| 1)).collect()
}

#[derive(Default)]
struct SectionDemand {
    required_items: Vec<Candidate>,
    optional_items: Vec<Candidate>,
    selected_items: Vec<Candidate>,
    total_cost: usize,
    maximum: usize,
    quota: usize,
    used: usize,
}

impl SectionDemand {
    fn allocate(
        &mut self,
        quota: usize,
        remaining: &mut usize,
        counts: &mut [Option<usize>],
        selected: &mut HashSet<(usize, usize)>,
        pinned: &HashSet<(usize, usize)>,
    ) -> Result<(), SectionError> {
        // Reconsider Optional picks when a larger grant can hold Required evidence.
        for item in self.selected_items.drain(..) {
            selected.remove(&(item.section_index, item.item_index));
            *remaining += item.cost;
            if let Some(count) = &mut counts[item.section_index] {
                *remaining += content_framing_tokens(*count) - content_framing_tokens(*count - 1);
                *count -= 1;
            }
        }
        self.used = 0;
        // Reserve separator rounding per group and delivery so other groups cannot
        // reprice pinned evidence. The global budget pays shared framing exactly once.
        let mut quota_counts = quota_counts(counts);
        let mut keep = |item: Candidate| {
            if item.cost(&quota_counts) > quota - self.used || item.cost(counts) > *remaining {
                return false;
            }
            let cost = item.select(counts);
            self.used += item.select(&mut quota_counts);
            *remaining -= cost;
            selected.insert((item.section_index, item.item_index));
            self.selected_items.push(item);
            true
        };
        for item in self
            .required_items
            .iter()
            .filter(|item| pinned.contains(&(item.section_index, item.item_index)))
        {
            if !keep(*item) {
                return Err(REQUEST_BUDGET_EXCEEDED);
            }
        }
        for item in self
            .required_items
            .iter()
            .rev()
            .filter(|item| !pinned.contains(&(item.section_index, item.item_index)))
        {
            if !keep(*item) {
                break;
            }
        }
        for item in self.optional_items.iter().rev() {
            keep(*item);
        }
        Ok(())
    }
}

pub(crate) fn allocation_section(
    section: &'static str,
    retention: Retention,
) -> Result<ElasticSection, SectionError> {
    Ok(match section {
        "conversation_transcript"
            if retention == Retention::Optional(BudgetPriority::SelectedExecResult) =>
        {
            ElasticSection::TranscriptSelectedExecResults
        }
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
    pub(crate) delivered_omissions: Option<codex_history::GuardianRetainedOmissions>,
}

impl PreparedElasticContext<'_> {
    /// Removes history-delivered instructions and deduplicates native deliveries.
    /// Pending instruction proof waits for Required selection.
    pub fn prepare_sync_delivery(
        mut self,
        history: &[ResponseItemEnvelope],
    ) -> Result<Self, SectionError> {
        self.context.remove_delivered_instructions(
            history
                .iter()
                .filter_map(|item| item.metadata.as_ref())
                .flat_map(|metadata| &metadata.guardian_sources),
            crate::ContextBudgeting::Elastic,
        );
        self.context.retain_new_native_messages(history)?;
        self.delivered_omissions = history
            .iter()
            .rev()
            .find_map(|item| item.metadata.as_ref()?.guardian_retained_omissions);
        Ok(self)
    }

    /// Includes all capped evidence and framing before section selection.
    pub fn estimated_tokens(&self) -> usize {
        self.context.estimated_tokens()
    }

    /// Funds every section's minimum, then grows allocations up to section maxima.
    /// Required records have priority in both phases; complete transcript proof stays selected.
    pub fn enforce_budget(
        self,
        budget: RequestBudget,
        omission_notice: String,
    ) -> Result<ComposedContext, SectionError> {
        self.config.validate()?;
        let mut context = self.context;
        context
            .sections
            .retain(|section| section.id != "budget_omission");
        let remaining = budget
            .max_input_tokens
            .checked_sub(budget.existing_context_tokens)
            .ok_or(REQUEST_BUDGET_EXCEEDED)?;
        let notice = SectionOutput {
            id: "budget_omission",
            delivery: SectionDelivery::user_content(vec![Budgeted::protected(
                ContentItem::InputText {
                    text: omission_notice,
                },
            )]),
        };
        context.prune_omission_notices(self.delivered_omissions);
        let mut pinned = HashSet::new();
        let mut minimums_funded = false;
        let (selected, needs_notice) = loop {
            let mut demand = collect_demand(&context)?;
            let exceeds_maximum = demand.sections.iter().any(|(section, demand)| {
                self.config
                    .section_limits(*section)
                    .max_total_section_tokens()
                    .is_some_and(|maximum| demand.total_cost > maximum)
            });
            let needs_notice = !context.truncations.is_empty()
                || exceeds_maximum
                || context.estimated_tokens() > remaining;
            let notice_tokens = if needs_notice {
                section_tokens(&notice)
            } else {
                0
            };
            let allowance = remaining
                .checked_sub(demand.protected_tokens.saturating_add(notice_tokens))
                .ok_or(REQUEST_BUDGET_EXCEEDED)?;
            let selected = allocate_items(&mut demand, self.config, allowance, &pinned)?;
            minimums_funded |= demand.minimums_fit;
            let sources = context
                .required_transcript_sources(|section, item| selected.contains(&(section, item)));
            if !context
                .remove_delivered_instructions(sources.iter(), crate::ContextBudgeting::Elastic)
            {
                if !minimums_funded {
                    return Err(REQUEST_BUDGET_EXCEEDED);
                }
                break (selected, needs_notice);
            }
            // Duplicate credit can fund more evidence in every section. Keep transcript
            // proof already used above selected during that refill. Only originals are
            // removed here, so the loop is bounded by the number of retained originals.
            for (section, entries) in &demand.sections {
                if matches!(
                    section,
                    ElasticSection::TranscriptMessages | ElasticSection::TranscriptTools
                ) {
                    pinned.extend(
                        entries
                            .required_items
                            .iter()
                            .filter(|item| {
                                selected.contains(&(item.section_index, item.item_index))
                            })
                            .map(|item| (item.section_index, item.item_index)),
                    );
                }
            }
        };
        retain_selected(&mut context, &selected);
        context.retain_selected_instructions(self.delivered_omissions);
        if needs_notice || !context.truncations.is_empty() {
            context.sections.push(notice);
        }
        if context.estimated_tokens() > remaining {
            return Err(REQUEST_BUDGET_EXCEEDED);
        }
        Ok(context)
    }
}

/// Protected global cost and priority-ordered demand for each section.
struct ContextDemand {
    protected_tokens: usize,
    sections: BTreeMap<ElasticSection, SectionDemand>,
    /// User-content counts for shared framing; native messages have no wrapper.
    content_counts: Vec<Option<usize>>,
    /// Whether reservations fit before packing gaps are redistributed.
    minimums_fit: bool,
}

fn collect_demand(context: &ComposedContext) -> Result<ContextDemand, SectionError> {
    let mut protected_tokens = 0usize;
    let mut sections = BTreeMap::<ElasticSection, SectionDemand>::new();
    let mut content_counts = Vec::with_capacity(context.sections.len());
    for (section_index, section) in context.sections.iter().enumerate() {
        let protected = |item: &Budgeted<crate::composition::SectionContent>| {
            section.id == "intro" || item.protected
        };
        let protected_count = match &section.delivery {
            SectionDelivery::UserContent(items) => {
                Some(items.iter().filter(|item| protected(item)).count())
            }
            SectionDelivery::Message(_) => None,
        };
        content_counts.push(protected_count);
        protected_tokens =
            protected_tokens.saturating_add(protected_count.map_or(0, content_framing_tokens));
        let mut add = |item_index, retention, cost: usize| -> Result<(), SectionError> {
            let demand = sections
                .entry(allocation_section(section.id, retention)?)
                .or_default();
            let candidate = Candidate {
                section_index,
                item_index,
                cost,
            };
            match retention {
                Retention::Optional(_) => demand.optional_items.push(candidate),
                Retention::Required | Retention::Historical => {
                    demand.required_items.push(candidate)
                }
            }
            Ok(())
        };
        match &section.delivery {
            SectionDelivery::Message(message) => {
                let cost = section_tokens(section);
                if message.protected || section.id == "intro" {
                    protected_tokens = protected_tokens.saturating_add(cost);
                } else {
                    add(/*item_index*/ 0, message.retention, cost)?;
                }
            }
            SectionDelivery::UserContent(items) => {
                for (index, item) in items.iter().enumerate() {
                    let cost = section_content_tokens(&item.content);
                    if protected(item) {
                        protected_tokens = protected_tokens.saturating_add(cost);
                    } else {
                        add(index, item.retention, cost)?;
                    }
                }
            }
        }
    }
    for section in sections.values_mut() {
        let mut counts = quota_counts(&content_counts);
        section.total_cost = section
            .required_items
            .iter()
            .chain(&section.optional_items)
            .map(|item| item.select(&mut counts))
            .sum();
    }
    Ok(ContextDemand {
        protected_tokens,
        sections,
        content_counts,
        minimums_fit: false,
    })
}

fn allocate_items(
    demand: &mut ContextDemand,
    config: &ElasticBudgetConfig,
    allowance: usize,
    pinned: &HashSet<(usize, usize)>,
) -> Result<HashSet<(usize, usize)>, SectionError> {
    let demands = &mut demand.sections;
    let mut counts = demand.content_counts.clone();
    let mut selected = HashSet::new();
    let mut remaining = allowance;
    for (group, demand) in demands.iter_mut() {
        let limits = config.section_limits(*group);
        demand.maximum = limits.max_total_section_tokens().unwrap_or(usize::MAX);
        let mut quota_counts = quota_counts(&counts);
        for item in &demand.required_items {
            if pinned.contains(&(item.section_index, item.item_index)) {
                let cost = item.select(&mut counts);
                remaining = remaining.checked_sub(cost).ok_or(REQUEST_BUDGET_EXCEEDED)?;
                demand.used += item.select(&mut quota_counts);
                demand.selected_items.push(*item);
                selected.insert((item.section_index, item.item_index));
            }
        }
        if demand.used > demand.maximum {
            return Err(REQUEST_BUDGET_EXCEEDED);
        }
        demand.quota = limits
            .min_total_section_tokens()
            .min(demand.total_cost)
            .max(demand.used);
    }
    let reserved: u128 = demands
        .values()
        .map(|demand| (demand.quota - demand.used) as u128)
        .sum();
    // Selection may provisionally establish duplicate proof before minimums fit.
    // Admission requires funded reservations; later refills can reuse packing gaps.
    demand.minimums_fit = reserved <= remaining as u128;
    let growth_budget = (remaining as u128).saturating_sub(reserved);
    let headroom: u128 = demands
        .values()
        .map(|demand| {
            demand
                .total_cost
                .min(demand.maximum)
                .saturating_sub(demand.quota) as u128
        })
        .sum();
    // Scale only the headroom above the fixed minimum, then select once.
    for demand in demands.values_mut() {
        let growth = demand
            .total_cost
            .min(demand.maximum)
            .saturating_sub(demand.quota);
        demand.quota += (growth as u128 * growth_budget.min(headroom))
            .checked_div(headroom)
            .unwrap_or(0) as usize;
    }
    for demand in demands.values_mut() {
        demand.allocate(
            demand.quota,
            &mut remaining,
            &mut counts,
            &mut selected,
            pinned,
        )?;
    }
    // Local quotas can round up by one more token per delivery than shared framing.
    // Allow that quota headroom while selection still checks actual global space.
    let rounding_headroom = counts.iter().filter(|count| count.is_some()).count();
    // Revisit priority selection as packing gaps enlarge a section's final grant.
    while let Some(demand) = demands
        .values_mut()
        .filter(|demand| {
            demand.quota
                < demand
                    .used
                    .saturating_add(remaining)
                    .saturating_add(rounding_headroom)
                    .min(demand.maximum)
                    .min(demand.total_cost)
        })
        .min_by_key(|demand| demand.used)
    {
        demand.quota = demand
            .used
            .saturating_add(remaining)
            .saturating_add(rounding_headroom)
            .min(demand.maximum)
            .min(demand.total_cost);
        demand.allocate(
            demand.quota,
            &mut remaining,
            &mut counts,
            &mut selected,
            pinned,
        )?;
    }

    Ok(selected)
}

fn retain_selected(context: &mut ComposedContext, selected: &HashSet<(usize, usize)>) {
    for (section_index, section) in context.sections.iter_mut().enumerate() {
        if section.id == "intro" {
            continue;
        }
        match &section.delivery {
            SectionDelivery::Message(message)
                if !message.protected && !selected.contains(&(section_index, 0)) =>
            {
                context.truncations.push(TruncationObservation {
                    source: message.source.clone(),
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
                        item.protected || selected.contains(&(section_index, index))
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
