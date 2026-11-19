//! Records attempted calls at existing execution boundaries. Request metadata policy
//! lives in the private request metadata module; this recorder must never dispatch or await tools.

#[cfg(test)]
#[path = "executed_tool_calls_direct_tests.rs"]
mod direct_tests;
mod mcp_attribution;
mod request_metadata;

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_code_mode::CellId;
use codex_features::Feature;
use codex_features::Features;
use codex_history::InitialHistory;
use codex_login::AuthOwner;
use codex_model_provider::provider_uses_first_party_auth_path;
use codex_protocol::ResponseItemId;
use codex_protocol::auth::AuthMode;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::mcp::McpAttribution;
use codex_protocol::mcp::McpAttributionSource;
use codex_protocol::models::ExecutedToolCall;
use codex_protocol::models::ExecutedToolCallArguments;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::ToolResultBody;
use codex_protocol::models::ToolResultMetadata;
use codex_protocol::models::executed_tool_call_metadata_bytes;
use codex_protocol::models::normalize_executed_tool_call_completeness;
use codex_protocol::openai_models::ToolMode;
use indexmap::IndexMap;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::session::step_context::StepContext;
use crate::session::turn_context::TurnContext;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::metadata_metrics;
use crate::tools::router::ToolCall;
use crate::turn_metadata::TurnMetadataState;
use codex_utils_string::serialized_json_bytes;

mod seen_ids;

use seen_ids::SeenIds;

const MAX_PENDING_EXECUTED_TOOL_CALLS: usize = 256;
// Limits new Direct metadata retained in history and the append-only rollout
// during one recorder lifetime. A new process cannot recover the exact prior
// budget because raw result metadata is intentionally not deserialized.
const MAX_RETAINED_DIRECT_METADATA_BYTES: usize = 1024 * 1024;
const MAX_TURN_RESULT_BODY_BYTES: usize = 1024 * 1024;

type ExecutedToolCallCache =
    HashMap<(std::mem::Discriminant<ResponseItem>, String), Vec<ExecutedToolCall>>;

/// Best-effort recording shared by a session and its Code Mode broker. Disabled
/// sessions allocate no recorder state; missing evidence remains incomplete.
#[derive(Clone, Default)]
pub(crate) struct ExecutedToolCalls {
    state: Arc<Mutex<Option<ExecutedToolCallRecorderState>>>,
    captures_result_bodies: bool,
    retained_direct_metadata_bytes: Arc<AtomicUsize>,
    pending_direct_calls: Arc<AtomicUsize>,
    mcp_attribution: mcp_attribution::McpAttributionRecorder,
}

/// Private bytes belong to one regular task. History keeps only the host-issued IDs.
pub(crate) struct ResultBodyTurn {
    turn_id: String,
    origin: Weak<TurnMetadataState>,
    owner: AuthOwner,
    auth_mode: AuthMode,
    retained: Mutex<RetainedResultBodies>,
}

#[derive(Default)]
struct RetainedResultBodies {
    by_id: HashMap<Uuid, Arc<ToolResultBody>>,
    bytes: usize,
}

/// Permission at dispatch for this call. Holding it must not extend the originating turn.
pub(super) struct ResultBodyCapture<'a> {
    turn: Weak<ResultBodyTurn>,
    nested_call_id: Option<&'a str>,
}

impl ResultBodyCapture<'_> {
    pub(super) fn record(
        self,
        result: &CallToolResult,
        permits_source: impl FnOnce(AuthMode) -> bool,
    ) -> Option<Arc<PendingResultBody>> {
        let turn = self.turn.upgrade()?;
        if !permits_source(turn.auth_mode) || !turn.is_current() {
            return None;
        }
        let mut body = ToolResultBody::from_mcp_result(result)?;
        if let Some(call_id) = self.nested_call_id {
            body.set_nested_call_id(call_id);
        }
        let mut retained = turn.lock_bodies();
        if !turn.is_current()
            || retained.by_id.len() >= MAX_PENDING_EXECUTED_TOOL_CALLS
            || body.compact_bytes() > MAX_TURN_RESULT_BODY_BYTES - retained.bytes
        {
            return None;
        }
        let id = Uuid::new_v4();
        retained.bytes += body.compact_bytes();
        retained.by_id.insert(id, Arc::new(body));
        Some(Arc::new(PendingResultBody {
            turn: self.turn,
            id,
            accepted: AtomicBool::new(false),
        }))
    }
}

/// Refund the provisional slot when a callback blocks or the call is cancelled.
#[derive(Debug)]
pub(crate) struct PendingResultBody {
    turn: Weak<ResultBodyTurn>,
    id: Uuid,
    accepted: AtomicBool,
}

impl PendingResultBody {
    pub(super) fn accept(&self) -> Option<Uuid> {
        self.turn.upgrade().filter(|turn| turn.is_current())?;
        self.accepted.store(true, Ordering::Relaxed);
        Some(self.id)
    }
}

impl Drop for PendingResultBody {
    fn drop(&mut self) {
        if !*self.accepted.get_mut() {
            ResultBodyTurn::discard(&self.turn, Some(self.id));
        }
    }
}

impl ResultBodyTurn {
    fn discard_call(turn: &Weak<Self>, call: &mut ExecutedToolCall) {
        Self::discard(turn, call.tool_result_id());
        call.clear_tool_result_id();
    }

    fn discard(turn: &Weak<Self>, id: Option<Uuid>) {
        if let Some(id) = id
            && let Some(turn) = turn.upgrade()
        {
            let mut retained = turn.lock_bodies();
            if let Some(body) = retained.by_id.remove(&id) {
                retained.bytes -= body.compact_bytes();
            }
        }
    }

    fn is_current(&self) -> bool {
        self.owner.is_current()
    }

    fn lock_bodies(&self) -> MutexGuard<'_, RetainedResultBodies> {
        self.retained
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn start_capture<'a>(
        self: &Arc<Self>,
        nested_call_id: Option<&'a str>,
    ) -> Option<ResultBodyCapture<'a>> {
        if !self.is_current() {
            return None;
        }
        Some(ResultBodyCapture {
            turn: Arc::downgrade(self),
            nested_call_id,
        })
    }

    /// Select the existing call records on this authorized sampling request.
    pub(crate) fn for_input(
        &self,
        input: &[ResponseItem],
        selected_owner: &AuthOwner,
    ) -> Vec<(usize, usize, Arc<ToolResultBody>)> {
        let mut results = Vec::new();
        if self.owner != *selected_owner || !self.is_current() {
            return results;
        }
        let retained = self.lock_bodies();
        if retained.by_id.is_empty() {
            return results;
        }
        for (index, item) in input.iter().enumerate() {
            let Some(calls) = item
                .executed_tool_call_metadata()
                .and_then(|metadata| metadata.executed_tool_calls.as_deref())
            else {
                continue;
            };
            for (row, call) in calls.iter().enumerate() {
                if let Some(body) = call.tool_result_id().and_then(|id| retained.by_id.get(&id)) {
                    results.push((index, row, Arc::clone(body)));
                }
            }
        }
        results
    }
}

// Avoid exposing recorded call arguments or result metadata through client Debug output.
impl std::fmt::Debug for ExecutedToolCalls {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExecutedToolCalls")
            .finish_non_exhaustive()
    }
}

// The tool future owns this reservation, so completion or cancellation releases it.
pub(crate) struct DirectCallPermit {
    recording: Weak<()>,
    pending_direct_calls: Arc<AtomicUsize>,
}

impl DirectCallPermit {
    pub(crate) fn strong_count(&self) -> usize {
        self.recording.strong_count()
    }
}

impl Drop for DirectCallPermit {
    fn drop(&mut self) {
        self.pending_direct_calls.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct ExecutedToolCallRecorderState {
    include_general_metadata: bool,
    // Disabling drops this lifetime, permanently invalidating prepared Direct records.
    recording: Arc<()>,
    // Full live Direct observations; persisted outputs keep their bounded projection.
    direct_calls: HashMap<ResponseItemId, ExecutedToolCall>,
    body_turn: Weak<ResultBodyTurn>,
    cells: HashMap<CellId, RecordedCell>,
    output_cells: HashMap<String, CellId>,
    retained_calls: HashMap<(std::mem::Discriminant<ResponseItem>, String), RetainedToolCalls>,
    pending_nested_calls: usize,
    seen_ids: SeenIds,
    // Keep nested IDs separate so this conservative filter cannot change existing completeness.
    seen_nested_ids: SeenIds,
    can_prove_wait_completion: bool,
    pending_wrapper_origins: HashSet<String>,
}

/// Keep each output's calls and completion marker together through replay and pruning.
#[derive(Default)]
struct RetainedToolCalls {
    output_item_id: Option<ResponseItemId>,
    calls: Vec<ExecutedToolCall>,
    complete: bool,
    cell_id: Option<String>,
    runtime_cell_id: Option<CellId>,
    // Invocation IDs stay local and are retained only with their original output's calls.
    call_index_by_id: HashMap<String, usize>,
    // Track newly backfilled metadata separately from pre-existing observations.
    truncated_call_index_by_id: HashMap<String, usize>,
    late_truncated_indices: HashSet<usize>,
    result_metadata_updated: bool,
}

impl RetainedToolCalls {
    fn invalidate_output(&mut self, turn: &Weak<ResultBodyTurn>) {
        if self.output_item_id.take().is_some() {
            for call in &mut self.calls {
                ResultBodyTurn::discard_call(turn, call);
            }
        }
        self.clear_late_truncated_metadata(turn);
    }

    fn clear_late_truncated_metadata(&mut self, turn: &Weak<ResultBodyTurn>) {
        for index in self.late_truncated_indices.drain() {
            if let Some(call) = self.calls.get_mut(index) {
                ResultBodyTurn::discard(turn, call.tool_result_id());
                call.set_tool_result_metadata(ToolResultMetadata::default());
                self.result_metadata_updated = true;
            }
        }
        self.truncated_call_index_by_id.clear();
    }
}

#[derive(Default, PartialEq, Eq)]
enum CellCompletion {
    #[default]
    Unobserved,
    Recording,
    Incomplete,
    Complete,
}

#[derive(Default)]
struct RecordedCell {
    output_item_id: Option<ResponseItemId>,
    pending_calls: IndexMap<String, ExecutedToolCall>,
    completion: CellCompletion,
    originating_call_id: Option<String>,
    truncated_metadata_binding_valid: bool,
    observed_truncated_call: bool,
    dispatch_closed: bool,
}

impl RecordedCell {
    fn invalidate_binding(&mut self, turn: &Weak<ResultBodyTurn>) {
        // Unbound rows lose their IDs at attachment. Only a live turn has bytes to release here.
        if std::mem::take(&mut self.truncated_metadata_binding_valid) && turn.strong_count() != 0 {
            for call in self.pending_calls.values_mut() {
                ResultBodyTurn::discard_call(turn, call);
            }
        }
    }
}

impl ExecutedToolCallRecorderState {
    fn invalidate_origin(&mut self, origin: &str) {
        self.pending_wrapper_origins.remove(origin);
        for cell in self.cells.values_mut() {
            if cell.originating_call_id.as_deref() == Some(origin) {
                cell.completion = CellCompletion::Incomplete;
                cell.invalidate_binding(&self.body_turn);
            }
        }
        for ((_, output_id), retained) in &mut self.retained_calls {
            if output_id == origin || retained.cell_id.as_deref() == Some(origin) {
                retained.complete = false;
                retained.invalidate_output(&self.body_turn);
            }
        }
    }

    // A successful callback consumes the freshness observed at wrapper submission.
    fn observe_cell_origin(&mut self, origin: &str) -> bool {
        let fresh =
            self.pending_wrapper_origins.remove(origin) || self.seen_ids.observe_call_id(origin);
        if !fresh {
            self.invalidate_origin(origin);
        }
        fresh
    }

    fn invalidate_cell(&mut self, cell_id: &CellId) {
        if let Some(cell) = self.cells.get_mut(cell_id) {
            cell.completion = CellCompletion::Incomplete;
            cell.invalidate_binding(&self.body_turn);
        }
        for retained in self.retained_calls.values_mut() {
            if retained.runtime_cell_id.as_ref() == Some(cell_id) {
                retained.complete = false;
                retained.invalidate_output(&self.body_turn);
            }
        }
    }

    fn register_cell(&mut self, cell_id: &CellId, output_call_id: &str) {
        if self.output_cells.len() >= MAX_PENDING_EXECUTED_TOOL_CALLS {
            // Ended empty cells can leave mappings without any records to attach.
            // Reclaim those only under pressure, preserving late partial records otherwise.
            self.output_cells
                .retain(|_, cell_id| self.cells.contains_key(cell_id));
        }
        loop {
            let needs_cell_slot = self.cells.len() >= MAX_PENDING_EXECUTED_TOOL_CALLS
                && !self.cells.contains_key(cell_id);
            if !needs_cell_slot && self.pending_nested_calls < MAX_PENDING_EXECUTED_TOOL_CALLS {
                break;
            }

            let output_cells = self.output_cells.values().collect::<HashSet<_>>();
            let finished_cell = self.cells.iter().find_map(|(id, cell)| {
                // Preserve late records until pressure, and never discard the cell
                // whose output is about to make those records attachable again.
                // Empty cells can free cell slots, but not pending-call slots.
                (id != cell_id
                    && (needs_cell_slot || !cell.pending_calls.is_empty())
                    && matches!(
                        cell.completion,
                        CellCompletion::Complete | CellCompletion::Incomplete
                    )
                    && !output_cells.contains(id))
                .then(|| (id.clone(), cell.pending_calls.len()))
            });
            let Some((id, pending_calls)) = finished_cell else {
                break;
            };
            self.invalidate_cell(&id);
            self.cells.remove(&id);
            self.pending_nested_calls = self.pending_nested_calls.saturating_sub(pending_calls);
        }
        if (self.cells.len() >= MAX_PENDING_EXECUTED_TOOL_CALLS
            && !self.cells.contains_key(cell_id))
            || (self.output_cells.len() >= MAX_PENDING_EXECUTED_TOOL_CALLS
                && !self.output_cells.contains_key(output_call_id))
        {
            return;
        }
        self.cells
            .entry(cell_id.clone())
            .or_default()
            .originating_call_id
            .get_or_insert_with(|| output_call_id.to_string());
        self.output_cells
            .insert(output_call_id.to_string(), cell_id.clone());
    }
}

impl ExecutedToolCalls {
    pub(crate) fn new(features: &Features, history: &InitialHistory) -> Self {
        Self {
            captures_result_bodies: features.enabled(Feature::ConnectorToolResultPassthrough),
            state: Arc::new(Mutex::new(Self::is_enabled(features).then(|| {
                ExecutedToolCallRecorderState {
                    include_general_metadata: features.enabled(Feature::ExecutedToolCallMetadata),
                    seen_ids: SeenIds::from_history(history),
                    // Runtime cell IDs can restart after resume/fork; inherited wait
                    // handles cannot be distinguished from newly allocated handles.
                    can_prove_wait_completion: matches!(
                        history,
                        InitialHistory::New | InitialHistory::Cleared
                    ),
                    ..Default::default()
                }
            }))),
            retained_direct_metadata_bytes: Arc::new(AtomicUsize::new(0)),
            pending_direct_calls: Arc::new(AtomicUsize::new(0)),
            mcp_attribution: mcp_attribution::McpAttributionRecorder::new(history),
        }
    }

    pub(crate) fn is_result_body_provider_header(name: &str) -> bool {
        name.eq_ignore_ascii_case("version") || name.eq_ignore_ascii_case("x-openai-originator")
    }

    pub(crate) fn captures_result_bodies(&self) -> bool {
        self.captures_result_bodies
    }

    pub(crate) fn begin_turn(&self, turn: &TurnContext) -> Option<Arc<ResultBodyTurn>> {
        if !self.captures_result_bodies
            || !turn
                .config
                .features
                .enabled(Feature::ConnectorToolResultPassthrough)
        {
            return None;
        }
        let manager = turn.auth_manager.as_ref()?;
        let info = turn.provider.info();
        // The built-in provider declares optional organization/project selectors. The
        // transport checks the resolved headers again and declines either active selector.
        if !info.include_internal_metadata
            || info.is_amazon_bedrock()
            || info.is_gcp()
            || !provider_uses_first_party_auth_path(info)
            || info.gateway_oauth.is_some()
            || !turn
                .provider
                .auth_manager()
                .is_some_and(|provider| Arc::ptr_eq(manager, &provider))
            || info
                .query_params
                .as_ref()
                .is_some_and(|params| !params.is_empty())
            || info
                .http_headers
                .iter()
                .flat_map(HashMap::keys)
                .chain(info.env_http_headers.iter().flat_map(HashMap::keys))
                .any(|name| {
                    !Self::is_result_body_provider_header(name)
                        && !name.eq_ignore_ascii_case(codex_client::OPENAI_ORGANIZATION_HEADER)
                        && !name.eq_ignore_ascii_case(codex_client::OPENAI_PROJECT_HEADER)
                })
        {
            return None;
        }
        let owner = manager.auth_owner();
        let auth_mode = manager.auth_mode().filter(|_| owner.is_current())?;
        if !matches!(auth_mode, AuthMode::Chatgpt | AuthMode::ChatgptAuthTokens)
            || !info
                .base_url
                .as_deref()
                .is_none_or(crate::client::is_local_connector_result_gateway)
        {
            return None;
        }
        let capture = Arc::new(ResultBodyTurn {
            owner,
            auth_mode,
            turn_id: turn.sub_id.clone(),
            origin: Arc::downgrade(&turn.turn_metadata_state),
            retained: Mutex::default(),
        });
        self.lock_state().as_mut()?.body_turn = Arc::downgrade(&capture);
        Some(capture)
    }

    pub(crate) fn result_body_turn(&self, turn_id: &str) -> Option<Arc<ResultBodyTurn>> {
        if !self.captures_result_bodies {
            return None;
        }
        self.lock_state()
            .as_ref()?
            .body_turn
            .upgrade()
            .filter(|capture| capture.turn_id == turn_id)
    }

    fn current_result_turn(&self, turn: &TurnContext) -> Option<Arc<ResultBodyTurn>> {
        self.result_body_turn(&turn.sub_id).filter(|capture| {
            capture.is_current()
                && capture
                    .origin
                    .ptr_eq(&Arc::downgrade(&turn.turn_metadata_state))
        })
    }

    pub(super) fn start_mcp_result_capture<'a>(
        &self,
        turn: &TurnContext,
        call: &codex_mcp::PreparedMcpCall,
        nested_call_id: Option<&'a str>,
    ) -> Option<ResultBodyCapture<'a>> {
        self.current_result_turn(turn)
            .filter(|capture| call.permits_result_body(capture.auth_mode))?
            .start_capture(nested_call_id)
    }

    pub(crate) fn record_mcp_source(&self, source: McpAttributionSource) {
        self.mcp_attribution.record(source);
    }

    pub(crate) fn mcp_attribution_snapshot(&self) -> McpAttribution {
        self.mcp_attribution.snapshot()
    }

    pub(crate) fn mcp_attribution_checkpoint(&self, force: bool) -> Option<(McpAttribution, u64)> {
        self.mcp_attribution.checkpoint(force)
    }

    pub(crate) fn mark_mcp_attribution_persisted(&self, revision: u64) {
        self.mcp_attribution.mark_persisted(revision);
    }

    /// Consult the inventory attached to this exact exec output during actor sampling.
    /// Pending cells can still produce other outputs and have no per-output inventory yet.
    pub(crate) fn exec_output_matches(
        &self,
        output: &ResponseItem,
        nested_tool_names: &[String],
    ) -> bool {
        let ResponseItem::CustomToolCallOutput { call_id, .. } = output else {
            return false;
        };
        let state = self.lock_state();
        state
            .as_ref()
            .and_then(|state| {
                state
                    .retained_calls
                    .get(&(std::mem::discriminant(output), call_id.clone()))
            })
            .is_some_and(|record| {
                record.complete
                    && record.cell_id.as_deref() == Some(call_id.as_str())
                    && record.calls.iter().any(|call| {
                        nested_tool_names.contains(&call.name)
                            || nested_tool_names.contains(
                                &codex_code_mode::normalize_code_mode_identifier(&call.name),
                            )
                    })
            })
    }

    fn lock_state(&self) -> MutexGuard<'_, Option<ExecutedToolCallRecorderState>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A later wait cannot claim a complete inventory if an earlier wire copy
    /// lost recorded calls or tool names from the same Code Mode cell.
    pub(crate) fn invalidate_wire_inventory_loss(
        &self,
        original: &[ResponseItem],
        bounded: &[ResponseItem],
    ) {
        let mut state = self.lock_state();
        let Some(state) = state.as_mut() else {
            return;
        };
        // Request bounding clones the same items in the same order; it only edits metadata.
        for (original, bounded) in original.iter().zip(bounded) {
            let Some(metadata) = original.executed_tool_call_metadata() else {
                continue;
            };
            let Some(origin) = metadata.cell_id.as_deref() else {
                continue;
            };
            let Some(calls) = metadata
                .executed_tool_calls
                .as_deref()
                .filter(|calls| !calls.is_empty())
            else {
                continue;
            };
            if bounded
                .executed_tool_call_metadata()
                .is_none_or(|bounded| !bounded.has_same_tool_call_inventory(calls))
            {
                state.invalidate_origin(origin);
            }
        }
    }

    /// Called under the session config lock; this never changes execution features.
    pub(crate) fn refresh(&self, features: &Features) {
        let include_general_metadata = features.enabled(Feature::ExecutedToolCallMetadata);
        let enabled = self.captures_result_bodies || include_general_metadata;
        let mut state = self.lock_state();
        if enabled != state.is_some() {
            *state = enabled.then(|| ExecutedToolCallRecorderState {
                seen_ids: SeenIds::unobserved_history(),
                ..Default::default()
            });
        }
        if let Some(state) = state.as_mut() {
            state.include_general_metadata = include_general_metadata;
        }
    }

    /// The turn's feature policy is independent of whether this session has a recorder.
    pub(crate) fn is_enabled(features: &Features) -> bool {
        features.enabled(Feature::ExecutedToolCallMetadata)
            || features.enabled(Feature::ConnectorToolResultPassthrough)
    }

    pub(crate) fn record_tool_call(
        &self,
        call: &ToolCall,
        source: &ToolCallSource,
        step_context: &StepContext,
    ) {
        self.record_call(call, source, step_context.tool_router.tool_mode());
    }

    /// Keep a Direct record with its invocation so it can be attached to that invocation's output.
    pub(crate) fn prepare_direct_call(
        &self,
        call: &ToolCall,
        source: &ToolCallSource,
        step_context: &StepContext,
    ) -> Option<(ExecutedToolCall, DirectCallPermit)> {
        if is_code_mode_wrapper(call, source, step_context.tool_router.tool_mode()) {
            return None;
        }
        let permit = self.reserve_direct_call()?;
        let (recorded, _) = recorded_call(call);
        Some((recorded, permit))
    }

    fn reserve_direct_call(&self) -> Option<DirectCallPermit> {
        let recording = Arc::downgrade(&self.lock_state().as_ref()?.recording);
        self.pending_direct_calls
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |pending| {
                (pending < MAX_PENDING_EXECUTED_TOOL_CALLS).then_some(pending + 1)
            })
            .ok()?;
        Some(DirectCallPermit {
            recording,
            pending_direct_calls: Arc::clone(&self.pending_direct_calls),
        })
    }

    /// Validate the recording lifetime and attach together so a config refresh
    /// cannot invalidate the call between those two operations.
    pub(crate) fn attach_direct_call_to_output(
        &self,
        item: &mut ResponseItem,
        prepared: Option<(ExecutedToolCall, DirectCallPermit)>,
        item_id_generator: &codex_protocol::ItemIdGenerator,
    ) {
        let Some((mut call, permit)) = prepared else {
            if self.captures_result_bodies
                && let Some(state) = self.lock_state().as_mut()
                && state.body_turn.strong_count() != 0
                && let Some(id) = output_call_id(item).and_then(|id| state.output_cells.get(id))
                && let Some(cell) = state.cells.get_mut(id)
                && cell.truncated_metadata_binding_valid
            {
                // Bind nested results to the actual exec/wait output before history projection.
                crate::session::assign_missing_item_id(item, item_id_generator);
                cell.output_item_id = item.id().cloned();
            }
            return;
        };
        let mut state = self.lock_state();
        let Some(state) = state.as_mut() else {
            return;
        };
        if !permit.recording.ptr_eq(&Arc::downgrade(&state.recording))
            || (!state.include_general_metadata && call.tool_result_id().is_none())
        {
            return;
        }
        crate::session::assign_missing_item_id(item, item_id_generator);
        let keep_live_call = !self.captures_result_bodies
            || state.direct_calls.len() < MAX_PENDING_EXECUTED_TOOL_CALLS
            || item
                .id()
                .is_some_and(|id| state.direct_calls.contains_key(id));
        if !keep_live_call {
            // The ordinary bounded observation can still travel on the output itself.
            ResultBodyTurn::discard_call(&state.body_turn, &mut call);
        }
        let complete = call.has_complete_inventory();
        item.append_executed_tool_calls(vec![call.clone()]);
        if complete {
            item.mark_tool_calls_complete();
        }
        let retained = self.retained_direct_metadata_bytes.load(Ordering::Relaxed);
        let available = MAX_RETAINED_DIRECT_METADATA_BYTES.saturating_sub(retained);
        let mut bytes = executed_tool_call_metadata_bytes(item);
        let original_bytes = bytes;
        if bytes > available {
            item.retain_tool_resource_access_or_omit_metadata(bytes - available);
            bytes = executed_tool_call_metadata_bytes(item);
        }
        if bytes > available {
            item.omit_tool_result_metadata(bytes - available);
            bytes = executed_tool_call_metadata_bytes(item);
        }
        if bytes > available {
            item.clear_tool_result_metadata();
            bytes = executed_tool_call_metadata_bytes(item);
        }
        if bytes > available {
            item.clear_executed_tool_calls();
            ResultBodyTurn::discard_call(&state.body_turn, &mut call);
            bytes = executed_tool_call_metadata_bytes(item);
        }
        if keep_live_call && let Some(id) = item.id() {
            state.direct_calls.insert(id.clone(), call);
        }
        // All Direct attachments share the recorder lock, including cloned handles.
        self.retained_direct_metadata_bytes
            .store(retained + bytes, Ordering::Relaxed);
        metadata_metrics::record_shedding(
            "direct_retained",
            original_bytes,
            bytes,
            codex_otel::global().as_ref(),
        );
    }

    /// Remember IDs from response items that bypass local tool dispatch.
    pub(crate) fn observe_non_dispatched_call(&self, item: &ResponseItem) {
        let Some(call_id) = input_call_id(item) else {
            return;
        };
        let mut state = self.lock_state();
        let Some(state) = state.as_mut() else {
            return;
        };
        if !state.seen_ids.observe_call_id(call_id) {
            state.invalidate_origin(call_id);
        }
    }

    pub(crate) fn record_accepted_result(
        &self,
        source: &ToolCallSource,
        call_id: &str,
        result: &dyn ToolOutput,
    ) {
        if !matches!(source, ToolCallSource::CodeMode { .. }) {
            return;
        }
        // Release the recorder lock before reading the output. Only refund this turn on failure.
        let Some(turn) = self
            .lock_state()
            .as_ref()
            .map(|state| state.body_turn.clone())
        else {
            return;
        };
        let metadata = result.tool_result_metadata();
        let id = metadata.result_id();
        if !self.record_result_metadata(source, call_id, metadata) {
            ResultBodyTurn::discard(&turn, id);
        }
    }

    fn record_call(&self, call: &ToolCall, source: &ToolCallSource, tool_mode: ToolMode) {
        if self.lock_state().is_none() {
            return;
        }
        match source {
            ToolCallSource::Direct | ToolCallSource::DirectPlaintextMessage => {
                let mut state = self.lock_state();
                let Some(state) = state.as_mut() else {
                    return;
                };
                let fresh = state.seen_ids.observe_call_id(&call.call_id);
                if !fresh {
                    state.invalidate_origin(&call.call_id);
                }
                if fresh
                    && is_code_mode_wrapper(call, source, tool_mode)
                    && state.pending_wrapper_origins.len() < MAX_PENDING_EXECUTED_TOOL_CALLS
                {
                    state.pending_wrapper_origins.insert(call.call_id.clone());
                }
            }
            ToolCallSource::CodeMode { cell_id, .. } => {
                let (recorded_call, original_bytes) = recorded_call(call);
                self.record_nested_tool_call(
                    CellId::new(cell_id.clone()),
                    call.call_id.clone(),
                    recorded_call,
                    original_bytes,
                );
            }
        }
    }

    fn record_nested_tool_call(
        &self,
        cell_id: CellId,
        call_id: String,
        call: ExecutedToolCall,
        original_bytes: usize,
    ) {
        let mut state = self.lock_state();
        let Some(state) = state.as_mut() else {
            return;
        };
        let unique_nested_id = state.seen_nested_ids.observe_call_id(&call_id);
        if !unique_nested_id {
            for retained in state.retained_calls.values_mut() {
                if retained.truncated_call_index_by_id.contains_key(&call_id) {
                    retained.clear_late_truncated_metadata(&state.body_turn);
                }
            }
        }
        if state
            .cells
            .get(&cell_id)
            .is_none_or(|cell| cell.dispatch_closed)
        {
            state.invalidate_cell(&cell_id);
        }
        if state.pending_nested_calls > MAX_PENDING_EXECUTED_TOOL_CALLS
            || (state.cells.len() >= MAX_PENDING_EXECUTED_TOOL_CALLS
                && !state.cells.contains_key(&cell_id))
        {
            if let Some(cell) = state.cells.get_mut(&cell_id) {
                cell.completion = CellCompletion::Incomplete;
            }
            return;
        }
        let at_pending_call_limit = state.pending_nested_calls == MAX_PENDING_EXECUTED_TOOL_CALLS;
        let cell = state.cells.entry(cell_id).or_default();
        let duplicate_call_id = cell.pending_calls.contains_key(&call_id);
        if !unique_nested_id || duplicate_call_id {
            cell.invalidate_binding(&state.body_turn);
        }
        let call = if at_pending_call_limit {
            ExecutedToolCall::truncated(call.name, original_bytes, /*max_bytes*/ 0)
        } else {
            call
        };
        let truncated = matches!(
            call.arguments(),
            ExecutedToolCallArguments::Truncated { .. }
        );
        cell.observed_truncated_call |= truncated;
        cell.completion = if cell.completion == CellCompletion::Recording
            && !duplicate_call_id
            && call.has_complete_inventory()
        {
            CellCompletion::Recording
        } else {
            CellCompletion::Incomplete
        };
        cell.pending_calls.insert(call_id, call);
        if !duplicate_call_id {
            state.pending_nested_calls += 1;
        }
    }

    fn record_result_metadata(
        &self,
        source: &ToolCallSource,
        call_id: &str,
        metadata: ToolResultMetadata,
    ) -> bool {
        let ToolCallSource::CodeMode { cell_id, .. } = source else {
            return false;
        };
        if !metadata.has_observation() {
            return false;
        }
        let mut state = self.lock_state();
        let Some(state) = state.as_mut() else {
            return false;
        };
        let turn = &state.body_turn;
        let update = |call: &mut ExecutedToolCall, can_bind_body: bool| {
            if call.tool_result_id() != metadata.result_id() {
                ResultBodyTurn::discard(turn, call.tool_result_id());
            }
            call.set_tool_result_metadata(metadata);
            if !can_bind_body {
                ResultBodyTurn::discard_call(turn, call);
            }
        };
        if let Some(cell) = state.cells.get_mut(&CellId::new(cell_id.clone()))
            && let Some(call) = cell.pending_calls.get_mut(call_id)
        {
            update(call, cell.truncated_metadata_binding_valid);
            return true;
        }
        let retained_call = state.retained_calls.values_mut().find_map(|retained| {
            if retained.runtime_cell_id.as_ref()?.as_str() != cell_id.as_str() {
                return None;
            }
            let index = *retained.call_index_by_id.get(call_id)?;
            Some((retained, index))
        });
        if let Some((retained, index)) = retained_call {
            // Older retry copies must not overwrite this output's accepted result metadata.
            retained.result_metadata_updated = true;
            update(
                &mut retained.calls[index],
                retained.output_item_id.is_some(),
            );
            return true;
        }
        let mut candidates = state.retained_calls.values_mut().filter_map(|retained| {
            if retained.runtime_cell_id.as_ref()?.as_str() != cell_id.as_str() {
                return None;
            }
            let index = *retained.truncated_call_index_by_id.get(call_id)?;
            Some((retained, index))
        });
        let Some((retained, index)) = candidates.next() else {
            return false;
        };
        if candidates.next().is_some() {
            return false;
        }
        retained.late_truncated_indices.insert(index);
        retained.result_metadata_updated = true;
        update(
            &mut retained.calls[index],
            retained.output_item_id.is_some(),
        );
        true
    }

    pub(crate) fn register_cell(&self, cell_id: &CellId, output_call_id: &str) {
        let mut state = self.lock_state();
        let Some(state) = state.as_mut() else {
            return;
        };
        if !state.observe_cell_origin(output_call_id) {
            state.invalidate_cell(cell_id);
        }
        state.register_cell(cell_id, output_call_id);
    }

    pub(crate) fn start_cell(&self, cell_id: &CellId, output_call_id: &str) {
        let mut state = self.lock_state();
        let Some(state) = state.as_mut() else {
            return;
        };
        let unique_cell = state.seen_ids.observe_runtime_cell_id(cell_id);
        let unique_origin = state.observe_cell_origin(output_call_id);
        if !unique_cell {
            state.invalidate_cell(cell_id);
            // A reused runtime ID cannot distinguish the old pending calls or
            // output mappings from this new execution. Do not attach them to it.
            if let Some(previous) = state.cells.remove(cell_id) {
                state.pending_nested_calls = state
                    .pending_nested_calls
                    .saturating_sub(previous.pending_calls.len());
            }
            state.output_cells.retain(|_, mapped| mapped != cell_id);
        }
        let history_ids_indexed = state.seen_ids.history_ids_indexed();
        state.register_cell(cell_id, output_call_id);
        if let Some(cell) = state.cells.get_mut(cell_id) {
            // Failed indexing must not make a known historical ID look fresh.
            cell.dispatch_closed = false;
            cell.truncated_metadata_binding_valid =
                unique_cell && unique_origin && history_ids_indexed;
            cell.completion = if unique_cell && unique_origin && history_ids_indexed {
                CellCompletion::Recording
            } else {
                CellCompletion::Incomplete
            };
        }
    }

    pub(crate) fn finish_cell_recording(&self, cell_id: &CellId) {
        let mut state = self.lock_state();
        let Some(state) = state.as_mut() else {
            return;
        };
        // A closed cell may still be referenced by the final wait output. Keep its
        // original binding while late truncated results remain eligible for backfill.
        let has_retained_truncated_calls = state.retained_calls.values().any(|retained| {
            retained.runtime_cell_id.as_ref() == Some(cell_id)
                && !retained.truncated_call_index_by_id.is_empty()
        });
        if let Some(cell) = state.cells.get_mut(cell_id) {
            cell.dispatch_closed = true;
            match cell.completion {
                CellCompletion::Recording => {
                    // The closed dispatch gate makes this lossless inventory final, even if empty.
                    cell.completion = CellCompletion::Complete;
                }
                CellCompletion::Unobserved | CellCompletion::Incomplete => {
                    // Drop unverified cells only after emitting any partial records.
                    if cell.pending_calls.is_empty() && !has_retained_truncated_calls {
                        state.cells.remove(cell_id);
                    }
                }
                CellCompletion::Complete => {}
            }
        }
    }
}

fn is_code_mode_wrapper(call: &ToolCall, source: &ToolCallSource, tool_mode: ToolMode) -> bool {
    matches!(source, ToolCallSource::Direct)
        && matches!(tool_mode, ToolMode::CodeMode | ToolMode::CodeModeOnly)
        && call.tool_name.is_default_namespace()
        && matches!(
            (call.tool_name.name.as_str(), &call.payload),
            (
                crate::tools::code_mode::PUBLIC_TOOL_NAME,
                ToolPayload::Custom { .. }
            ) | (
                crate::tools::code_mode::WAIT_TOOL_NAME,
                ToolPayload::Function { .. }
            )
        )
}

fn recorded_call(call: &ToolCall) -> (ExecutedToolCall, usize) {
    let original_bytes = match &call.payload {
        ToolPayload::Function { arguments } => arguments.len(),
        ToolPayload::Custom { input } => serialized_json_bytes(input).unwrap_or(usize::MAX),
        ToolPayload::ToolSearch { arguments } => {
            serialized_json_bytes(arguments).unwrap_or(usize::MAX)
        }
    };
    let name = codex_tools::code_mode_name_for_tool_name(&call.tool_name);
    let arguments = match &call.payload {
        ToolPayload::Function { arguments } => {
            serde_json::from_str(arguments).unwrap_or_else(|_| JsonValue::String(arguments.clone()))
        }
        ToolPayload::Custom { input } => JsonValue::String(input.clone()),
        ToolPayload::ToolSearch { arguments } => {
            serde_json::to_value(arguments).unwrap_or_default()
        }
    };
    (ExecutedToolCall::new(name, arguments), original_bytes)
}

pub(crate) fn input_call_id(item: &ResponseItem) -> Option<&str> {
    match item {
        ResponseItem::FunctionCall { call_id, .. }
        | ResponseItem::CustomToolCall { call_id, .. } => Some(call_id),
        ResponseItem::ToolSearchCall { call_id, .. }
        | ResponseItem::LocalShellCall { call_id, .. } => call_id.as_deref(),
        _ => None,
    }
}

pub(crate) fn output_call_id(item: &ResponseItem) -> Option<&str> {
    match item {
        ResponseItem::FunctionCallOutput { call_id, .. }
        | ResponseItem::ToolSearchOutput { call_id, .. } => call_id.as_deref(),
        ResponseItem::CustomToolCallOutput { call_id, .. } => Some(call_id),
        _ => None,
    }
}

pub(crate) fn is_matching_tool_output(call: &ResponseItem, output: &ResponseItem) -> bool {
    let Some(call_id) = input_call_id(call) else {
        return false;
    };
    output_call_id(output) == Some(call_id)
        && matches!(
            (call, output),
            (
                ResponseItem::FunctionCall { .. } | ResponseItem::LocalShellCall { .. },
                ResponseItem::FunctionCallOutput { .. }
            ) | (
                ResponseItem::CustomToolCall { .. },
                ResponseItem::CustomToolCallOutput { .. }
            ) | (
                ResponseItem::ToolSearchCall { .. },
                ResponseItem::ToolSearchOutput { .. }
            )
        )
}
