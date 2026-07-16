use std::collections::HashMap;
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
const FIELD_WEIGHTS: [f64; 3] = [8.0, 4.0, 1.0];
const K1: f64 = 1.2;
const B: f64 = 0.75;

const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "do", "for", "from", "how", "i", "in", "is",
    "it", "me", "my", "of", "on", "or", "please", "that", "the", "this", "to", "use", "we", "what",
    "when", "where", "which", "with", "you", "your",
];

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FieldedBm25SkillSelector;

impl CheapSkillSelector for FieldedBm25SkillSelector {
    fn method(&self) -> &'static str {
        "fielded_bm25_v1"
    }

    fn select(
        &self,
        query: &str,
        documents: &[SkillSelectionDocument<'_>],
        limit: usize,
    ) -> CheapSkillSelection {
        let (query, query_bytes_truncated) = bounded(query, MAX_QUERY_BYTES);
        let (query_terms, query_terms_truncated) = query_terms(query);
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

        let prepared = documents
            .iter()
            .take(MAX_CANDIDATES)
            .map(PreparedDocument::new)
            .collect::<Vec<_>>();
        let averages = average_field_lengths(&prepared);
        let document_frequencies = document_frequencies(&prepared);
        let document_count = prepared.len() as f64;
        let mut scored = prepared
            .iter()
            .filter_map(|document| {
                let score = score_document(
                    document,
                    &query_terms,
                    &document_frequencies,
                    document_count,
                    averages,
                );
                (score > 0.0).then_some((score, document.id, document.name))
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
    fields: [Vec<String>; 3],
}

impl<'a> PreparedDocument<'a> {
    fn new(document: &'a SkillSelectionDocument<'a>) -> Self {
        Self {
            id: document.id,
            name: document.name,
            fields: [
                document_terms(document.name),
                document_terms(document.short_description.unwrap_or_default()),
                document_terms(document.description),
            ],
        }
    }
}

fn score_document(
    document: &PreparedDocument<'_>,
    query_terms: &[String],
    document_frequencies: &HashMap<String, usize>,
    document_count: f64,
    average_field_lengths: [f64; 3],
) -> f64 {
    query_terms.iter().fold(0.0, |score, query_term| {
        let frequency = document_frequencies
            .get(query_term)
            .copied()
            .unwrap_or_default() as f64;
        if frequency == 0.0 {
            return score;
        }
        let weighted_term_frequency =
            document
                .fields
                .iter()
                .enumerate()
                .fold(0.0, |weighted, (field_index, terms)| {
                    let term_frequency =
                        terms.iter().filter(|term| *term == query_term).count() as f64;
                    if term_frequency == 0.0 {
                        return weighted;
                    }
                    let average_length = average_field_lengths[field_index];
                    let length_ratio = if average_length == 0.0 {
                        1.0
                    } else {
                        terms.len() as f64 / average_length
                    };
                    weighted
                        + FIELD_WEIGHTS[field_index] * term_frequency / (1.0 - B + B * length_ratio)
                });
        if weighted_term_frequency == 0.0 {
            return score;
        }
        let inverse_document_frequency =
            (1.0 + (document_count - frequency + 0.5) / (frequency + 0.5)).ln();
        score
            + inverse_document_frequency * weighted_term_frequency * (K1 + 1.0)
                / (weighted_term_frequency + K1)
    })
}

fn average_field_lengths(documents: &[PreparedDocument<'_>]) -> [f64; 3] {
    if documents.is_empty() {
        return [0.0; 3];
    }
    let totals = documents.iter().fold([0usize; 3], |mut totals, document| {
        for (index, field) in document.fields.iter().enumerate() {
            totals[index] = totals[index].saturating_add(field.len());
        }
        totals
    });
    totals.map(|total| total as f64 / documents.len() as f64)
}

fn document_frequencies(documents: &[PreparedDocument<'_>]) -> HashMap<String, usize> {
    let mut frequencies = HashMap::new();
    for document in documents {
        let terms = document
            .fields
            .iter()
            .flatten()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        for term in terms {
            *frequencies.entry(term.to_string()).or_default() += 1;
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

fn document_terms(value: &str) -> Vec<String> {
    let (value, _) = bounded(value, MAX_DOCUMENT_BYTES);
    normalized_terms(value)
        .into_iter()
        .take(MAX_DOCUMENT_TERMS)
        .collect()
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
#[path = "fielded_bm25_tests.rs"]
mod tests;
