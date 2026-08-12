use std::collections::HashSet;

use super::CheapSkillSelection;
use super::CheapSkillSelector;
use super::SkillSelectionDocument;

const MAX_CANDIDATES: usize = 1_000;
const MAX_QUERY_TERMS: usize = 64;
const MAX_RESULTS: usize = 50;

#[derive(Clone, Debug, Default)]
pub(crate) struct LruSkillSelector {
    recent_skill_ids: Vec<usize>,
}

impl LruSkillSelector {
    pub(crate) fn new(recent_skill_ids: Vec<usize>) -> Self {
        Self { recent_skill_ids }
    }
}

impl CheapSkillSelector for LruSkillSelector {
    fn method(&self) -> &'static str {
        "lru_v1"
    }

    fn select(
        &self,
        query: &str,
        documents: &[SkillSelectionDocument<'_>],
        limit: usize,
    ) -> CheapSkillSelection {
        let mut query_terms = query.split_whitespace();
        let query_term_count = query_terms.by_ref().take(MAX_QUERY_TERMS).count();
        let query_truncated = query_terms.next().is_some();
        let candidate_set_truncated = documents.len() > MAX_CANDIDATES;
        let eligible_ids = documents
            .iter()
            .take(MAX_CANDIDATES)
            .map(|document| document.id)
            .collect::<HashSet<_>>();
        let mut seen_ids = HashSet::new();

        CheapSkillSelection {
            candidate_ids: self
                .recent_skill_ids
                .iter()
                .copied()
                .filter(|id| eligible_ids.contains(id) && seen_ids.insert(*id))
                .take(limit.min(MAX_RESULTS))
                .collect(),
            query_term_count,
            query_truncated,
            candidate_set_truncated,
        }
    }
}

#[cfg(test)]
#[path = "lru_tests.rs"]
mod tests;
