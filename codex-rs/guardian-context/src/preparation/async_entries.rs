//! Caps typed classifier evidence before it becomes trusted developer messages.
//! Delivery metadata must describe only the complete records kept for rendering.

use crate::CollectedContext;
use crate::ContextSection;
use crate::ElasticBudgetConfig;
use crate::ElasticSection;
use crate::TruncationObservation;
use crate::budget::content_framing_tokens;
use crate::budget::content_tokens;
use crate::trusted_tool::MAX_TRUSTED_TOOL_CONTEXT_TOKENS;
use codex_protocol::models::ContentItem;
use codex_protocol::models::input_text;
use codex_protocol::protocol::TruncationPolicy;

pub(super) fn apply(
    context: &mut CollectedContext,
    config: &ElasticBudgetConfig,
    truncations: &mut Vec<TruncationObservation>,
) {
    let cap = |section| config.section_limits(section).max_section_entry_tokens();
    let mut record_omission = |component, original_bytes| {
        truncations.push(TruncationObservation {
            source: None,
            component,
            original_bytes,
            retained_bytes: 0,
        });
    };
    context.sections.retain_mut(|section| match section {
        ContextSection::TrustedTool(tool) => {
            let attestation = tool.attestation();
            let bytes = attestation.len();
            let rendering_limit =
                TruncationPolicy::Tokens(MAX_TRUSTED_TOOL_CONTEXT_TOKENS).byte_budget();
            let keep = bytes <= rendering_limit
                && fits(&input_text(attestation), cap(ElasticSection::TrustedTools));
            if !keep {
                record_omission("trusted_tool", bytes);
            }
            keep
        }
        ContextSection::TrustedSkills(skills) => {
            let maximum = cap(ElasticSection::TrustedSkills);
            for path in std::mem::take(&mut skills.paths) {
                let path_bytes = path.len();
                skills.paths.push(path);
                let (rendered, count) = skills.render_paths();
                if count != skills.paths.len() || !fits(&rendered, maximum) {
                    drop(skills.paths.pop());
                    record_omission("trusted_skills", path_bytes);
                }
            }
            !skills.paths.is_empty()
        }
        ContextSection::PreviousReviews(reviews) => {
            let maximum = cap(ElasticSection::PreviousReviews);
            reviews.retain(|review| {
                let keep = fits(&input_text(review.fragment.clone()), maximum);
                if !keep {
                    record_omission("previous_reviews", review.fragment.len());
                }
                keep
            })
        }
        _ => true,
    });
}

fn fits(content: &ContentItem, maximum: usize) -> bool {
    content_tokens(content).saturating_add(content_framing_tokens(/*item_count*/ 1)) <= maximum
}

#[cfg(test)]
#[path = "async_entries_tests.rs"]
mod tests;
