//! Always-on, bounded tool-search diagnostics retained for immediate feedback.
//!
//! The recorder stores only compact identities and latest stage snapshots. It is
//! independent of tracing so the normal feedback subscriber cannot accidentally
//! turn on verbose payloads.

use codex_mcp::ToolInfo;
use codex_mcp::ToolSearchDiagnosticIdentity;
use codex_mcp::ToolSearchMcpDiagnosticsSnapshot;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_tools::LoadableToolSpec;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolSearchInfo;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::sync::Mutex;

const MAX_STAGE_IDENTITIES: usize = 256;
const MAX_STAGE_CHANGES: usize = 128;
const MAX_EXCLUDED_IDENTITIES: usize = 256;
const MAX_SEARCHES: usize = 32;
const MAX_SEARCH_RESULTS: usize = 100;
const MAX_IDENTITY_CHARS: usize = 160;
const MAX_QUERY_CHARS: usize = 256;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct DiagnosticToolIdentity {
    server_name: String,
    raw_tool_name: String,
    tool_name: Option<String>,
    connector_id: Option<String>,
    connector_name: Option<String>,
    plugin_display_names: Vec<String>,
}

impl DiagnosticToolIdentity {
    pub(crate) fn from_tool_info(tool: &ToolInfo) -> Self {
        Self::from_mcp_identity(ToolSearchDiagnosticIdentity::from_tool_info(tool))
    }

    fn from_mcp_identity(identity: ToolSearchDiagnosticIdentity) -> Self {
        let tool_name = match (identity.callable_namespace, identity.callable_name) {
            (Some(namespace), Some(name)) => Some(format!("{namespace}.{name}")),
            (None, None) => None,
            (Some(namespace), None) => Some(namespace),
            (None, Some(name)) => Some(name),
        };
        Self {
            server_name: bounded(identity.server_name, MAX_IDENTITY_CHARS),
            raw_tool_name: bounded(identity.raw_tool_name, MAX_IDENTITY_CHARS),
            tool_name: tool_name.map(|name| bounded(name, MAX_IDENTITY_CHARS)),
            connector_id: identity
                .connector_id
                .map(|connector_id| bounded(connector_id, MAX_IDENTITY_CHARS)),
            connector_name: identity
                .connector_name
                .map(|connector_name| bounded(connector_name, MAX_IDENTITY_CHARS)),
            plugin_display_names: identity
                .plugin_display_names
                .into_iter()
                .take(4)
                .map(|name| bounded(name, MAX_IDENTITY_CHARS))
                .collect(),
        }
    }

    pub(crate) fn from_search_info(search_info: &ToolSearchInfo) -> Self {
        Self {
            server_name: String::new(),
            raw_tool_name: String::new(),
            tool_name: Some(bounded(
                search_info_output_identity(search_info),
                MAX_IDENTITY_CHARS,
            )),
            connector_id: None,
            connector_name: search_info
                .source_info
                .as_ref()
                .map(|source| bounded(source.name.clone(), MAX_IDENTITY_CHARS)),
            plugin_display_names: Vec::new(),
        }
    }

    fn key(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.server_name,
            self.raw_tool_name,
            self.tool_name.as_deref().unwrap_or_default(),
            self.connector_id.as_deref().unwrap_or_default()
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ToolExposureDecision {
    pub(crate) identity: DiagnosticToolIdentity,
    pub(crate) included: bool,
    pub(crate) model_visible: bool,
    pub(crate) connector_id_present: bool,
    pub(crate) connector_allowed: bool,
    pub(crate) policy_enabled: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
struct StageMetadata {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    observation_sources: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cached_server_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    startup_complete_server_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    included_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    search_tool_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_state: Option<String>,
    #[serde(skip)]
    total_count_override: Option<usize>,
    #[serde(skip)]
    upstream_identities_truncated: bool,
    #[serde(skip)]
    fingerprint_override: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct StageSnapshot {
    stage: String,
    inventory_generation: String,
    fingerprint: String,
    total_count: usize,
    identities: Vec<DiagnosticToolIdentity>,
    identities_truncated: bool,
    added: Vec<String>,
    removed: Vec<String>,
    changes_truncated: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    excluded: Vec<ToolExposureDecision>,
    excluded_truncated: bool,
    #[serde(flatten)]
    metadata: StageMetadata,
    #[serde(skip_serializing)]
    retained_keys: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SearchResultDiagnostic {
    pub(crate) rank: usize,
    pub(crate) score: f32,
    pub(crate) document_index: usize,
    pub(crate) identity: DiagnosticToolIdentity,
}

#[derive(Clone, Debug, Serialize)]
struct SearchSnapshot {
    stage: &'static str,
    inventory_generation: String,
    query: String,
    requested_limit: usize,
    returned_count: usize,
    results: Vec<SearchResultDiagnostic>,
    results_truncated: bool,
}

#[derive(Default)]
struct RecorderState {
    inventory_generation: String,
    stages: BTreeMap<String, StageSnapshot>,
    identity_by_tool_name: BTreeMap<String, DiagnosticToolIdentity>,
    searches: VecDeque<SearchSnapshot>,
}

#[derive(Default)]
pub(crate) struct ToolSearchPipelineDiagnostics {
    state: Mutex<RecorderState>,
}

impl ToolSearchPipelineDiagnostics {
    pub(crate) fn record_mcp_snapshot(
        &self,
        mcp: &ToolSearchMcpDiagnosticsSnapshot,
        inventory: &[ToolInfo],
    ) {
        let raw_identities = mcp
            .tools_list_responses
            .iter()
            .flat_map(|response| response.identities.iter().cloned())
            .map(DiagnosticToolIdentity::from_mcp_identity)
            .collect::<Vec<_>>();
        let raw_total_count = mcp
            .tools_list_responses
            .iter()
            .map(|response| response.total_count)
            .sum();
        let raw_identities_truncated = mcp
            .tools_list_responses
            .iter()
            .any(|response| response.identities_truncated);
        let raw_fingerprint = {
            let mut hasher = Sha1::new();
            for response in &mcp.tools_list_responses {
                hasher.update(response.server_name.as_bytes());
                hasher.update([0]);
                hasher.update(response.fingerprint.as_bytes());
                hasher.update([0]);
            }
            format!("{:x}", hasher.finalize())
        };
        let inventory_identities = inventory
            .iter()
            .map(DiagnosticToolIdentity::from_tool_info)
            .collect::<Vec<_>>();
        let generation = fingerprint(&inventory_identities);
        let observation_sources = mcp
            .tools_list_responses
            .iter()
            .map(|response| format!("{}={:?}", response.server_name, response.source))
            .collect();

        let mut state = self.state();
        state.inventory_generation = generation.clone();
        state.identity_by_tool_name.clear();
        record_stage(
            &mut state,
            "mcp_tools_list_response",
            &generation,
            raw_identities,
            StageMetadata {
                observation_sources,
                server_count: Some(mcp.server_count),
                cached_server_count: Some(mcp.cached_server_count),
                startup_complete_server_count: Some(mcp.startup_complete_server_count),
                total_count_override: Some(raw_total_count),
                upstream_identities_truncated: raw_identities_truncated,
                fingerprint_override: Some(raw_fingerprint),
                ..Default::default()
            },
            Vec::new(),
        );
        record_stage(
            &mut state,
            "mcp_inventory_snapshot",
            &generation,
            inventory_identities,
            StageMetadata {
                server_count: Some(mcp.server_count),
                cached_server_count: Some(mcp.cached_server_count),
                startup_complete_server_count: Some(mcp.startup_complete_server_count),
                ..Default::default()
            },
            Vec::new(),
        );
    }

    pub(crate) fn record_exposure(
        &self,
        decisions: Vec<ToolExposureDecision>,
        search_tool_enabled: bool,
    ) {
        let identities = decisions
            .iter()
            .map(|decision| decision.identity.clone())
            .collect();
        let included_count = decisions
            .iter()
            .filter(|decision| decision.included)
            .count();
        let excluded = decisions
            .into_iter()
            .filter(|decision| !decision.included)
            .collect();
        let mut state = self.state();
        let generation = state.inventory_generation.clone();
        record_stage(
            &mut state,
            "mcp_exposure_filter",
            &generation,
            identities,
            StageMetadata {
                included_count: Some(included_count),
                search_tool_enabled: Some(search_tool_enabled),
                ..Default::default()
            },
            excluded,
        );
    }

    pub(crate) fn record_deferred(&self, search_infos: &[ToolSearchInfo], cache_state: &str) {
        self.record_search_info_stage("deferred_tool_search_info", search_infos, cache_state);
    }

    pub(crate) fn record_index(&self, search_infos: &[ToolSearchInfo], cache_state: &str) {
        self.record_search_info_stage("bm25_index_cache", search_infos, cache_state);
    }

    pub(crate) fn record_search(
        &self,
        query: &str,
        requested_limit: usize,
        returned_count: usize,
        mut results: Vec<SearchResultDiagnostic>,
    ) {
        let mut state = self.state();
        for result in &mut results {
            enrich_identity(&state, &mut result.identity);
        }
        let results_truncated = results.len() > MAX_SEARCH_RESULTS;
        results.truncate(MAX_SEARCH_RESULTS);
        let generation = state.inventory_generation.clone();
        state.searches.push_back(SearchSnapshot {
            stage: "bm25_search_results",
            inventory_generation: generation,
            query: bounded(query.to_string(), MAX_QUERY_CHARS),
            requested_limit,
            returned_count,
            results,
            results_truncated,
        });
        while state.searches.len() > MAX_SEARCHES {
            state.searches.pop_front();
        }
    }

    pub(crate) fn feedback_json(
        &self,
        thread_id: ThreadId,
        session_id: SessionId,
    ) -> Option<Vec<u8>> {
        let state = self.state();
        if state.stages.is_empty() && state.searches.is_empty() {
            return None;
        }
        let snapshot = FeedbackSnapshot {
            version: 1,
            thread_id: thread_id.to_string(),
            session_id: session_id.to_string(),
            inventory_generation: state.inventory_generation.clone(),
            limits: FeedbackLimits {
                max_stage_identities: MAX_STAGE_IDENTITIES,
                max_stage_changes: MAX_STAGE_CHANGES,
                max_excluded_identities: MAX_EXCLUDED_IDENTITIES,
                max_searches: MAX_SEARCHES,
                max_search_results: MAX_SEARCH_RESULTS,
                max_identity_chars: MAX_IDENTITY_CHARS,
                max_query_chars: MAX_QUERY_CHARS,
            },
            stages: state.stages.values().cloned().collect(),
            searches: state.searches.iter().cloned().collect(),
        };
        serde_json::to_vec(&snapshot).ok()
    }

    fn record_search_info_stage(
        &self,
        stage: &str,
        search_infos: &[ToolSearchInfo],
        cache_state: &str,
    ) {
        let mut state = self.state();
        let mut identities = search_infos
            .iter()
            .map(DiagnosticToolIdentity::from_search_info)
            .collect::<Vec<_>>();
        for identity in &mut identities {
            enrich_identity(&state, identity);
        }
        let generation = state.inventory_generation.clone();
        record_stage(
            &mut state,
            stage,
            &generation,
            identities,
            StageMetadata {
                cache_state: Some(cache_state.to_string()),
                ..Default::default()
            },
            Vec::new(),
        );
    }

    fn state(&self) -> std::sync::MutexGuard<'_, RecorderState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[derive(Serialize)]
struct FeedbackSnapshot {
    version: u8,
    thread_id: String,
    session_id: String,
    inventory_generation: String,
    limits: FeedbackLimits,
    stages: Vec<StageSnapshot>,
    searches: Vec<SearchSnapshot>,
}

#[derive(Serialize)]
struct FeedbackLimits {
    max_stage_identities: usize,
    max_stage_changes: usize,
    max_excluded_identities: usize,
    max_searches: usize,
    max_search_results: usize,
    max_identity_chars: usize,
    max_query_chars: usize,
}

fn record_stage(
    state: &mut RecorderState,
    stage: &str,
    generation: &str,
    mut identities: Vec<DiagnosticToolIdentity>,
    metadata: StageMetadata,
    mut excluded: Vec<ToolExposureDecision>,
) {
    identities.sort();
    identities.dedup();
    let total_count = metadata.total_count_override.unwrap_or(identities.len());
    let stage_fingerprint = metadata
        .fingerprint_override
        .clone()
        .unwrap_or_else(|| fingerprint(&identities));
    let retained_keys = identities
        .iter()
        .take(MAX_STAGE_IDENTITIES)
        .map(DiagnosticToolIdentity::key)
        .collect::<Vec<_>>();
    let identities_truncated =
        metadata.upstream_identities_truncated || identities.len() > MAX_STAGE_IDENTITIES;
    identities.truncate(MAX_STAGE_IDENTITIES);
    let excluded_truncated = excluded.len() > MAX_EXCLUDED_IDENTITIES;
    excluded.truncate(MAX_EXCLUDED_IDENTITIES);

    if let Some(existing) = state.stages.get_mut(stage)
        && existing.fingerprint == stage_fingerprint
    {
        existing.inventory_generation = generation.to_string();
        existing.total_count = total_count;
        existing.identities_truncated = identities_truncated;
        existing.metadata = metadata;
        existing.excluded = excluded;
        existing.excluded_truncated = excluded_truncated;
        return;
    }

    let previous_keys = state
        .stages
        .get(stage)
        .map(|existing| {
            existing
                .retained_keys
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let current_keys = retained_keys.iter().cloned().collect::<BTreeSet<_>>();
    let mut added = current_keys
        .difference(&previous_keys)
        .cloned()
        .collect::<Vec<_>>();
    let mut removed = previous_keys
        .difference(&current_keys)
        .cloned()
        .collect::<Vec<_>>();
    let changes_truncated = metadata.upstream_identities_truncated
        || added.len() > MAX_STAGE_CHANGES
        || removed.len() > MAX_STAGE_CHANGES;
    added.truncate(MAX_STAGE_CHANGES);
    removed.truncate(MAX_STAGE_CHANGES);

    for identity in &identities {
        if let Some(tool_name) = identity.tool_name.as_ref() {
            state
                .identity_by_tool_name
                .insert(tool_name.clone(), identity.clone());
        }
    }
    state.stages.insert(
        stage.to_string(),
        StageSnapshot {
            stage: stage.to_string(),
            inventory_generation: generation.to_string(),
            fingerprint: stage_fingerprint,
            total_count,
            identities,
            identities_truncated,
            added,
            removed,
            changes_truncated,
            excluded,
            excluded_truncated,
            metadata,
            retained_keys,
        },
    );
}

fn enrich_identity(state: &RecorderState, identity: &mut DiagnosticToolIdentity) {
    let Some(tool_name) = identity.tool_name.as_ref() else {
        return;
    };
    let Some(known) = state.identity_by_tool_name.get(tool_name) else {
        return;
    };
    let source_name = identity.connector_name.clone();
    *identity = known.clone();
    if identity.connector_name.is_none() {
        identity.connector_name = source_name;
    }
}

fn fingerprint(identities: &[DiagnosticToolIdentity]) -> String {
    let mut hasher = Sha1::new();
    for identity in identities {
        hasher.update(identity.key().as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn search_info_output_identity(search_info: &ToolSearchInfo) -> String {
    match &search_info.entry.output {
        LoadableToolSpec::Function(tool) => tool.name.clone(),
        LoadableToolSpec::Namespace(namespace) => {
            let Some(ResponsesApiNamespaceTool::Function(tool)) = namespace.tools.first() else {
                return namespace.name.clone();
            };
            if namespace.tools.len() == 1 {
                format!("{}.{}", namespace.name, tool.name)
            } else {
                format!("{} (+{} tools)", namespace.name, namespace.tools.len())
            }
        }
    }
}

fn bounded(value: String, max_chars: usize) -> String {
    let mut chars = value.chars();
    let bounded = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{bounded}…")
    } else {
        bounded
    }
}

#[cfg(test)]
#[path = "tool_search_diagnostics_tests.rs"]
mod tests;
