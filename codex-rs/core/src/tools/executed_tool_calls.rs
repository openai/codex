//! Records attempted calls at existing execution boundaries. Request metadata policy
//! lives in the private request metadata module; this recorder must never dispatch or await tools.

#[cfg(test)]
#[path = "executed_tool_calls_direct_tests.rs"]
mod direct_tests;
mod request_metadata;

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::Weak;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_code_mode::CellId;
use codex_features::Feature;
use codex_features::Features;
use codex_history::InitialHistory;
use codex_protocol::models::ExecutedToolCall;
use codex_protocol::models::ExecutedToolCallArguments;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::bound_executed_tool_calls_for_prompt;
use codex_protocol::models::bound_executed_tool_calls_for_prompt_prioritizing_recent;
use codex_protocol::models::executed_tool_call_metadata_bytes;
use codex_protocol::openai_models::ToolMode;
use indexmap::IndexMap;
use serde_json::Value as JsonValue;

use crate::session::step_context::StepContext;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::router::ToolCall;
use crate::utils::json::serialized_json_bytes;

mod seen_ids;

use seen_ids::SeenIds;

const MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES: usize = 8 * 1024;
const MAX_EXECUTED_TOOL_CALL_FULL_ARGUMENT_BYTES_PER_OUTPUT: usize = 32 * 1024;
const MAX_PENDING_EXECUTED_TOOL_CALLS: usize = 256;
// Limits new Direct metadata retained in history and the append-only rollout
// during one recorder lifetime. A new process cannot recover the exact prior
// budget because raw result metadata is intentionally not deserialized.
const MAX_RETAINED_DIRECT_METADATA_BYTES: usize = 1024 * 1024;

type ExecutedToolCallCache =
    HashMap<(std::mem::Discriminant<ResponseItem>, String), Vec<ExecutedToolCall>>;

/// Best-effort recording shared by a session and its Code Mode broker. Disabled
/// sessions allocate no recorder state; missing evidence remains incomplete.
#[derive(Clone, Default)]
pub(crate) struct ExecutedToolCalls {
    state: Arc<Mutex<Option<ExecutedToolCallRecorderState>>>,
    retained_direct_metadata_bytes: Arc<AtomicUsize>,
    pending_direct_calls: Arc<AtomicUsize>,
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
    // Disabling drops this lifetime, permanently invalidating prepared Direct records.
    recording: Arc<()>,
    cells: HashMap<CellId, RecordedCell>,
    output_cells: HashMap<String, CellId>,
    retained_calls: HashMap<(std::mem::Discriminant<ResponseItem>, String), RetainedToolCalls>,
    pending_nested_calls: usize,
    seen_ids: SeenIds,
    can_prove_wait_completion: bool,
    pending_wrapper_origins: HashSet<String>,
}

/// Keep each output's calls and completion marker together through replay and pruning.
#[derive(Default)]
struct RetainedToolCalls {
    calls: Vec<ExecutedToolCall>,
    complete: bool,
    cell_id: Option<String>,
    runtime_cell_id: Option<CellId>,
    // Invocation IDs stay local and are retained only with their original output's calls.
    call_index_by_id: HashMap<String, usize>,
    result_metadata_updated: bool,
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
    pending_calls: IndexMap<String, ExecutedToolCall>,
    pending_full_argument_bytes: usize,
    completion: CellCompletion,
    originating_call_id: Option<String>,
}

impl ExecutedToolCallRecorderState {
    fn invalidate_origin(&mut self, origin: &str) {
        self.pending_wrapper_origins.remove(origin);
        for cell in self.cells.values_mut() {
            if cell.originating_call_id.as_deref() == Some(origin) {
                cell.completion = CellCompletion::Incomplete;
            }
        }
        for ((_, output_id), retained) in &mut self.retained_calls {
            if output_id == origin || retained.cell_id.as_deref() == Some(origin) {
                retained.complete = false;
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
        }
        for retained in self.retained_calls.values_mut() {
            if retained.runtime_cell_id.as_ref() == Some(cell_id) {
                retained.complete = false;
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
        while (self.cells.len() >= MAX_PENDING_EXECUTED_TOOL_CALLS
            && !self.cells.contains_key(cell_id))
            || self.pending_nested_calls >= MAX_PENDING_EXECUTED_TOOL_CALLS
        {
            let output_cells = self.output_cells.values().collect::<HashSet<_>>();
            let finished_cell = self.cells.iter().find_map(|(id, cell)| {
                // Preserve late records until pressure, and never discard the cell
                // whose output is about to make those records attachable again.
                (id != cell_id
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
            state: Arc::new(Mutex::new(Self::is_enabled(features).then(|| {
                ExecutedToolCallRecorderState {
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
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, Option<ExecutedToolCallRecorderState>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Called under the session config lock; this never changes execution features.
    pub(crate) fn refresh(&self, features: &Features) {
        let enabled = Self::is_enabled(features);
        let mut state = self.lock_state();
        if enabled != state.is_some() {
            *state = enabled.then(|| ExecutedToolCallRecorderState {
                seen_ids: SeenIds::unobserved_history(),
                ..Default::default()
            });
        }
    }

    /// The turn's feature policy is independent of whether this session has a recorder.
    pub(crate) fn is_enabled(features: &Features) -> bool {
        features.enabled(Feature::ExecutedToolCallMetadata)
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
        let (mut recorded, _) = recorded_call(call);
        let argument_bytes = serialized_json_bytes(recorded.arguments()).unwrap_or(usize::MAX);
        if matches!(recorded.arguments(), ExecutedToolCallArguments::Raw(_))
            && argument_bytes > MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES
        {
            recorded = ExecutedToolCall::truncated(
                recorded.name,
                argument_bytes,
                MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES,
            );
        }
        Some((recorded, permit))
    }

    fn reserve_direct_call(&self) -> Option<DirectCallPermit> {
        let recording = Arc::downgrade(&self.lock_state().as_ref()?.recording);
        self.pending_direct_calls
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |pending| {
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
    ) {
        let Some((call, permit)) = prepared else {
            return;
        };
        let state = self.lock_state();
        let Some(state) = state.as_ref() else {
            return;
        };
        if !permit.recording.ptr_eq(&Arc::downgrade(&state.recording)) {
            return;
        }
        let complete = matches!(call.arguments(), ExecutedToolCallArguments::Raw(_));
        item.append_executed_tool_calls(vec![call]);
        if complete {
            item.mark_tool_calls_complete();
        }
        let retained = self.retained_direct_metadata_bytes.load(Ordering::Relaxed);
        let available = MAX_RETAINED_DIRECT_METADATA_BYTES.saturating_sub(retained);
        let mut bytes = executed_tool_call_metadata_bytes(item);
        if bytes > available {
            item.clear_tool_result_metadata();
            bytes = executed_tool_call_metadata_bytes(item);
        }
        if bytes > available {
            item.clear_executed_tool_calls();
            return;
        }
        // All Direct attachments share the recorder lock, including cloned handles.
        self.retained_direct_metadata_bytes
            .store(retained + bytes, Ordering::Relaxed);
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
        // Release the lock before calling the output's trait method.
        let recording = self.lock_state().is_some();
        if recording && let Some(metadata) = result.tool_result_metadata() {
            self.record_tool_result_metadata(source, call_id, metadata);
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
        if !state.cells.contains_key(&cell_id) {
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
        let max_bytes = MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES.min(
            MAX_EXECUTED_TOOL_CALL_FULL_ARGUMENT_BYTES_PER_OUTPUT
                .saturating_sub(cell.pending_full_argument_bytes),
        );
        let call = if at_pending_call_limit {
            ExecutedToolCall::truncated(call.name, original_bytes, /*max_bytes*/ 0)
        } else if original_bytes <= max_bytes {
            cell.pending_full_argument_bytes = cell
                .pending_full_argument_bytes
                .saturating_add(original_bytes);
            call
        } else {
            ExecutedToolCall::truncated(call.name, original_bytes, max_bytes)
        };
        cell.completion = if cell.completion == CellCompletion::Recording
            && !duplicate_call_id
            && !matches!(
                call.arguments(),
                ExecutedToolCallArguments::Truncated { .. }
            ) {
            CellCompletion::Recording
        } else {
            CellCompletion::Incomplete
        };
        cell.pending_calls.insert(call_id, call);
        state.pending_nested_calls += 1;
    }

    fn record_tool_result_metadata(
        &self,
        source: &ToolCallSource,
        call_id: &str,
        metadata: &JsonValue,
    ) -> bool {
        let ToolCallSource::CodeMode { cell_id, .. } = source else {
            return false;
        };
        let metadata = codex_protocol::models::ToolResultMetadata::new(metadata);
        let has_metadata = metadata.is_some();
        let mut state = self.lock_state();
        let Some(state) = state.as_mut() else {
            return false;
        };
        let call = state
            .cells
            .get_mut(&CellId::new(cell_id.clone()))
            .and_then(|cell| cell.pending_calls.get_mut(call_id));
        if let Some(call) = call {
            call.set_tool_result_metadata(metadata);
            return has_metadata;
        }
        let Some((retained, index)) = state.retained_calls.values_mut().find_map(|retained| {
            if retained.runtime_cell_id.as_ref()?.as_str() != cell_id.as_str() {
                return None;
            }
            let index = *retained.call_index_by_id.get(call_id)?;
            Some((retained, index))
        }) else {
            return false;
        };
        // Older retry copies must not overwrite this output's accepted result metadata.
        retained.result_metadata_updated = true;
        retained.calls[index].set_tool_result_metadata(metadata);
        has_metadata
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
        }
        let history_ids_indexed = state.seen_ids.history_ids_indexed();
        state.register_cell(cell_id, output_call_id);
        if let Some(cell) = state.cells.get_mut(cell_id) {
            // Failed indexing must not make a known historical ID look fresh.
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
        if let Some(cell) = state.cells.get_mut(cell_id) {
            match cell.completion {
                CellCompletion::Recording => {
                    // The closed dispatch gate makes this lossless inventory final, even if empty.
                    cell.completion = CellCompletion::Complete;
                }
                CellCompletion::Unobserved | CellCompletion::Incomplete => {
                    // Drop unverified cells only after emitting any partial records.
                    if cell.pending_calls.is_empty() {
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
    let recorded_call = if original_bytes > MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES {
        ExecutedToolCall::truncated(name, original_bytes, MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES)
    } else {
        let arguments = match &call.payload {
            ToolPayload::Function { arguments } => serde_json::from_str(arguments)
                .unwrap_or_else(|_| JsonValue::String(arguments.clone())),
            ToolPayload::Custom { input } => JsonValue::String(input.clone()),
            ToolPayload::ToolSearch { arguments } => {
                serde_json::to_value(arguments).unwrap_or_default()
            }
        };
        ExecutedToolCall::new(name, arguments)
    };
    (recorded_call, original_bytes)
}

fn input_call_id(item: &ResponseItem) -> Option<&str> {
    match item {
        ResponseItem::FunctionCall { call_id, .. }
        | ResponseItem::CustomToolCall { call_id, .. } => Some(call_id),
        ResponseItem::ToolSearchCall { call_id, .. }
        | ResponseItem::LocalShellCall { call_id, .. } => call_id.as_deref(),
        _ => None,
    }
}

fn output_call_id(item: &ResponseItem) -> Option<&str> {
    match item {
        ResponseItem::FunctionCallOutput { call_id, .. }
        | ResponseItem::ToolSearchOutput { call_id, .. } => call_id.as_deref(),
        ResponseItem::CustomToolCallOutput { call_id, .. } => Some(call_id),
        _ => None,
    }
}
