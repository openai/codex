use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

use codex_otel::MetricsClient;
use codex_protocol::ThreadId;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use serde::Serialize;

use crate::policy::is_persisted_rollout_item;

const ITEM_BYTES_METRIC: &str = "codex.rollout.persistence.item_bytes";
const APPEND_METRIC: &str = "codex.rollout.persistence.append";
const TURN_BYTES_METRIC: &str = "codex.rollout.persistence.turn_bytes";
const LOAD_THREAD_BYTES_METRIC: &str = "codex.rollout.persistence.load_thread_bytes";
const LOAD_THREAD_ITEMS_METRIC: &str = "codex.rollout.persistence.load_thread_items";
const LOAD_TURN_BYTES_METRIC: &str = "codex.rollout.persistence.load_turn_bytes";
const LOAD_TURN_ITEMS_METRIC: &str = "codex.rollout.persistence.load_turn_items";
const PROJECTION_THREAD_BYTES_METRIC: &str = "codex.rollout.persistence.projection_thread_bytes";
const PROJECTION_THREAD_ITEMS_METRIC: &str = "codex.rollout.persistence.projection_thread_items";
const PROJECTION_THREAD_TURNS_METRIC: &str = "codex.rollout.persistence.projection_thread_turns";
const PROJECTION_TURN_BYTES_METRIC: &str = "codex.rollout.persistence.projection_turn_bytes";
const PROJECTION_TURN_ITEMS_METRIC: &str = "codex.rollout.persistence.projection_turn_items";
const MEASUREMENT_ERROR_METRIC: &str = "codex.rollout.persistence.measurement_error";
const SAMPLE_DENOMINATOR: u64 = 100;
const SAMPLE_RATE_LABEL: &str = "0.01";
static MEASURED_ROLLOUT_LOAD_THREADS: LazyLock<Mutex<HashSet<ThreadId>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
static MEASURED_PROJECTED_THREADS: LazyLock<Mutex<HashSet<ThreadId>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistenceDecision {
    Kept,
    Dropped,
}

impl PersistenceDecision {
    fn as_str(self) -> &'static str {
        match self {
            Self::Kept => "kept",
            Self::Dropped => "dropped",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RolloutSizeTotals {
    pub items: u64,
    pub payload_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutItemMeasurement {
    pub decision: PersistenceDecision,
    pub rollout_item_type: String,
    pub payload_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RolloutPersistenceBatchMeasurement {
    pub pre_filter: RolloutSizeTotals,
    pub post_filter: RolloutSizeTotals,
    pub items: Vec<RolloutItemMeasurement>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TurnSizeTotals {
    pre_filter: RolloutSizeTotals,
    post_filter: RolloutSizeTotals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnOutcome {
    Completed,
    Aborted,
}

impl TurnOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Aborted => "aborted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompletedTurnMeasurement {
    totals: TurnSizeTotals,
    outcome: TurnOutcome,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct TurnMeasurementState {
    pending: TurnSizeTotals,
    active: Option<TurnSizeTotals>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct TurnMeasurementUpdate {
    completed: Vec<CompletedTurnMeasurement>,
    boundary_errors: Vec<&'static str>,
}

/// Measures logical JSON sizes while applying the shared rollout persistence policy once.
pub fn measure_and_filter_rollout_items(
    items: &[RolloutItem],
) -> (Vec<RolloutItem>, RolloutPersistenceBatchMeasurement) {
    let mut persisted = Vec::new();
    let mut measurement = RolloutPersistenceBatchMeasurement {
        items: Vec::with_capacity(items.len()),
        ..Default::default()
    };

    for item in items {
        let kept = is_persisted_rollout_item(item);
        let decision = if kept {
            PersistenceDecision::Kept
        } else {
            PersistenceDecision::Dropped
        };
        let payload_bytes = serialized_len(item).ok();
        add_to_totals(&mut measurement.pre_filter, payload_bytes);
        if kept {
            add_to_totals(&mut measurement.post_filter, payload_bytes);
            persisted.push(item.clone());
        }
        measurement.items.push(RolloutItemMeasurement {
            decision,
            rollout_item_type: rollout_item_type(item),
            payload_bytes,
        });
    }

    (persisted, measurement)
}

fn add_to_totals(totals: &mut RolloutSizeTotals, payload_bytes: Option<u64>) {
    totals.items = totals.items.saturating_add(1);
    if let Some(payload_bytes) = payload_bytes {
        totals.payload_bytes = totals.payload_bytes.saturating_add(payload_bytes);
    }
}

fn update_turn_measurements(
    state: &mut TurnMeasurementState,
    items: &[RolloutItem],
    measurement: &RolloutPersistenceBatchMeasurement,
) -> TurnMeasurementUpdate {
    let mut update = TurnMeasurementUpdate::default();
    for (item, item_measurement) in items.iter().zip(&measurement.items) {
        match item {
            RolloutItem::EventMsg(EventMsg::TurnStarted(_)) => {
                if state.active.take().is_some() {
                    update.boundary_errors.push("event.turn_started");
                }
                let mut totals = std::mem::take(&mut state.pending);
                add_item_to_turn(&mut totals, item_measurement);
                state.active = Some(totals);
            }
            RolloutItem::EventMsg(EventMsg::TurnComplete(_)) => {
                finish_turn(
                    state,
                    item_measurement,
                    TurnOutcome::Completed,
                    "event.turn_complete",
                    &mut update,
                );
            }
            RolloutItem::EventMsg(EventMsg::TurnAborted(_)) => {
                finish_turn(
                    state,
                    item_measurement,
                    TurnOutcome::Aborted,
                    "event.turn_aborted",
                    &mut update,
                );
            }
            _ => match state.active.as_mut() {
                Some(totals) => add_item_to_turn(totals, item_measurement),
                None => add_item_to_turn(&mut state.pending, item_measurement),
            },
        }
    }
    update
}

fn finish_turn(
    state: &mut TurnMeasurementState,
    item: &RolloutItemMeasurement,
    outcome: TurnOutcome,
    boundary_type: &'static str,
    update: &mut TurnMeasurementUpdate,
) {
    let Some(mut totals) = state.active.take() else {
        state.pending = TurnSizeTotals::default();
        update.boundary_errors.push(boundary_type);
        return;
    };
    add_item_to_turn(&mut totals, item);
    update
        .completed
        .push(CompletedTurnMeasurement { totals, outcome });
}

fn add_item_to_turn(totals: &mut TurnSizeTotals, item: &RolloutItemMeasurement) {
    add_to_totals(&mut totals.pre_filter, item.payload_bytes);
    if item.decision == PersistenceDecision::Kept {
        add_to_totals(&mut totals.post_filter, item.payload_bytes);
    }
}

fn serialized_len(item: &(impl Serialize + ?Sized)) -> serde_json::Result<u64> {
    let mut writer = CountingWriter::default();
    serde_json::to_writer(&mut writer, item)?;
    Ok(writer.bytes)
}

#[derive(Default)]
struct CountingWriter {
    bytes: u64,
}

impl Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.bytes = self.bytes.saturating_add(buf.len() as u64);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn rollout_item_type(item: &RolloutItem) -> String {
    match item {
        RolloutItem::SessionMeta(_) => "session_meta".to_string(),
        RolloutItem::ResponseItem(item) => response_item_type(item).to_string(),
        RolloutItem::InterAgentCommunication(_) => "inter_agent_communication".to_string(),
        RolloutItem::InterAgentCommunicationMetadata { .. } => {
            "inter_agent_communication_metadata".to_string()
        }
        RolloutItem::Compacted(_) => "compacted".to_string(),
        RolloutItem::TurnContext(_) => "turn_context".to_string(),
        RolloutItem::EventMsg(EventMsg::ItemCompleted(event)) => {
            format!("event.item_completed.{}", turn_item_type(&event.item))
        }
        RolloutItem::EventMsg(event) => format!("event.{event}"),
    }
}

fn turn_item_type(item: &TurnItem) -> &'static str {
    match item {
        TurnItem::UserMessage(_) => "user_message",
        TurnItem::HookPrompt(_) => "hook_prompt",
        TurnItem::AgentMessage(_) => "agent_message",
        TurnItem::Plan(_) => "plan",
        TurnItem::Reasoning(_) => "reasoning",
        TurnItem::WebSearch(_) => "web_search",
        TurnItem::ImageView(_) => "image_view",
        TurnItem::Sleep(_) => "sleep",
        TurnItem::ImageGeneration(_) => "image_generation",
        TurnItem::FileChange(_) => "file_change",
        TurnItem::McpToolCall(_) => "mcp_tool_call",
        TurnItem::ContextCompaction(_) => "context_compaction",
    }
}

fn response_item_type(item: &ResponseItem) -> &'static str {
    match item {
        ResponseItem::Message { .. } => "response.message",
        ResponseItem::AdditionalTools { .. } => "response.additional_tools",
        ResponseItem::AgentMessage { .. } => "response.agent_message",
        ResponseItem::Reasoning { .. } => "response.reasoning",
        ResponseItem::LocalShellCall { .. } => "response.local_shell_call",
        ResponseItem::FunctionCall { .. } => "response.function_call",
        ResponseItem::ToolSearchCall { .. } => "response.tool_search_call",
        ResponseItem::FunctionCallOutput { .. } => "response.function_call_output",
        ResponseItem::ToolSearchOutput { .. } => "response.tool_search_output",
        ResponseItem::CustomToolCall { .. } => "response.custom_tool_call",
        ResponseItem::CustomToolCallOutput { .. } => "response.custom_tool_call_output",
        ResponseItem::WebSearchCall { .. } => "response.web_search_call",
        ResponseItem::ImageGenerationCall { .. } => "response.image_generation_call",
        ResponseItem::Compaction { .. } => "response.compaction",
        ResponseItem::CompactionTrigger { .. } => "response.compaction_trigger",
        ResponseItem::ContextCompaction { .. } => "response.context_compaction",
        ResponseItem::Other => "response.other",
    }
}

#[derive(Clone)]
pub struct RolloutPersistenceTelemetry {
    metrics: Option<MetricsClient>,
    sampled: bool,
    turn_state: Option<Arc<Mutex<TurnMeasurementState>>>,
}

impl RolloutPersistenceTelemetry {
    pub fn new(thread_id: ThreadId) -> Self {
        let metrics = codex_otel::global();
        let sampled = metrics.is_some() && is_thread_sampled(thread_id);
        Self {
            metrics,
            sampled,
            turn_state: sampled.then(|| Arc::new(Mutex::new(TurnMeasurementState::default()))),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled_metrics().is_some()
    }

    pub fn record_batch(
        &self,
        items: &[RolloutItem],
        measurement: &RolloutPersistenceBatchMeasurement,
    ) {
        let Some(metrics) = self.enabled_metrics() else {
            return;
        };

        for item in &measurement.items {
            if let Some(payload_bytes) = item.payload_bytes {
                let _ = metrics.histogram(
                    ITEM_BYTES_METRIC,
                    saturating_i64(payload_bytes),
                    &[
                        ("decision", item.decision.as_str()),
                        ("rollout_item_type", item.rollout_item_type.as_str()),
                        ("encoding", "rollout_item_json_v1"),
                        ("sample_rate", SAMPLE_RATE_LABEL),
                    ],
                );
            } else {
                let _ = metrics.counter(
                    MEASUREMENT_ERROR_METRIC,
                    /*inc*/ 1,
                    &[
                        ("rollout_item_type", item.rollout_item_type.as_str()),
                        ("phase", "serialize"),
                    ],
                );
            }
        }
        // Count successful input appends and the subset that remain storage operations after the
        // persistence policy removes filtered items.
        let _ = metrics.counter(
            APPEND_METRIC,
            /*inc*/ 1,
            &[("stage", "pre_filter"), ("sample_rate", SAMPLE_RATE_LABEL)],
        );
        if measurement.post_filter.items > 0 {
            let _ = metrics.counter(
                APPEND_METRIC,
                /*inc*/ 1,
                &[("stage", "post_filter"), ("sample_rate", SAMPLE_RATE_LABEL)],
            );
        }

        let Some(turn_state) = self.turn_state.as_ref() else {
            return;
        };
        let turn_update = update_turn_measurements(
            &mut turn_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            items,
            measurement,
        );
        for boundary_type in turn_update.boundary_errors {
            let _ = metrics.counter(
                MEASUREMENT_ERROR_METRIC,
                /*inc*/ 1,
                &[
                    ("rollout_item_type", boundary_type),
                    ("phase", "turn_boundary"),
                ],
            );
        }
        for turn in turn_update.completed {
            for (stage, totals) in [
                ("pre_filter", turn.totals.pre_filter),
                ("post_filter", turn.totals.post_filter),
            ] {
                let _ = metrics.histogram(
                    TURN_BYTES_METRIC,
                    saturating_i64(totals.payload_bytes),
                    &[
                        ("stage", stage),
                        ("outcome", turn.outcome.as_str()),
                        ("encoding", "rollout_item_json_v1"),
                        ("sample_rate", SAMPLE_RATE_LABEL),
                    ],
                );
            }
        }
    }

    fn enabled_metrics(&self) -> Option<&MetricsClient> {
        self.sampled.then_some(self.metrics.as_ref()).flatten()
    }
}

/// Records detached per-thread and per-turn size proxies for rollout loads.
pub(crate) struct RolloutLoadTelemetry {
    metrics: Option<MetricsClient>,
    sampled: bool,
    thread_id: ThreadId,
}

impl RolloutLoadTelemetry {
    pub fn new(thread_id: ThreadId) -> Self {
        let metrics = codex_otel::global();
        let sampled = metrics.is_some() && is_thread_sampled(thread_id);
        Self {
            metrics,
            sampled,
            thread_id,
        }
    }

    /// Records bytes for successfully loaded decompressed JSONL records, excluding newlines.
    pub fn record_rollout_load(
        &self,
        totals: RolloutSizeTotals,
        turn_totals: &[RolloutSizeTotals],
    ) {
        let Some(metrics) = self.enabled_metrics() else {
            return;
        };
        if !mark_thread_measured(&MEASURED_ROLLOUT_LOAD_THREADS, self.thread_id) {
            return;
        }
        record_size_totals(
            metrics,
            totals,
            LOAD_THREAD_BYTES_METRIC,
            LOAD_THREAD_ITEMS_METRIC,
            "rollout_jsonl",
            "rollout_line_json_v1",
            "rollout_load",
        );
        record_turn_size_totals(
            metrics,
            turn_totals,
            LOAD_TURN_BYTES_METRIC,
            LOAD_TURN_ITEMS_METRIC,
            "rollout_jsonl",
            "rollout_line_json_v1",
            "rollout_load",
        );
    }

    fn enabled_metrics(&self) -> Option<&MetricsClient> {
        self.sampled.then_some(self.metrics.as_ref()).flatten()
    }
}

/// Records detached size proxies for app-server ThreadItem projection reads.
pub struct RolloutProjectionTelemetry {
    metrics: Option<MetricsClient>,
    sampled: bool,
    thread_id: ThreadId,
}

impl RolloutProjectionTelemetry {
    pub fn new(thread_id: ThreadId) -> Self {
        let metrics = codex_otel::global();
        let sampled = metrics.is_some() && is_thread_sampled(thread_id);
        Self {
            metrics,
            sampled,
            thread_id,
        }
    }

    /// Records compact JSON bytes for the final materialized app-server turns.
    pub fn record_projected_turns<'a, T, U>(
        &self,
        turns: &T,
        turn_count: u64,
        item_count: u64,
        forecast_turn_items: impl IntoIterator<Item = (&'a U, u64)>,
    ) where
        T: Serialize + ?Sized,
        U: Serialize + 'a,
    {
        let Some(metrics) = self.enabled_metrics() else {
            return;
        };
        if MEASURED_PROJECTED_THREADS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&self.thread_id)
        {
            return;
        }
        let payload_bytes = match serialized_len(turns) {
            Ok(payload_bytes) => payload_bytes,
            Err(_) => {
                let _ = metrics.counter(
                    MEASUREMENT_ERROR_METRIC,
                    /*inc*/ 1,
                    &[
                        ("phase", "projection_serialize"),
                        ("representation", "thread_turns"),
                        ("source", "app_server_thread_read"),
                    ],
                );
                return;
            }
        };
        let turn_totals = match forecast_turn_items
            .into_iter()
            .map(|(items, item_count)| {
                serialized_len(items).map(|payload_bytes| RolloutSizeTotals {
                    items: item_count,
                    payload_bytes,
                })
            })
            .collect::<serde_json::Result<Vec<_>>>()
        {
            Ok(turn_totals) => turn_totals,
            Err(_) => {
                let _ = metrics.counter(
                    MEASUREMENT_ERROR_METRIC,
                    /*inc*/ 1,
                    &[
                        ("phase", "projection_turn_serialize"),
                        ("representation", "thread_turns"),
                        ("source", "app_server_thread_read"),
                    ],
                );
                return;
            }
        };
        if !mark_thread_measured(&MEASURED_PROJECTED_THREADS, self.thread_id) {
            return;
        }
        record_size_totals(
            metrics,
            RolloutSizeTotals {
                items: item_count,
                payload_bytes,
            },
            PROJECTION_THREAD_BYTES_METRIC,
            PROJECTION_THREAD_ITEMS_METRIC,
            "thread_turns",
            "app_server_turns_json_v2",
            "app_server_thread_read",
        );
        record_turn_size_totals(
            metrics,
            &turn_totals,
            PROJECTION_TURN_BYTES_METRIC,
            PROJECTION_TURN_ITEMS_METRIC,
            "thread_turns",
            "app_server_turn_items_json_v2",
            "app_server_thread_read",
        );
        let _ = metrics.histogram(
            PROJECTION_THREAD_TURNS_METRIC,
            saturating_i64(turn_count),
            &[
                ("representation", "thread_turns"),
                ("encoding", "app_server_turns_json_v2"),
                ("sample_rate", SAMPLE_RATE_LABEL),
                ("source", "app_server_thread_read"),
            ],
        );
    }

    fn enabled_metrics(&self) -> Option<&MetricsClient> {
        self.sampled.then_some(self.metrics.as_ref()).flatten()
    }
}

#[derive(Default)]
pub(crate) struct RolloutTurnSizeTracker {
    current: Option<PendingRolloutTurn>,
    completed: Vec<RolloutSizeTotals>,
}

#[derive(Default)]
struct PendingRolloutTurn {
    totals: RolloutSizeTotals,
    explicit: bool,
    saw_user_message: bool,
    saw_final_agent_message: bool,
}

impl RolloutTurnSizeTracker {
    pub(crate) fn observe(&mut self, item: &RolloutItem, payload_bytes: u64) {
        match item {
            RolloutItem::EventMsg(EventMsg::TurnStarted(_)) => {
                self.finish_implicit_turn();
                self.current = Some(PendingRolloutTurn {
                    explicit: true,
                    ..Default::default()
                });
            }
            RolloutItem::EventMsg(EventMsg::UserMessage(_)) => {
                if self
                    .current
                    .as_ref()
                    .is_some_and(|turn| !turn.explicit && turn.saw_user_message)
                {
                    self.finish_implicit_turn();
                }
                self.current
                    .get_or_insert_with(PendingRolloutTurn::default)
                    .saw_user_message = true;
            }
            _ => {}
        }

        let Some(turn) = self.current.as_mut() else {
            return;
        };
        turn.totals.items = turn.totals.items.saturating_add(1);
        turn.totals.payload_bytes = turn.totals.payload_bytes.saturating_add(payload_bytes);

        match item {
            RolloutItem::EventMsg(EventMsg::AgentMessage(message))
                if !matches!(
                    message.phase,
                    Some(codex_protocol::models::MessagePhase::Commentary)
                ) =>
            {
                turn.saw_final_agent_message = true;
            }
            RolloutItem::EventMsg(EventMsg::TurnComplete(_)) => self.finish_current_turn(),
            RolloutItem::EventMsg(EventMsg::TurnAborted(_)) => {
                self.current = None;
            }
            _ => {}
        }
    }

    pub(crate) fn finish(mut self) -> Vec<RolloutSizeTotals> {
        self.finish_implicit_turn();
        self.completed
    }

    fn finish_implicit_turn(&mut self) {
        if self.current.as_ref().is_some_and(|turn| !turn.explicit) {
            self.finish_current_turn();
        }
    }

    fn finish_current_turn(&mut self) {
        if let Some(turn) = self.current.take()
            && turn.saw_user_message
            && turn.saw_final_agent_message
        {
            self.completed.push(turn.totals);
        }
    }
}

fn record_size_totals(
    metrics: &MetricsClient,
    totals: RolloutSizeTotals,
    bytes_metric: &'static str,
    items_metric: &'static str,
    representation: &str,
    encoding: &str,
    source: &str,
) {
    let tags = [
        ("representation", representation),
        ("encoding", encoding),
        ("sample_rate", SAMPLE_RATE_LABEL),
        ("source", source),
    ];
    let _ = metrics.histogram(bytes_metric, saturating_i64(totals.payload_bytes), &tags);
    let _ = metrics.histogram(items_metric, saturating_i64(totals.items), &tags);
}

fn record_turn_size_totals(
    metrics: &MetricsClient,
    turn_totals: &[RolloutSizeTotals],
    bytes_metric: &'static str,
    items_metric: &'static str,
    representation: &str,
    encoding: &str,
    source: &str,
) {
    let tags = [
        ("representation", representation),
        ("encoding", encoding),
        ("sample_rate", SAMPLE_RATE_LABEL),
        ("source", source),
    ];
    for totals in turn_totals {
        let _ = metrics.histogram(bytes_metric, saturating_i64(totals.payload_bytes), &tags);
        let _ = metrics.histogram(items_metric, saturating_i64(totals.items), &tags);
    }
}

fn mark_thread_measured(measured_threads: &Mutex<HashSet<ThreadId>>, thread_id: ThreadId) -> bool {
    measured_threads
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(thread_id)
}

fn saturating_i64(value: u64) -> i64 {
    value.try_into().unwrap_or(i64::MAX)
}

fn is_thread_sampled(thread_id: ThreadId) -> bool {
    let hash = thread_id
        .to_string()
        .bytes()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    hash % SAMPLE_DENOMINATOR == 0
}

#[cfg(test)]
#[path = "persistence_metrics_tests.rs"]
mod tests;
