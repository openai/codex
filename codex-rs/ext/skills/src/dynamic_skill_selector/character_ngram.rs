use std::collections::HashMap;
use std::collections::HashSet;

use super::CheapSkillSelection;
use super::CheapSkillSelector;
use super::SkillSelectionDocument;

const MAX_QUERY_BYTES: usize = 4 * 1024;
const MAX_QUERY_TERMS: usize = 64;
const MAX_QUERY_GRAMS: usize = 512;
const MAX_DOCUMENT_BYTES: usize = 4 * 1024;
const MAX_DOCUMENT_TERMS: usize = 256;
const MAX_DOCUMENT_GRAMS: usize = 512;
const MAX_CANDIDATES: usize = 1_000;
const MAX_RESULTS: usize = 50;
const MIN_GRAM_CHARS: usize = 2;
const MAX_GRAM_CHARS: usize = 5;
const FIELD_WEIGHTS: [f64; 3] = [8.0, 4.0, 1.0];

const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "do", "for", "from", "how", "i", "in", "is",
    "it", "me", "my", "of", "on", "or", "please", "that", "the", "this", "to", "use", "we", "what",
    "when", "where", "which", "with", "you", "your",
];

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CharacterNgramSkillSelector;

impl CheapSkillSelector for CharacterNgramSkillSelector {
    fn method(&self) -> &'static str {
        "character_ngram_v1"
    }

    fn select(
        &self,
        query: &str,
        documents: &[SkillSelectionDocument<'_>],
        limit: usize,
    ) -> CheapSkillSelection {
        let (query, query_bytes_truncated) = bounded(query, MAX_QUERY_BYTES);
        let (query_terms, query_terms_truncated) = query_terms(query);
        let (query_grams, query_grams_truncated) = grams(&query_terms, MAX_QUERY_GRAMS);
        let query_truncated =
            query_bytes_truncated || query_terms_truncated || query_grams_truncated;
        let candidate_set_truncated = documents.len() > MAX_CANDIDATES;
        if query_grams.is_empty() || limit == 0 {
            return CheapSkillSelection {
                query_term_count: query_terms.len(),
                query_truncated,
                candidate_set_truncated,
                ..Default::default()
            };
        }

        let prepared = documents
            .iter()
            .take(MAX_CANDIDATES)
            .map(PreparedDocument::new)
            .collect::<Vec<_>>();
        let document_frequencies = document_frequencies(&prepared);
        let document_count = prepared.len() as f64;
        let minimum_matches = query_grams.len().min(3);
        let mut scored = prepared
            .iter()
            .filter_map(|document| {
                let (score, matched_grams) = score_document(
                    document,
                    &query_grams,
                    &document_frequencies,
                    document_count,
                );
                (matched_grams >= minimum_matches).then_some((score, document.id, document.name))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| {
            right
                .0
                .total_cmp(&left.0)
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

struct PreparedDocument<'a> {
    id: usize,
    name: &'a str,
    fields: [HashSet<String>; 3],
}

impl<'a> PreparedDocument<'a> {
    fn new(document: &'a SkillSelectionDocument<'a>) -> Self {
        Self {
            id: document.id,
            name: document.name,
            fields: [
                document_grams(document.name),
                document_grams(document.short_description.unwrap_or_default()),
                document_grams(document.description),
            ],
        }
    }
}

fn score_document(
    document: &PreparedDocument<'_>,
    query_grams: &[String],
    document_frequencies: &HashMap<String, usize>,
    document_count: f64,
) -> (f64, usize) {
    query_grams.iter().fold((0.0, 0), |(score, matched), gram| {
        let frequency = document_frequencies.get(gram).copied().unwrap_or_default() as f64;
        if frequency == 0.0 {
            return (score, matched);
        }
        let inverse_document_frequency =
            (1.0 + (document_count - frequency + 0.5) / (frequency + 0.5)).ln();
        let field_weight = document
            .fields
            .iter()
            .enumerate()
            .filter(|(_, field)| field.contains(gram))
            .map(|(index, _)| FIELD_WEIGHTS[index])
            .sum::<f64>();
        if field_weight == 0.0 {
            (score, matched)
        } else {
            (
                score + inverse_document_frequency * field_weight,
                matched + 1,
            )
        }
    })
}

fn document_frequencies(documents: &[PreparedDocument<'_>]) -> HashMap<String, usize> {
    let mut frequencies = HashMap::new();
    for document in documents {
        let grams = document
            .fields
            .iter()
            .flatten()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        for gram in grams {
            *frequencies.entry(gram.to_string()).or_default() += 1;
        }
    }
    frequencies
}

fn query_terms(query: &str) -> (Vec<String>, bool) {
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    for term in normalized_terms(query)
        .into_iter()
        .filter(|term| term.chars().count() >= 2 && !STOP_WORDS.contains(&term.as_str()))
    {
        if !seen.insert(term.clone()) {
            continue;
        }
        if terms.len() == MAX_QUERY_TERMS {
            return (terms, true);
        }
        terms.push(term);
    }
    (terms, false)
}

fn document_grams(value: &str) -> HashSet<String> {
    let (value, _) = bounded(value, MAX_DOCUMENT_BYTES);
    let terms = normalized_terms(value)
        .into_iter()
        .take(MAX_DOCUMENT_TERMS)
        .collect::<Vec<_>>();
    grams(&terms, MAX_DOCUMENT_GRAMS).0.into_iter().collect()
}

fn grams(terms: &[String], limit: usize) -> (Vec<String>, bool) {
    let mut seen = HashSet::new();
    let mut grams = Vec::new();
    for term in terms {
        let characters = term.chars().collect::<Vec<_>>();
        let minimum_gram_chars = if term.is_ascii() && characters.len() > MIN_GRAM_CHARS {
            MIN_GRAM_CHARS + 1
        } else {
            MIN_GRAM_CHARS
        };
        for gram_size in minimum_gram_chars..=MAX_GRAM_CHARS.min(characters.len()) {
            for start in 0..=characters.len() - gram_size {
                let gram = characters[start..start + gram_size]
                    .iter()
                    .collect::<String>();
                if !seen.insert(gram.clone()) {
                    continue;
                }
                if grams.len() == limit {
                    return (grams, true);
                }
                grams.push(gram);
            }
        }
    }
    (grams, false)
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
#[path = "character_ngram_tests.rs"]
mod tests;
