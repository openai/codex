mod character_ngram;
mod fielded_bm25;
mod multi_query_lexical;
mod rrf_lexical_char;
mod weighted_lexical;
pub(crate) use character_ngram::CharacterNgramSkillSelector;
pub(crate) use fielded_bm25::FieldedBm25SkillSelector;
pub(crate) use multi_query_lexical::MultiQueryLexicalSkillSelector;
pub(crate) use rrf_lexical_char::RrfLexicalCharSkillSelector;
pub(crate) use weighted_lexical::WeightedLexicalSkillSelector;

/// Metadata searched by a cheap skill selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SkillSelectionDocument<'a> {
    /// Caller-owned identifier returned in [`CheapSkillSelection::candidate_ids`].
    pub id: usize,
    pub name: &'a str,
    pub short_description: Option<&'a str>,
    pub description: &'a str,
}

/// Bounded output from one cheap skill-selection method.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CheapSkillSelection {
    pub candidate_ids: Vec<usize>,
    pub query_term_count: usize,
    pub query_truncated: bool,
    pub candidate_set_truncated: bool,
}

/// Selects likely-relevant skills without changing the model-visible catalog.
///
/// Implementations must be deterministic, side-effect free, and cheap enough to run in shadow
/// mode on every turn. Callers must validate returned IDs against the supplied documents.
pub(crate) trait CheapSkillSelector: Send + Sync {
    /// Low-cardinality identifier suitable for experiment metrics.
    fn method(&self) -> &'static str;

    fn select(
        &self,
        query: &str,
        documents: &[SkillSelectionDocument<'_>],
        limit: usize,
    ) -> CheapSkillSelection;
}
