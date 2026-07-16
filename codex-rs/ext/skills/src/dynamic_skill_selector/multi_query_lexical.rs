use std::collections::HashMap;
use std::collections::HashSet;

use super::CheapSkillSelection;
use super::CheapSkillSelector;
use super::SkillSelectionDocument;
use super::WeightedLexicalSkillSelector;

const MAX_QUERY_VIEWS: usize = 8;
const MAX_DECOMPOSITION_BYTES: usize = 4 * 1024;
const MAX_RESULTS: usize = 50;
const CONNECTORS: &[&str] = &[" and then ", " and ", " then ", " also "];

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MultiQueryLexicalSkillSelector;

impl CheapSkillSelector for MultiQueryLexicalSkillSelector {
    fn method(&self) -> &'static str {
        "multi_query_lexical_v1"
    }

    fn select(
        &self,
        query: &str,
        documents: &[SkillSelectionDocument<'_>],
        limit: usize,
    ) -> CheapSkillSelection {
        let full_selection =
            WeightedLexicalSkillSelector.select(query, documents, limit.min(MAX_RESULTS));
        let views = query_views(query);
        if views.len() <= 1 || limit == 0 {
            return full_selection;
        }

        let mut candidates = HashMap::new();
        record_candidates(&mut candidates, &full_selection, /*view_index*/ 0);
        let mut query_truncated = full_selection.query_truncated;
        let mut candidate_set_truncated = full_selection.candidate_set_truncated;
        for (view_index, view) in views.into_iter().enumerate().skip(1) {
            let selection =
                WeightedLexicalSkillSelector.select(view, documents, limit.min(MAX_RESULTS));
            query_truncated |= selection.query_truncated;
            candidate_set_truncated |= selection.candidate_set_truncated;
            record_candidates(&mut candidates, &selection, view_index);
        }

        let mut candidates = candidates.into_values().collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.best_rank
                .cmp(&right.best_rank)
                .then_with(|| {
                    left.full_query_rank
                        .unwrap_or(usize::MAX)
                        .cmp(&right.full_query_rank.unwrap_or(usize::MAX))
                })
                .then_with(|| right.view_count.cmp(&left.view_count))
                .then_with(|| left.first_view.cmp(&right.first_view))
                .then_with(|| left.id.cmp(&right.id))
        });

        CheapSkillSelection {
            candidate_ids: candidates
                .into_iter()
                .take(limit.min(MAX_RESULTS))
                .map(|candidate| candidate.id)
                .collect(),
            query_term_count: full_selection.query_term_count,
            query_truncated,
            candidate_set_truncated,
        }
    }
}

struct RankedCandidate {
    id: usize,
    best_rank: usize,
    full_query_rank: Option<usize>,
    view_count: usize,
    first_view: usize,
}

fn record_candidates(
    candidates: &mut HashMap<usize, RankedCandidate>,
    selection: &CheapSkillSelection,
    view_index: usize,
) {
    for (rank, id) in selection.candidate_ids.iter().copied().enumerate() {
        let rank = rank + 1;
        candidates
            .entry(id)
            .and_modify(|candidate| {
                candidate.best_rank = candidate.best_rank.min(rank);
                candidate.view_count = candidate.view_count.saturating_add(1);
                if view_index == 0 {
                    candidate.full_query_rank = Some(rank);
                }
            })
            .or_insert(RankedCandidate {
                id,
                best_rank: rank,
                full_query_rank: (view_index == 0).then_some(rank),
                view_count: 1,
                first_view: view_index,
            });
    }
}

fn query_views(query: &str) -> Vec<&str> {
    let full_query = bounded(query, MAX_DECOMPOSITION_BYTES).trim();
    if full_query.is_empty() {
        return Vec::new();
    }

    let mut views = vec![full_query];
    let mut seen = HashSet::from([full_query]);
    for sentence in full_query.split(['\n', '\r', '.', '!', '?', ';']) {
        for clause in split_connectors(sentence) {
            let clause = clause.trim();
            if clause.chars().count() >= 2 && seen.insert(clause) {
                views.push(clause);
                if views.len() == MAX_QUERY_VIEWS {
                    return views;
                }
            }
        }
    }
    views
}

fn bounded(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    &value[..end]
}

fn split_connectors(value: &str) -> Vec<&str> {
    let lowercase = value.to_ascii_lowercase();
    let mut segments = Vec::new();
    let mut start = 0;
    while start < value.len() {
        let next = CONNECTORS
            .iter()
            .filter_map(|connector| {
                lowercase[start..]
                    .find(connector)
                    .map(|offset| (start + offset, connector.len()))
            })
            .min_by_key(|(position, _)| *position);
        let Some((position, connector_length)) = next else {
            break;
        };
        segments.push(&value[start..position]);
        start = position + connector_length;
    }
    segments.push(&value[start..]);
    segments
}

#[cfg(test)]
#[path = "multi_query_lexical_tests.rs"]
mod tests;
