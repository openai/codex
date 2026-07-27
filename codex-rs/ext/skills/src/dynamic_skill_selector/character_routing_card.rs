use std::collections::HashMap;

use codex_core_skills::HostSkillsSnapshot;

use crate::catalog::SkillCatalog;
use crate::catalog::SkillSourceKind;

use super::CharacterNgramSkillSelector;
use super::CheapSkillSelection;
use super::CheapSkillSelector;
use super::SkillSelectionDocument;

const MAX_ROUTING_FIELD_BYTES: usize = 4 * 1024;
const MAX_DEPENDENCY_RECORDS: usize = 32;
const MAX_CANDIDATES: usize = 1_000;

#[derive(Clone, Debug, Default)]
pub(crate) struct CharacterRoutingCardSkillSelector {
    routing_fields: HashMap<usize, String>,
}

impl CharacterRoutingCardSkillSelector {
    pub(crate) fn new(catalog: &SkillCatalog, host_snapshot: Option<&HostSkillsSnapshot>) -> Self {
        let host_skill_ids = catalog
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.authority.kind == SkillSourceKind::Host && entry.is_model_visible()
            })
            .take(MAX_CANDIDATES)
            .map(|(id, entry)| (entry.main_prompt.as_str().replace('\\', "/"), id))
            .collect::<HashMap<_, _>>();
        let routing_fields = host_snapshot
            .into_iter()
            .flat_map(|snapshot| snapshot.outcome().skills.iter())
            .filter_map(|skill| {
                let id = host_skill_ids
                    .get(&skill.path_to_skills_md.to_string_lossy().replace('\\', "/"))?;
                let interface = skill.interface.as_ref()?;
                let mut field = String::new();
                for value in [
                    interface.display_name.as_deref(),
                    interface.short_description.as_deref(),
                    interface.default_prompt.as_deref(),
                ]
                .into_iter()
                .flatten()
                {
                    push_bounded(&mut field, value);
                }
                (!field.is_empty()).then_some((*id, field))
            })
            .take(MAX_CANDIDATES)
            .collect();
        Self { routing_fields }
    }
}

impl CheapSkillSelector for CharacterRoutingCardSkillSelector {
    fn method(&self) -> &'static str {
        "character_routing_card_v1"
    }

    fn select(
        &self,
        query: &str,
        documents: &[SkillSelectionDocument<'_>],
        limit: usize,
    ) -> CheapSkillSelection {
        if limit == 0 {
            return CharacterNgramSkillSelector.select(query, documents, /*limit*/ 0);
        }

        let routing_fields = documents
            .iter()
            .take(MAX_CANDIDATES)
            .map(|document| {
                let mut field = String::new();
                if let Some(description) = document.short_description {
                    push_bounded(&mut field, description);
                }
                if let Some(routing_field) = self.routing_fields.get(&document.id) {
                    push_bounded(&mut field, routing_field);
                }
                if let Some(dependencies) = document.dependencies {
                    for dependency in dependencies.tools.iter().take(MAX_DEPENDENCY_RECORDS) {
                        push_bounded(&mut field, dependency.value.as_str());
                        if let Some(description) = dependency.description.as_deref() {
                            push_bounded(&mut field, description);
                        }
                    }
                }
                field
            })
            .collect::<Vec<_>>();
        let routing_documents = documents
            .iter()
            .zip(&routing_fields)
            .map(|(document, field)| SkillSelectionDocument {
                short_description: (!field.is_empty()).then_some(field.as_str()),
                ..*document
            })
            .collect::<Vec<_>>();
        let selection = CharacterNgramSkillSelector.select(query, &routing_documents, limit);
        CheapSkillSelection {
            candidate_set_truncated: documents.len() > MAX_CANDIDATES
                || selection.candidate_set_truncated,
            ..selection
        }
    }
}

fn push_bounded(destination: &mut String, value: &str) {
    if value.is_empty() || destination.len() == MAX_ROUTING_FIELD_BYTES {
        return;
    }
    if !destination.is_empty() {
        destination.push(' ');
    }
    let remaining = MAX_ROUTING_FIELD_BYTES.saturating_sub(destination.len());
    let mut end = value.len().min(remaining);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    destination.push_str(&value[..end]);
}

#[cfg(test)]
#[path = "character_routing_card_tests.rs"]
mod tests;
