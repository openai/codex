//! Model slugs requiring auto-review remain protected across all policy layers.

use crate::AutoReviewRequirementsToml;
use crate::RequirementSource;
use crate::Sourced;

use super::stack::merge_output_source;

#[derive(Default)]
pub(super) struct AutoReviewModelsMergeState {
    slugs: Vec<String>,
    source: Option<RequirementSource>,
}

impl AutoReviewModelsMergeState {
    pub(super) fn merge(
        &mut self,
        incoming: Option<AutoReviewRequirementsToml>,
        source: &RequirementSource,
    ) {
        let Some(incoming_slugs) = incoming
            .and_then(|auto_review| auto_review.required_on_models)
            .filter(|slugs| !slugs.is_empty())
        else {
            return;
        };

        for slug in incoming_slugs {
            if !self.slugs.contains(&slug) {
                self.slugs.push(slug);
                if let Some(existing_source) = self.source.as_mut() {
                    merge_output_source(existing_source, source);
                } else {
                    self.source = Some(source.clone());
                }
            }
        }
    }

    pub(super) fn apply_to(self, target: &mut Option<Sourced<AutoReviewRequirementsToml>>) {
        if self.slugs.is_empty() {
            return;
        }

        let source = self.source.unwrap_or(RequirementSource::Unknown);
        let Some(existing) = target.as_mut() else {
            *target = Some(Sourced::new(
                AutoReviewRequirementsToml {
                    required_on_models: Some(self.slugs),
                    ignore_rules: None,
                },
                source,
            ));
            return;
        };

        existing.value.required_on_models = Some(self.slugs);
        merge_output_source(&mut existing.source, &source);
    }
}
