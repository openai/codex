use std::collections::HashMap;

use super::CharacterNgramSkillSelector;
use super::CheapSkillSelection;
use super::CheapSkillSelector;
use super::SkillSelectionDocument;
use super::WeightedLexicalSkillSelector;

const MAX_FUSION_CANDIDATES: usize = 50;
const RRF_K: usize = 60;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RrfLexicalCharSkillSelector;

impl CheapSkillSelector for RrfLexicalCharSkillSelector {
    fn method(&self) -> &'static str {
        "rrf_lexical_char_v1"
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

        let lexical = WeightedLexicalSkillSelector.select(
            query,
            documents,
            /*limit*/ MAX_FUSION_CANDIDATES,
        );
        let character = CharacterNgramSkillSelector.select(
            query,
            documents,
            /*limit*/ MAX_FUSION_CANDIDATES,
        );
        let candidate_ids = fuse_rankings(
            [&lexical.candidate_ids, &character.candidate_ids],
            limit.min(MAX_FUSION_CANDIDATES),
        );

        CheapSkillSelection {
            candidate_ids,
            query_term_count: lexical.query_term_count.max(character.query_term_count),
            query_truncated: lexical.query_truncated || character.query_truncated,
            candidate_set_truncated: lexical.candidate_set_truncated
                || character.candidate_set_truncated,
        }
    }
}

fn fuse_rankings<const N: usize>(rankings: [&[usize]; N], limit: usize) -> Vec<usize> {
    let mut scores = HashMap::<usize, f64>::new();
    let mut best_ranks = HashMap::<usize, usize>::new();
    for ranking in rankings {
        for (index, id) in ranking.iter().copied().enumerate() {
            let rank = index + 1;
            *scores.entry(id).or_default() += 1.0 / (RRF_K + rank) as f64;
            best_ranks
                .entry(id)
                .and_modify(|best_rank| *best_rank = (*best_rank).min(rank))
                .or_insert(rank);
        }
    }

    let mut candidates = scores.into_iter().collect::<Vec<_>>();
    candidates.sort_by(|(left_id, left_score), (right_id, right_score)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| best_ranks[left_id].cmp(&best_ranks[right_id]))
            .then_with(|| left_id.cmp(right_id))
    });
    candidates
        .into_iter()
        .take(limit)
        .map(|(id, _)| id)
        .collect()
}

#[cfg(test)]
#[path = "rrf_lexical_char_tests.rs"]
mod tests;
