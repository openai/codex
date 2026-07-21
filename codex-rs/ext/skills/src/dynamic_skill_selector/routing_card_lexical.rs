use std::collections::HashSet;

use super::CheapSkillSelection;
use super::CheapSkillSelector;
use super::SkillSelectionDocument;

const MAX_QUERY_BYTES: usize = 4 * 1024;
const MAX_QUERY_TERMS: usize = 64;
const MAX_DOCUMENT_BYTES: usize = 4 * 1024;
const MAX_FIELD_TERMS: usize = 256;
const MAX_DEPENDENCY_RECORDS: usize = 32;
const MAX_CANDIDATES: usize = 1_000;
const MAX_RESULTS: usize = 50;

const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "do", "for", "from", "how", "i", "in", "is",
    "it", "me", "my", "of", "on", "or", "please", "that", "the", "this", "to", "use", "we", "what",
    "when", "where", "which", "with", "you", "your",
];

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RoutingCardLexicalSkillSelector;

impl CheapSkillSelector for RoutingCardLexicalSkillSelector {
    fn method(&self) -> &'static str {
        "routing_card_exact_v1"
    }

    fn select(
        &self,
        query: &str,
        documents: &[SkillSelectionDocument<'_>],
        limit: usize,
    ) -> CheapSkillSelection {
        let (query, query_bytes_truncated) = bounded(query, MAX_QUERY_BYTES);
        let query_phrase = normalize(query);
        let (query_terms, query_terms_truncated) = query_terms(&query_phrase);
        let query_truncated = query_bytes_truncated || query_terms_truncated;
        let candidate_set_truncated = documents.len() > MAX_CANDIDATES;
        if query_phrase.is_empty() || limit == 0 {
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
                let prepared = PreparedRoutingCard::new(document);
                let score = prepared.score(&query_phrase, &query_terms);
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

struct PreparedRoutingCard {
    name: String,
    name_has_searchable_term: bool,
    fields: RoutingFields,
}

struct RoutingFields {
    name: HashSet<String>,
    dependencies: HashSet<String>,
    short_description: HashSet<String>,
    description: HashSet<String>,
}

impl PreparedRoutingCard {
    fn new(document: &SkillSelectionDocument<'_>) -> Self {
        let name = normalize_bounded(document.name);
        Self {
            name_has_searchable_term: name.split_whitespace().any(is_searchable_query_term),
            name,
            fields: RoutingFields {
                name: field_terms(document.name),
                dependencies: dependency_terms(document),
                short_description: field_terms(document.short_description.unwrap_or_default()),
                description: field_terms(document.description),
            },
        }
    }

    fn score(&self, query_phrase: &str, query_terms: &[&str]) -> u32 {
        let mut score = 0u32;
        let name_phrase_matches = contains_phrase(query_phrase, &self.name);
        if !self.name.is_empty()
            && ((self.name_has_searchable_term && name_phrase_matches) || query_phrase == self.name)
        {
            score = score.saturating_add(256);
        }

        let mut matched_query_terms = 0u32;
        for query_term in query_terms {
            let mut matched = false;
            score = score.saturating_add(score_field(
                &self.fields.name,
                query_term,
                /*weight*/ 80,
                &mut matched,
            ));
            score = score.saturating_add(score_field(
                &self.fields.dependencies,
                query_term,
                /*weight*/ 24,
                &mut matched,
            ));
            score = score.saturating_add(score_field(
                &self.fields.short_description,
                query_term,
                /*weight*/ 12,
                &mut matched,
            ));
            score = score.saturating_add(score_field(
                &self.fields.description,
                query_term,
                /*weight*/ 3,
                &mut matched,
            ));
            if matched {
                matched_query_terms = matched_query_terms.saturating_add(1);
            }
        }
        score.saturating_add(matched_query_terms.saturating_mul(matched_query_terms))
    }
}

fn score_field(terms: &HashSet<String>, query_term: &str, weight: u32, matched: &mut bool) -> u32 {
    if terms.contains(query_term) {
        *matched = true;
        weight
    } else {
        0
    }
}

fn dependency_terms(document: &SkillSelectionDocument<'_>) -> HashSet<String> {
    let mut terms = HashSet::new();
    let Some(dependencies) = document.dependencies else {
        return terms;
    };
    let mut remaining_bytes = MAX_DOCUMENT_BYTES;
    for tool in dependencies.tools.iter().take(MAX_DEPENDENCY_RECORDS) {
        extend_terms(&mut terms, &tool.value, &mut remaining_bytes);
        if let Some(description) = tool.description.as_deref() {
            extend_terms(&mut terms, description, &mut remaining_bytes);
        }
        if remaining_bytes == 0 || terms.len() >= MAX_FIELD_TERMS {
            break;
        }
    }
    terms
}

fn field_terms(value: &str) -> HashSet<String> {
    normalized_terms(bounded(value, MAX_DOCUMENT_BYTES).0)
        .into_iter()
        .take(MAX_FIELD_TERMS)
        .collect()
}

fn extend_terms(terms: &mut HashSet<String>, value: &str, remaining_bytes: &mut usize) {
    let value = bounded(value, *remaining_bytes).0;
    *remaining_bytes = remaining_bytes.saturating_sub(value.len());
    for term in normalized_terms(value) {
        if terms.len() == MAX_FIELD_TERMS {
            return;
        }
        terms.insert(term);
    }
}

fn query_terms(query_phrase: &str) -> (Vec<&str>, bool) {
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    for term in query_phrase
        .split_whitespace()
        .filter(|term| is_searchable_query_term(term))
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

fn is_searchable_query_term(term: &str) -> bool {
    term.chars().count() >= 2 && !STOP_WORDS.contains(&term)
}

fn normalize_bounded(value: &str) -> String {
    normalize(bounded(value, MAX_DOCUMENT_BYTES).0)
}

fn normalize(value: &str) -> String {
    normalized_terms(value).join(" ")
}

fn normalized_terms(value: &str) -> Vec<String> {
    let mut normalized = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_alphanumeric() {
            normalized.extend(character.to_lowercase());
        } else {
            normalized.push(' ');
        }
    }
    normalized.split_whitespace().map(str::to_string).collect()
}

fn contains_phrase(haystack: &str, needle: &str) -> bool {
    haystack == needle
        || haystack.starts_with(&format!("{needle} "))
        || haystack.ends_with(&format!(" {needle}"))
        || haystack.contains(&format!(" {needle} "))
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
#[path = "routing_card_lexical_tests.rs"]
mod tests;
