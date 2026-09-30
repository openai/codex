//! Shadow-only skill ranking. Capture turn inputs before recording new relevance signals,
//! then evaluate on a blocking worker without delaying model input construction.
//! This experiment is temporary and should be removed after evaluation.

mod task_context;
use task_context::TaskContextSnapshot;

pub(crate) use task_context::ShadowTaskContext;

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Duration;
use std::time::Instant;

use crate::HostSkillsSnapshot;
use codex_extension_api::TurnInputContext;
use codex_otel::MetricsClient;
use codex_protocol::user_input::UserInput;
use tokio::sync::Semaphore;

use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillSourceKind;
use crate::dynamic_skill_selector::CharacterNgramSkillSelector;
use crate::dynamic_skill_selector::CharacterRoutingCardSkillSelector;
use crate::dynamic_skill_selector::CheapSkillSelection;
use crate::dynamic_skill_selector::CheapSkillSelector;
use crate::dynamic_skill_selector::FieldedBm25SkillSelector;
use crate::dynamic_skill_selector::LruPlusCharacterRoutingSkillSelector;
use crate::dynamic_skill_selector::LruPlusLexicalCharacterRoutingSkillSelector;
use crate::dynamic_skill_selector::LruPlusLexicalSkillSelector;
use crate::dynamic_skill_selector::LruSkillSelector;
use crate::dynamic_skill_selector::MultiQueryLexicalSkillSelector;
use crate::dynamic_skill_selector::RoutingCardLexicalSkillSelector;
use crate::dynamic_skill_selector::RrfLexicalCharSkillSelector;
use crate::dynamic_skill_selector::SkillSelectionDocument;
use crate::dynamic_skill_selector::WeightedLexicalSkillSelector;

const MAX_SHADOW_QUERY_BYTES: usize = 16 * 1024;
const MAX_SHADOW_RESULTS: usize = 50;

// Share the CPU budget across threads and extension registries. Admission happens
// only in the background; turn preparation never waits for a ranking slot.
static RANKING_SLOTS: Semaphore = Semaphore::const_new(2);

const RUN_METRIC: &str = "codex.skills.shadow_selection";
const DURATION_METRIC: &str = "codex.skills.shadow_selection.duration_ms";
const CATALOG_ENTRY_COUNT_METRIC: &str = "codex.skills.shadow_selection.catalog_entries";
const SELECTED_ENTRY_COUNT_METRIC: &str = "codex.skills.shadow_selection.selected_entries";
const QUERY_TERM_COUNT_METRIC: &str = "codex.skills.shadow_selection.query_terms";
const REDUCTION_BPS_METRIC: &str = "codex.skills.shadow_selection.reduction_bps";

pub(crate) struct ShadowSelectionExperiment {
    selectors: Vec<Box<dyn CheapSkillSelector>>,
    metrics_client: Option<MetricsClient>,
}

impl ShadowSelectionExperiment {
    pub(crate) fn new(metrics_client: Option<MetricsClient>) -> Self {
        Self {
            selectors: vec![
                Box::new(WeightedLexicalSkillSelector),
                Box::new(FieldedBm25SkillSelector),
                Box::new(CharacterNgramSkillSelector),
                Box::new(MultiQueryLexicalSkillSelector),
                Box::new(RrfLexicalCharSkillSelector),
                Box::new(RoutingCardLexicalSkillSelector),
            ],
            metrics_client,
        }
    }

    /// Freeze prediction inputs before this turn can contribute new invocation history.
    /// The returned state observes invocations even while its rankings are still pending.
    pub(crate) fn start(
        self: &Arc<Self>,
        input: &TurnInputContext<'_>,
        catalog: SkillCatalog,
        explicitly_selected: &[SkillCatalogEntry],
        host_snapshot: Option<Arc<HostSkillsSnapshot>>,
        recent_skill_invocations: Arc<RecentSkillInvocations>,
        task_context: Arc<ShadowTaskContext>,
    ) -> ShadowSelectionTurnState {
        let query = build_shadow_query(&input.user_input);
        let task_snapshot = task_context.begin_turn(&input.turn_id, &query, &input.user_input);
        let recent_skill_resources = recent_skill_invocations.snapshot();
        let explicitly_selected_skill_resources = explicitly_selected
            .iter()
            .map(|entry| normalize_skill_resource(entry.main_prompt.as_str()))
            .collect::<HashSet<_>>();
        let mut eligible_ids = HashSet::new();
        let mut eligible_skill_resources = HashSet::new();
        for (id, entry) in catalog.entries.iter().enumerate() {
            // Invocation observation exists only for host shell use and cloud reads.
            if !entry.is_model_visible()
                || !matches!(
                    &entry.authority.kind,
                    SkillSourceKind::Host | SkillSourceKind::Cloud
                )
            {
                continue;
            }
            let resource = normalize_skill_resource(entry.main_prompt.as_str());
            if !explicitly_selected_skill_resources.contains(&resource) {
                eligible_ids.insert(id);
                eligible_skill_resources.insert(resource);
            }
        }
        let (invocations, pending_invocations) = tokio::sync::mpsc::unbounded_channel();
        let state = ShadowSelectionTurnState {
            turn_id: input.turn_id.clone(),
            eligible_skill_resources,
            seen_skill_resources: Mutex::new(HashSet::new()),
            recent_skill_invocations,
            task_context: Arc::clone(&task_context),
            invocations,
        };

        // Explicit intent is a relevance signal even if the subsequent prompt read fails.
        // It can update future turns now that this turn's prediction inputs are frozen.
        for entry in explicitly_selected.iter().filter(|entry| {
            entry.is_model_visible()
                && matches!(
                    &entry.authority.kind,
                    SkillSourceKind::Host | SkillSourceKind::Cloud
                )
        }) {
            task_context.record(
                &input.turn_id,
                normalize_skill_resource(entry.main_prompt.as_str()),
            );
        }

        let selection_input = ShadowSelectionInput {
            catalog,
            host_snapshot,
            query,
            eligible_ids,
            recent_skill_resources,
            task_snapshot,
        };
        tokio::spawn(Arc::clone(self).evaluate(
            selection_input,
            pending_invocations,
            tracing::Span::current(),
        ));
        state
    }

    /// Own the rankings and drain observations for this turn, including those queued
    /// during ranking. Dropping the turn's observer closes the channel after its last read.
    async fn evaluate(
        self: Arc<Self>,
        input: ShadowSelectionInput,
        mut invocations: tokio::sync::mpsc::UnboundedReceiver<String>,
        parent: tracing::Span,
    ) {
        let query_script = query_script_tag(&input.query.text);
        let experiment = Arc::clone(&self);
        let Ok(permit) = RANKING_SLOTS.acquire().await else {
            return;
        };
        let ranked_selections = match tokio::task::spawn_blocking(move || {
            // A cancelled async task must not release capacity while its blocking
            // worker is still running. Release it before waiting for invocations.
            let _permit = permit;
            experiment.rank(input, parent)
        })
        .await
        {
            Ok(rankings) => rankings,
            Err(error) => {
                tracing::warn!(%error, "shadow skill selection failed");
                return;
            }
        };
        let Some(metrics_client) = self.metrics_client.as_ref() else {
            return;
        };
        while let Some(skill_resource) = invocations.recv().await {
            for selection in &ranked_selections {
                let rank = selection
                    .skill_resources
                    .iter()
                    .position(|candidate| candidate == &skill_resource)
                    .map(|index| index + 1);
                let tags = [
                    ("method", selection.method),
                    ("hit", bool_tag(rank.is_some())),
                    ("rank", rank_bucket(rank)),
                    ("query_script", query_script),
                ];
                let _ = metrics_client.counter(
                    "codex.skills.shadow_selection.invocation",
                    /*inc*/ 1,
                    &tags,
                );
            }
        }
    }

    #[tracing::instrument(
        name = "skills.shadow_selection.rank",
        level = "info",
        parent = &parent,
        skip_all,
        fields(catalog_entries = input.eligible_ids.len())
    )]
    fn rank(&self, input: ShadowSelectionInput, parent: tracing::Span) -> Vec<RankedSelection> {
        let ShadowSelectionInput {
            catalog,
            host_snapshot,
            query,
            eligible_ids,
            recent_skill_resources,
            task_snapshot,
        } = input;
        let documents = catalog
            .entries
            .iter()
            .enumerate()
            .filter(|(id, _)| eligible_ids.contains(id))
            .map(|(id, entry)| SkillSelectionDocument {
                id,
                name: entry.name.as_str(),
                short_description: entry.short_description.as_deref(),
                description: entry.description.as_str(),
                dependencies: entry.dependencies.as_ref(),
            })
            .collect::<Vec<_>>();
        let eligible_skill_ids_by_resource = documents
            .iter()
            .map(|document| {
                (
                    normalize_skill_resource(catalog.entries[document.id].main_prompt.as_str()),
                    document.id,
                )
            })
            .collect::<HashMap<_, _>>();
        let recent_skill_ids = recent_skill_resources
            .iter()
            .filter_map(|resource| eligible_skill_ids_by_resource.get(resource).copied())
            .collect();
        let routing_selector =
            CharacterRoutingCardSkillSelector::new(&catalog, host_snapshot.as_deref());
        let lru_selector = LruSkillSelector::new(recent_skill_ids);
        let lru_plus_lexical_selector = LruPlusLexicalSkillSelector::new(lru_selector.clone());
        let lru_plus_character_selector = LruPlusCharacterRoutingSkillSelector::new(
            lru_selector.clone(),
            routing_selector.clone(),
        );
        let lru_plus_lexical_character_selector = LruPlusLexicalCharacterRoutingSkillSelector::new(
            lru_selector.clone(),
            routing_selector.clone(),
        );
        let task_selector = LruPlusLexicalCharacterRoutingSkillSelector::new(
            LruSkillSelector::new(
                task_snapshot
                    .recent_skills
                    .iter()
                    .filter_map(|resource| eligible_skill_ids_by_resource.get(resource).copied())
                    .collect(),
            ),
            routing_selector.clone(),
        );
        let mut ranked_selections = Vec::with_capacity(self.selectors.len() + 6);

        for (method, selector, query) in self
            .selectors
            .iter()
            .map(std::convert::AsRef::as_ref)
            .chain([
                &routing_selector as &dyn CheapSkillSelector,
                &lru_selector as &dyn CheapSkillSelector,
                &lru_plus_lexical_selector as &dyn CheapSkillSelector,
                &lru_plus_character_selector as &dyn CheapSkillSelector,
                &lru_plus_lexical_character_selector as &dyn CheapSkillSelector,
            ])
            .map(|selector| (selector.method(), selector, &query))
            .chain([(
                "task_context_fusion_v1",
                &task_selector as &dyn CheapSkillSelector,
                &task_snapshot.query,
            )])
        {
            let query_script = query_script_tag(&query.text);
            let start = Instant::now();
            let selection =
                selector.select(&query.text, &documents, /*limit*/ MAX_SHADOW_RESULTS);
            let duration = start.elapsed();
            let selected_ids = sanitize_selected_ids(&selection, &eligible_ids);
            self.record_metrics(ShadowSelectionObservation {
                method,
                selection: &selection,
                query_truncated_before_selection: query.truncated,
                query_script,
                catalog_entry_count: documents.len(),
                selected_entry_count: selected_ids.len(),
                duration,
            });
            ranked_selections.push(RankedSelection {
                method,
                skill_resources: selected_ids
                    .iter()
                    .map(|id| normalize_skill_resource(catalog.entries[*id].main_prompt.as_str()))
                    .collect(),
            });
            tracing::debug!(
                method,
                catalog_entries = documents.len(),
                selected_entries = selected_ids.len(),
                query_terms = selection.query_term_count,
                query_script,
                query_truncated = query.truncated || selection.query_truncated,
                candidate_set_truncated = selection.candidate_set_truncated,
                "ran shadow skill selection"
            );
        }

        ranked_selections
    }

    fn record_metrics(&self, observation: ShadowSelectionObservation<'_>) {
        let Some(metrics_client) = self.metrics_client.as_ref() else {
            return;
        };
        let ShadowSelectionObservation {
            method,
            selection,
            query_truncated_before_selection,
            query_script,
            catalog_entry_count,
            selected_entry_count,
            duration,
        } = observation;
        let status = selection_status(selection, selected_entry_count);
        let query_truncated =
            bool_tag(query_truncated_before_selection || selection.query_truncated);
        let candidate_set_truncated = bool_tag(selection.candidate_set_truncated);
        let tags = [
            ("method", method),
            ("status", status),
            ("query_script", query_script),
            ("query_truncated", query_truncated),
            ("candidate_set_truncated", candidate_set_truncated),
        ];
        let _ = metrics_client.counter(RUN_METRIC, /*inc*/ 1, &tags);
        let _ = metrics_client.record_duration(DURATION_METRIC, duration, &tags);
        let _ = metrics_client.histogram(
            CATALOG_ENTRY_COUNT_METRIC,
            metric_value(catalog_entry_count),
            &tags,
        );
        let _ = metrics_client.histogram(
            SELECTED_ENTRY_COUNT_METRIC,
            metric_value(selected_entry_count),
            &tags,
        );
        let _ = metrics_client.histogram(
            QUERY_TERM_COUNT_METRIC,
            metric_value(selection.query_term_count),
            &tags,
        );
        let _ = metrics_client.histogram(
            REDUCTION_BPS_METRIC,
            reduction_bps(catalog_entry_count, selected_entry_count),
            &tags,
        );
    }
}

pub(crate) struct ShadowSelectionTurnState {
    turn_id: String,
    eligible_skill_resources: HashSet<String>,
    seen_skill_resources: Mutex<HashSet<String>>,
    recent_skill_invocations: Arc<RecentSkillInvocations>,
    task_context: Arc<ShadowTaskContext>,
    invocations: tokio::sync::mpsc::UnboundedSender<String>,
}

impl ShadowSelectionTurnState {
    /// Advance relevance history immediately; scoring can wait for the frozen rankings.
    pub(crate) fn record_invocation(&self, skill_resource: &str) {
        let skill_resource = normalize_skill_resource(skill_resource);
        if !self.eligible_skill_resources.contains(&skill_resource)
            || !self
                .seen_skill_resources
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(skill_resource.clone())
        {
            return;
        }
        self.recent_skill_invocations.record(skill_resource.clone());
        self.task_context
            .record(&self.turn_id, skill_resource.clone());
        let _ = self.invocations.send(skill_resource);
    }
}

/// Owned turn-start inputs; workers never read the evolving relevance histories.
struct ShadowSelectionInput {
    catalog: SkillCatalog,
    host_snapshot: Option<Arc<HostSkillsSnapshot>>,
    query: ShadowQuery,
    eligible_ids: HashSet<usize>,
    recent_skill_resources: Vec<String>,
    task_snapshot: TaskContextSnapshot,
}

#[derive(Default)]
pub(crate) struct RecentSkillInvocations {
    skill_resources: Mutex<VecDeque<String>>,
}

impl RecentSkillInvocations {
    fn snapshot(&self) -> Vec<String> {
        self.skill_resources
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }

    fn record(&self, skill_resource: String) {
        let mut skill_resources = self
            .skill_resources
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(index) = skill_resources
            .iter()
            .position(|resource| resource == &skill_resource)
        {
            skill_resources.remove(index);
        }
        skill_resources.push_front(skill_resource);
        skill_resources.truncate(MAX_SHADOW_RESULTS);
    }
}

struct RankedSelection {
    method: &'static str,
    skill_resources: Vec<String>,
}

struct ShadowSelectionObservation<'a> {
    method: &'static str,
    selection: &'a CheapSkillSelection,
    query_truncated_before_selection: bool,
    query_script: &'static str,
    catalog_entry_count: usize,
    selected_entry_count: usize,
    duration: Duration,
}

fn sanitize_selected_ids(
    selection: &CheapSkillSelection,
    eligible_ids: &HashSet<usize>,
) -> Vec<usize> {
    let mut seen = HashSet::new();
    selection
        .candidate_ids
        .iter()
        .copied()
        .filter(|id| eligible_ids.contains(id) && seen.insert(*id))
        .take(MAX_SHADOW_RESULTS)
        .collect()
}

fn selection_status(selection: &CheapSkillSelection, selected_entry_count: usize) -> &'static str {
    if selected_entry_count > 0 {
        "selected"
    } else if selection.query_term_count == 0 {
        "no_query_terms"
    } else {
        "no_matches"
    }
}

fn reduction_bps(catalog_entry_count: usize, selected_entry_count: usize) -> i64 {
    if catalog_entry_count == 0 {
        return 0;
    }
    10_000i64.saturating_sub(ratio_bps(selected_entry_count, catalog_entry_count))
}

fn ratio_bps(numerator: usize, denominator: usize) -> i64 {
    if denominator == 0 {
        return 0;
    }
    let numerator = u128::try_from(numerator).unwrap_or(u128::MAX);
    let denominator = u128::try_from(denominator).unwrap_or(u128::MAX);
    let basis_points = numerator.saturating_mul(10_000) / denominator;
    i64::try_from(basis_points).unwrap_or(i64::MAX)
}

fn metric_value(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn bool_tag(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn rank_bucket(rank: Option<usize>) -> &'static str {
    match rank {
        Some(1) => "1",
        Some(2..=5) => "2_5",
        Some(6..=10) => "6_10",
        Some(11..=20) => "11_20",
        Some(21..=MAX_SHADOW_RESULTS) => "21_50",
        Some(_) | None => "miss",
    }
}

fn normalize_skill_resource(skill_resource: &str) -> String {
    skill_resource.replace('\\', "/")
}

fn query_script_tag(query: &str) -> &'static str {
    let mut has_ascii_latin = false;
    let mut has_cjk = false;
    let mut has_other = false;

    for character in query.chars().filter(|character| character.is_alphabetic()) {
        if character.is_ascii_alphabetic() {
            has_ascii_latin = true;
        } else if is_cjk(character) {
            has_cjk = true;
        } else {
            has_other = true;
        }
    }

    match (has_ascii_latin, has_cjk, has_other) {
        (false, false, false) => "none",
        (true, false, false) => "ascii_latin",
        (false, true, false) => "cjk",
        (false, false, true) => "other",
        (true, true, false) | (true, false, true) | (false, true, true) | (true, true, true) => {
            "mixed"
        }
    }
}

fn is_cjk(character: char) -> bool {
    matches!(
        character,
        '\u{1100}'..='\u{11ff}'
            | '\u{3040}'..='\u{30ff}'
            | '\u{3100}'..='\u{312f}'
            | '\u{3130}'..='\u{318f}'
            | '\u{31a0}'..='\u{31bf}'
            | '\u{31f0}'..='\u{31ff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{a960}'..='\u{a97f}'
            | '\u{ac00}'..='\u{d7af}'
            | '\u{d7b0}'..='\u{d7ff}'
            | '\u{f900}'..='\u{faff}'
            | '\u{20000}'..='\u{2fa1f}'
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ShadowQuery {
    text: String,
    truncated: bool,
}

fn build_shadow_query(inputs: &[UserInput]) -> ShadowQuery {
    let mut text = String::new();
    let mut truncated = false;
    for input in inputs {
        let part = match input {
            UserInput::Text { text, .. } => text.as_str(),
            UserInput::Skill { name, .. } | UserInput::Mention { name, .. } => name.as_str(),
            _ => continue,
        };
        if part.is_empty() {
            continue;
        }
        if !text.is_empty() && !push_bounded(&mut text, " ") {
            truncated = true;
            break;
        }
        if !push_bounded(&mut text, part) {
            truncated = true;
            break;
        }
    }
    ShadowQuery { text, truncated }
}

fn push_bounded(destination: &mut String, value: &str) -> bool {
    let remaining = MAX_SHADOW_QUERY_BYTES.saturating_sub(destination.len());
    if value.len() <= remaining {
        destination.push_str(value);
        return true;
    }
    let mut end = remaining;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    destination.push_str(&value[..end]);
    false
}

#[cfg(test)]
#[path = "experiment_tests.rs"]
mod tests;
