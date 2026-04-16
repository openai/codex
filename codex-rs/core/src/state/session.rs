//! Session-wide mutable state.

use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_sandboxing::policy_transforms::merge_permission_profiles;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::codex::PreviousTurnSettings;
use crate::codex::SessionConfiguration;
use crate::context_manager::ContextManager;
use crate::session_startup_prewarm::SessionStartupPrewarmHandle;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::protocol::TurnContextItem;
use codex_utils_output_truncation::TruncationPolicy;

/// Persistent, session-scoped state previously stored directly on `Session`.
pub(crate) struct SessionState {
    pub(crate) session_configuration: SessionConfiguration,
    pub(crate) history: ContextManager,
    pub(crate) latest_rate_limits: Option<RateLimitSnapshot>,
    pub(crate) server_reasoning_included: bool,
    pub(crate) dependency_env: HashMap<String, String>,
    pub(crate) mcp_dependency_prompted: HashSet<String>,
    /// Settings used by the latest regular user turn, used for turn-to-turn
    /// model/realtime handling on subsequent regular turns (including full-context
    /// reinjection after resume or `/compact`).
    previous_turn_settings: Option<PreviousTurnSettings>,
    /// Startup prewarmed session prepared during session initialization.
    pub(crate) startup_prewarm: Option<SessionStartupPrewarmHandle>,
    pub(crate) active_connector_selection: HashSet<String>,
    pub(crate) pending_session_start_source: Option<codex_hooks::SessionStartSource>,
    granted_permissions: Option<PermissionProfile>,
    next_prefix_compact_generation: u64,
    prefix_compact: PrefixCompactState,
    next_turn_is_first: bool,
}

impl SessionState {
    /// Create a new session state mirroring previous `State::default()` semantics.
    pub(crate) fn new(session_configuration: SessionConfiguration) -> Self {
        let history = ContextManager::new();
        Self {
            session_configuration,
            history,
            latest_rate_limits: None,
            server_reasoning_included: false,
            dependency_env: HashMap::new(),
            mcp_dependency_prompted: HashSet::new(),
            previous_turn_settings: None,
            startup_prewarm: None,
            active_connector_selection: HashSet::new(),
            pending_session_start_source: None,
            granted_permissions: None,
            next_prefix_compact_generation: 0,
            prefix_compact: PrefixCompactState::Idle,
            next_turn_is_first: true,
        }
    }

    // History helpers
    pub(crate) fn record_items<I>(&mut self, items: I, policy: TruncationPolicy)
    where
        I: IntoIterator,
        I::Item: std::ops::Deref<Target = ResponseItem>,
    {
        self.history.record_items(items, policy);
    }

    pub(crate) fn previous_turn_settings(&self) -> Option<PreviousTurnSettings> {
        self.previous_turn_settings.clone()
    }
    pub(crate) fn set_previous_turn_settings(
        &mut self,
        previous_turn_settings: Option<PreviousTurnSettings>,
    ) {
        self.previous_turn_settings = previous_turn_settings;
    }

    pub(crate) fn set_next_turn_is_first(&mut self, value: bool) {
        self.next_turn_is_first = value;
    }

    pub(crate) fn take_next_turn_is_first(&mut self) -> bool {
        let is_first_turn = self.next_turn_is_first;
        self.next_turn_is_first = false;
        is_first_turn
    }

    pub(crate) fn clone_history(&self) -> ContextManager {
        self.history.clone()
    }

    pub(crate) fn replace_history(
        &mut self,
        items: Vec<ResponseItem>,
        reference_context_item: Option<TurnContextItem>,
    ) {
        self.history.replace(items);
        self.history
            .set_reference_context_item(reference_context_item);
        self.prefix_compact = PrefixCompactState::Idle;
    }

    pub(crate) fn begin_prefix_compact(
        &mut self,
        model_slug: String,
    ) -> Option<PrefixCompactStart> {
        if !matches!(self.prefix_compact, PrefixCompactState::Idle) {
            return None;
        }

        let base_history = self.history.raw_items().to_vec();
        if base_history.is_empty() {
            return None;
        }

        let generation = self.next_prefix_compact_generation;
        self.next_prefix_compact_generation = self.next_prefix_compact_generation.saturating_add(1);
        self.prefix_compact = PrefixCompactState::Running { generation };
        Some(PrefixCompactStart {
            generation,
            model_slug,
            base_history,
            captured_context: Vec::new(),
            captured_reference_context_item: None,
        })
    }

    pub(crate) fn finish_prefix_compact(&mut self, candidate: PrefixCompactCandidate) {
        if matches!(
            self.prefix_compact,
            PrefixCompactState::Running { generation } if generation == candidate.generation
        ) {
            self.prefix_compact = PrefixCompactState::Ready(candidate);
        }
    }

    pub(crate) fn fail_prefix_compact(&mut self, generation: u64) {
        if matches!(
            self.prefix_compact,
            PrefixCompactState::Running { generation: running } if running == generation
        ) {
            self.prefix_compact = PrefixCompactState::Idle;
        }
    }

    pub(crate) fn abandon_prefix_compact(&mut self) {
        self.prefix_compact = PrefixCompactState::Idle;
    }

    pub(crate) fn take_ready_prefix_compact(
        &mut self,
        model_slug: &str,
    ) -> Option<PrefixCompactCandidate> {
        let PrefixCompactState::Ready(candidate) = std::mem::take(&mut self.prefix_compact) else {
            return None;
        };

        let current_history = self.history.raw_items();
        let prefix_is_current = candidate.model_slug == model_slug
            && current_history.len() >= candidate.base_history.len()
            && current_history[..candidate.base_history.len()] == candidate.base_history;

        if prefix_is_current {
            Some(candidate)
        } else {
            None
        }
    }

    pub(crate) fn set_token_info(&mut self, info: Option<TokenUsageInfo>) {
        self.history.set_token_info(info);
    }

    pub(crate) fn set_reference_context_item(&mut self, item: Option<TurnContextItem>) {
        self.history.set_reference_context_item(item);
    }

    pub(crate) fn reference_context_item(&self) -> Option<TurnContextItem> {
        self.history.reference_context_item()
    }

    // Token/rate limit helpers
    pub(crate) fn update_token_info_from_usage(
        &mut self,
        usage: &TokenUsage,
        model_context_window: Option<i64>,
    ) {
        self.history.update_token_info(usage, model_context_window);
    }

    pub(crate) fn token_info(&self) -> Option<TokenUsageInfo> {
        self.history.token_info()
    }

    pub(crate) fn set_rate_limits(&mut self, snapshot: RateLimitSnapshot) {
        self.latest_rate_limits = Some(merge_rate_limit_fields(
            self.latest_rate_limits.as_ref(),
            snapshot,
        ));
    }

    pub(crate) fn token_info_and_rate_limits(
        &self,
    ) -> (Option<TokenUsageInfo>, Option<RateLimitSnapshot>) {
        (self.token_info(), self.latest_rate_limits.clone())
    }

    pub(crate) fn set_token_usage_full(&mut self, context_window: i64) {
        self.history.set_token_usage_full(context_window);
    }

    pub(crate) fn get_total_token_usage(&self, server_reasoning_included: bool) -> i64 {
        self.history
            .get_total_token_usage(server_reasoning_included)
    }

    pub(crate) fn set_server_reasoning_included(&mut self, included: bool) {
        self.server_reasoning_included = included;
    }

    pub(crate) fn server_reasoning_included(&self) -> bool {
        self.server_reasoning_included
    }

    pub(crate) fn record_mcp_dependency_prompted<I>(&mut self, names: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.mcp_dependency_prompted.extend(names);
    }

    pub(crate) fn mcp_dependency_prompted(&self) -> HashSet<String> {
        self.mcp_dependency_prompted.clone()
    }

    pub(crate) fn set_dependency_env(&mut self, values: HashMap<String, String>) {
        for (key, value) in values {
            self.dependency_env.insert(key, value);
        }
    }

    pub(crate) fn dependency_env(&self) -> HashMap<String, String> {
        self.dependency_env.clone()
    }

    pub(crate) fn set_session_startup_prewarm(
        &mut self,
        startup_prewarm: SessionStartupPrewarmHandle,
    ) {
        self.startup_prewarm = Some(startup_prewarm);
    }

    pub(crate) fn take_session_startup_prewarm(&mut self) -> Option<SessionStartupPrewarmHandle> {
        self.startup_prewarm.take()
    }

    // Adds connector IDs to the active set and returns the merged selection.
    pub(crate) fn merge_connector_selection<I>(&mut self, connector_ids: I) -> HashSet<String>
    where
        I: IntoIterator<Item = String>,
    {
        self.active_connector_selection.extend(connector_ids);
        self.active_connector_selection.clone()
    }

    // Returns the current connector selection tracked on session state.
    pub(crate) fn get_connector_selection(&self) -> HashSet<String> {
        self.active_connector_selection.clone()
    }

    // Removes all currently tracked connector selections.
    pub(crate) fn clear_connector_selection(&mut self) {
        self.active_connector_selection.clear();
    }

    pub(crate) fn set_pending_session_start_source(
        &mut self,
        value: Option<codex_hooks::SessionStartSource>,
    ) {
        self.pending_session_start_source = value;
    }

    pub(crate) fn take_pending_session_start_source(
        &mut self,
    ) -> Option<codex_hooks::SessionStartSource> {
        self.pending_session_start_source.take()
    }

    pub(crate) fn record_granted_permissions(&mut self, permissions: PermissionProfile) {
        self.granted_permissions =
            merge_permission_profiles(self.granted_permissions.as_ref(), Some(&permissions));
    }

    pub(crate) fn granted_permissions(&self) -> Option<PermissionProfile> {
        self.granted_permissions.clone()
    }
}

#[derive(Debug, Default)]
enum PrefixCompactState {
    #[default]
    Idle,
    Running {
        generation: u64,
    },
    Ready(PrefixCompactCandidate),
}

#[derive(Debug, Clone)]
pub(crate) struct PrefixCompactStart {
    pub(crate) generation: u64,
    pub(crate) model_slug: String,
    pub(crate) base_history: Vec<ResponseItem>,
    pub(crate) captured_context: Vec<ResponseItem>,
    pub(crate) captured_reference_context_item: Option<TurnContextItem>,
}

#[derive(Debug, Clone)]
pub(crate) struct PrefixCompactCandidate {
    pub(crate) generation: u64,
    pub(crate) model_slug: String,
    pub(crate) base_history: Vec<ResponseItem>,
    pub(crate) replacement_prefix: Vec<ResponseItem>,
    pub(crate) captured_context: Vec<ResponseItem>,
    pub(crate) captured_reference_context_item: Option<TurnContextItem>,
}

// Sometimes new snapshots don't include credits or plan information.
// Preserve those from the previous snapshot when missing. For `limit_id`, treat
// missing values as the default `"codex"` bucket.
fn merge_rate_limit_fields(
    previous: Option<&RateLimitSnapshot>,
    mut snapshot: RateLimitSnapshot,
) -> RateLimitSnapshot {
    if snapshot.limit_id.is_none() {
        snapshot.limit_id = Some("codex".to_string());
    }
    if snapshot.credits.is_none() {
        snapshot.credits = previous.and_then(|prior| prior.credits.clone());
    }
    if snapshot.plan_type.is_none() {
        snapshot.plan_type = previous.and_then(|prior| prior.plan_type);
    }
    snapshot
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
