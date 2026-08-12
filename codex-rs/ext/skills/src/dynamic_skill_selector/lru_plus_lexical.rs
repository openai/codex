use super::CheapSkillSelection;
use super::CheapSkillSelector;
use super::LruSkillSelector;
use super::SkillSelectionDocument;
use super::WeightedLexicalSkillSelector;
use super::rrf_lexical_char::fuse_rankings_with_constant;

const MAX_RESULTS: usize = 50;
const RRF_K: usize = 10;

#[derive(Clone, Debug, Default)]
pub(crate) struct LruPlusLexicalSkillSelector {
    lru_selector: LruSkillSelector,
}

impl LruPlusLexicalSkillSelector {
    pub(crate) fn new(lru_selector: LruSkillSelector) -> Self {
        Self { lru_selector }
    }
}

impl CheapSkillSelector for LruPlusLexicalSkillSelector {
    fn method(&self) -> &'static str {
        "lru_plus_lexical_v1"
    }

    fn select(
        &self,
        query: &str,
        documents: &[SkillSelectionDocument<'_>],
        limit: usize,
    ) -> CheapSkillSelection {
        if limit == 0 {
            return CheapSkillSelection::default();
        }

        let recent = self
            .lru_selector
            .select(query, documents, /*limit*/ MAX_RESULTS);
        let lexical =
            WeightedLexicalSkillSelector.select(query, documents, /*limit*/ MAX_RESULTS);
        let candidate_ids = fuse_rankings_with_constant(
            [&recent.candidate_ids, &lexical.candidate_ids],
            limit.min(MAX_RESULTS),
            /*rank_constant*/ RRF_K,
        );

        CheapSkillSelection {
            candidate_ids,
            query_term_count: recent.query_term_count.max(lexical.query_term_count),
            query_truncated: recent.query_truncated || lexical.query_truncated,
            candidate_set_truncated: recent.candidate_set_truncated
                || lexical.candidate_set_truncated,
        }
    }
}

#[cfg(test)]
#[path = "lru_plus_lexical_tests.rs"]
mod tests;
