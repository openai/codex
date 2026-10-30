//! Applies configured section text caps exactly once before allocation.
//! Required items remain present after truncation. Exact actions, images and opaque
//! native payloads stay whole. Prepared representations do not depend on optional allocation quotas.

use crate::ComposedContext;
use crate::ElasticBudgetConfig;
use crate::PreparedElasticContext;
use crate::SectionError;
use crate::TruncationObservation;
use crate::allocation::allocation_section;
use crate::budget::content_framing_tokens;
use crate::budget::section_content_tokens;
use crate::composition::SectionContent;
use crate::composition::SectionDelivery;
use crate::truncation::truncate_content_to_fit;
use codex_protocol::models::ContentItem;

impl ComposedContext {
    /// Caps supporting text using the same policy for required and optional evidence.
    /// Prepare images first, then deduplicate the returned capped context.
    /// The returned context
    /// can only select whole entries, so later grants cannot change their text.
    /// Omitted groups inherit the same defaults used by allocation.
    /// The existing minimum marker/framing is retained when the cap cannot hold it.
    /// Entry preparation never selects items; allocation may omit Optional entries.
    pub fn prepare_elastic_entries(
        mut self,
        config: &ElasticBudgetConfig,
    ) -> Result<PreparedElasticContext<'_>, SectionError> {
        for section in &mut self.sections {
            // Request scaffolding is not a contributed evidence section.
            if matches!(section.id, "intro" | "budget_omission") {
                continue;
            }
            let limit = |retention| {
                allocation_section(section.id, retention)
                    .map(|group| config.section_limits(group).max_section_entry_tokens())
            };
            match &mut section.delivery {
                // Sync native deliveries carry opaque payloads and stay whole.
                SectionDelivery::Message(_) => {}
                SectionDelivery::UserContent(items) => {
                    for item in items {
                        let maximum = limit(item.retention)?;
                        let original_tokens = section_content_tokens(&item.content);
                        if matches!(
                            item.content,
                            SectionContent::Exact(_)
                                | SectionContent::Other(
                                    ContentItem::InputImage { .. } | ContentItem::InputAudio { .. }
                                )
                        ) || original_tokens
                            .saturating_add(content_framing_tokens(/*item_count*/ 1))
                            <= maximum
                        {
                            continue;
                        }
                        let original_bytes = item.content.evidence_bytes();
                        truncate_content_to_fit(
                            &mut item.content,
                            maximum.saturating_sub(content_framing_tokens(/*item_count*/ 1)),
                            /*min_text_tokens*/ 0,
                        );
                        if section_content_tokens(&item.content) >= original_tokens {
                            continue;
                        }
                        if let Some(source) = &mut item.source {
                            source.complete = false;
                        }
                        self.truncations.push(TruncationObservation {
                            component: section.id,
                            original_bytes,
                            retained_bytes: item.content.evidence_bytes(),
                        });
                    }
                }
            }
        }
        Ok(PreparedElasticContext {
            context: self,
            config,
        })
    }
}

#[cfg(test)]
#[path = "elastic_tests.rs"]
mod tests;
