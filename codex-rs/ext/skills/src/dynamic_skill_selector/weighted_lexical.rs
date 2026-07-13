use std::collections::HashSet;

use super::CheapSkillSelection;
use super::CheapSkillSelector;
use super::SkillSelectionDocument;

const MAX_QUERY_BYTES: usize = 4 * 1024;
const MAX_QUERY_TERMS: usize = 64;
const MAX_DOCUMENT_BYTES: usize = 4 * 1024;
const MAX_DOCUMENT_TERMS: usize = 256;
const MAX_CANDIDATES: usize = 1_000;
const MAX_RESULTS: usize = 50;

const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "do", "for", "from", "how", "i", "in", "is",
    "it", "me", "my", "of", "on", "or", "please", "that", "the", "this", "to", "use", "we", "what",
    "when", "where", "which", "with", "you", "your",
];

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct WeightedLexicalSkillSelector;

impl CheapSkillSelector for WeightedLexicalSkillSelector {
    fn method(&self) -> &'static str {
        "weighted_lexical_v1"
    }

    fn select(
        &self,
        query: &str,
        documents: &[SkillSelectionDocument<'_>],
        limit: usize,
    ) -> CheapSkillSelection {
        let (query, query_bytes_truncated) = bounded(query, MAX_QUERY_BYTES);
        let query_phrase = normalize_phrase(query);
        let (query_terms, query_terms_truncated) = query_terms(&query_phrase);
        let query_truncated = query_bytes_truncated || query_terms_truncated;
        let candidate_set_truncated = documents.len() > MAX_CANDIDATES;
        if query_terms.is_empty() || limit == 0 {
            return CheapSkillSelection {
                query_term_count: query_terms.len(),
                query_truncated,
                candidate_set_truncated,
                ..Default::default()
            };
        }

        let mut scored = documents
            .iter()
            .take(MAX_CANDIDATES)
            .filter_map(|document| {
                let score = score_document(&query_phrase, &query_terms, document);
                (score > 0).then_some((score, document.id, document.name))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.2.cmp(right.2))
                .then_with(|| left.1.cmp(&right.1))
        });

        CheapSkillSelection {
            candidate_ids: scored
                .into_iter()
                .take(limit.min(MAX_RESULTS))
                .map(|(_, id, _)| id)
                .collect(),
            query_term_count: query_terms.len(),
            query_truncated,
            candidate_set_truncated,
        }
    }
}

fn score_document(
    query_phrase: &str,
    query_terms: &[&str],
    document: &SkillSelectionDocument<'_>,
) -> u32 {
    let name = normalize_bounded(document.name);
    let short_description = document
        .short_description
        .map(normalize_bounded)
        .unwrap_or_default();
    let description = normalize_bounded(document.description);
    let name_terms = phrase_terms(&name);
    let short_description_terms = phrase_terms(&short_description);
    let description_terms = phrase_terms(&description);

    let mut score = 0u32;
    if !name.is_empty() && contains_phrase(query_phrase, &name) {
        score = score.saturating_add(256);
    }

    let mut matched_query_terms = 0u32;
    for query_term in query_terms {
        let mut matched = false;
        if name == *query_term {
            score = score.saturating_add(128);
            matched = true;
        } else if name_terms.contains(query_term) {
            score = score.saturating_add(64);
            matched = true;
        } else if contains_related_term(&name_terms, query_term) {
            score = score.saturating_add(24);
            matched = true;
        }

        if short_description_terms.contains(query_term) {
            score = score.saturating_add(16);
            matched = true;
        } else if contains_related_term(&short_description_terms, query_term) {
            score = score.saturating_add(6);
            matched = true;
        }

        if description_terms.contains(query_term) {
            score = score.saturating_add(4);
            matched = true;
        } else if contains_related_term(&description_terms, query_term) {
            score = score.saturating_add(1);
            matched = true;
        }

        if matched {
            matched_query_terms = matched_query_terms.saturating_add(1);
        }
    }

    score.saturating_add(matched_query_terms.saturating_mul(matched_query_terms))
}

fn normalize_bounded(value: &str) -> String {
    normalize_phrase(bounded(value, MAX_DOCUMENT_BYTES).0)
}

fn normalize_phrase(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_was_separator = true;
    for character in value.chars() {
        if character.is_alphanumeric() {
            normalized.extend(character.to_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            normalized.push(' ');
            previous_was_separator = true;
        }
    }
    if previous_was_separator {
        normalized.pop();
    }
    normalized
}

fn query_terms(query_phrase: &str) -> (Vec<&str>, bool) {
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    for term in query_phrase
        .split_whitespace()
        .filter(|term| term.chars().count() >= 2 && !STOP_WORDS.contains(term))
    {
        if !seen.insert(term) {
            continue;
        }
        if terms.len() == MAX_QUERY_TERMS {
            return (terms, true);
        }
        terms.push(term);
    }
    (terms, false)
}

fn phrase_terms(phrase: &str) -> HashSet<&str> {
    phrase.split_whitespace().take(MAX_DOCUMENT_TERMS).collect()
}

fn contains_phrase(haystack: &str, needle: &str) -> bool {
    haystack == needle
        || haystack.starts_with(&format!("{needle} "))
        || haystack.ends_with(&format!(" {needle}"))
        || haystack.contains(&format!(" {needle} "))
}

fn contains_related_term(terms: &HashSet<&str>, query_term: &str) -> bool {
    if query_term.chars().count() < 4 {
        return false;
    }
    terms.iter().any(|term| {
        term.chars().count() >= 4 && (term.starts_with(query_term) || query_term.starts_with(*term))
    })
}

fn bounded(value: &str, max_bytes: usize) -> (&str, bool) {
    if value.len() <= max_bytes {
        return (value, false);
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    (&value[..end], true)
}

#[cfg(test)]
#[path = "weighted_lexical_tests.rs"]
mod tests;
