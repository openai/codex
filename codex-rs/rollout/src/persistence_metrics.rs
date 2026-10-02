use std::borrow::Cow;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;

use codex_otel::MetricsClient;
use codex_protocol::ThreadId;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadHistoryMode;

use crate::ResponseItemEnvelope;
use crate::RolloutItem;
use crate::policy::persisted_rollout_item;

const ITEM_BYTES_METRIC: &str = "codex.rollout.persistence.item_bytes_v2";
const BYTES_REMOVED_METRIC: &str = "codex.rollout.persistence.bytes_removed";
const APPEND_METRIC: &str = "codex.rollout.persistence.append";
const TURN_BYTES_METRIC: &str = "codex.rollout.persistence.turn_bytes_v2";
const MEASUREMENT_ERROR_METRIC: &str = "codex.rollout.persistence.measurement_error";
const SAMPLE_DENOMINATOR: u64 = 100;
const SAMPLE_RATE_LABEL: &str = "0.01";

const KIB: f64 = 1_024.0;
const MIB: f64 = 1_024.0 * KIB;

// Cover the persistence cap while retaining enough range for large items and turns. Values above
// 64 MiB remain visible in the overflow bucket and histogram sum.
const ROLLOUT_BYTES_BOUNDARIES: &[f64] = &[
    KIB,
    4.0 * KIB,
    16.0 * KIB,
    64.0 * KIB,
    256.0 * KIB,
    MIB,
    4.0 * MIB,
    16.0 * MIB,
    64.0 * MIB,
];

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
    pub persisted_payload_bytes: Option<u64>,
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
    history_mode: ThreadHistoryMode,
) -> (Vec<RolloutItem>, RolloutPersistenceBatchMeasurement) {
    let mut persisted = Vec::new();
    let mut measurement = RolloutPersistenceBatchMeasurement {
        items: Vec::with_capacity(items.len()),
        ..Default::default()
    };

    for item in items {
        let projected = persisted_rollout_item(item, history_mode);
        let decision = if projected.is_some() {
            PersistenceDecision::Kept
        } else {
            PersistenceDecision::Dropped
        };
        let payload_bytes = serialized_len(item).ok();
        let persisted_payload_bytes = match projected.as_ref() {
            Some(Cow::Borrowed(_)) => payload_bytes,
            Some(Cow::Owned(item)) => serialized_len(item).ok(),
            None => None,
        };
        add_to_totals(&mut measurement.pre_filter, payload_bytes);
        if let Some(projected) = projected {
            add_to_totals(&mut measurement.post_filter, persisted_payload_bytes);
            persisted.push(projected.into_owned());
        }
        measurement.items.push(RolloutItemMeasurement {
            decision,
            rollout_item_type: rollout_item_type(item),
            payload_bytes,
            persisted_payload_bytes,
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
        add_to_totals(&mut totals.post_filter, item.persisted_payload_bytes);
    }
}

fn serialized_len(item: &RolloutItem) -> serde_json::Result<u64> {
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
        RolloutItem::TokenUsageRecord(_) => "token_usage_record".to_string(),
        RolloutItem::WorldState(_) => "world_state".to_string(),
        RolloutItem::RetainedContext(_) => "retained_context".to_string(),
        RolloutItem::SecurityRiskScore(_) => "security_risk_score".to_string(),
        RolloutItem::RealtimeItem(item) => match &item.content {
            codex_protocol::realtime::RealtimeItemContent::RealtimeSessionStarted => {
                "realtime.session_started".to_string()
            }
            codex_protocol::realtime::RealtimeItemContent::TranscriptSegment { .. } => {
                "realtime.transcript_segment".to_string()
            }
            codex_protocol::realtime::RealtimeItemContent::BemItemPromoted { .. } => {
                "realtime.bem_item_promoted".to_string()
            }
            codex_protocol::realtime::RealtimeItemContent::RealtimeSessionClosed { .. } => {
                "realtime.session_closed".to_string()
            }
        },
        RolloutItem::EventMsg(EventMsg::ItemCompleted(event)) => {
            format!("event.item_completed.{}", turn_item_type(&event.item))
        }
        RolloutItem::EventMsg(event) => format!("event.{event}"),
    }
}

fn turn_item_type(item: &TurnItem) -> &'static str {
    match item {
        TurnItem::UserMessage(_) => "user_message",
        TurnItem::FunctionCallOutput(_) => "function_call_output",
        TurnItem::HookPrompt(_) => "hook_prompt",
        TurnItem::AgentMessage(_) => "agent_message",
        TurnItem::Plan(_) => "plan",
        TurnItem::Reasoning(_) => "reasoning",
        TurnItem::CommandExecution(_) => "command_execution",
        TurnItem::DynamicToolCall(_) => "dynamic_tool_call",
        TurnItem::CollabAgentToolCall(_) => "collab_agent_tool_call",
        TurnItem::SubAgentActivity(_) => "sub_agent_activity",
        TurnItem::WebSearch(_) => "web_search",
        TurnItem::ImageView(_) => "image_view",
        TurnItem::Extension(_) => "extension",
        TurnItem::ImageGeneration(_) => "image_generation",
        TurnItem::EnteredReviewMode(_) => "entered_review_mode",
        TurnItem::ExitedReviewMode(_) => "exited_review_mode",
        TurnItem::FileChange(_) => "file_change",
        TurnItem::McpToolCall(_) => "mcp_tool_call",
        TurnItem::ContextCompaction(_) => "context_compaction",
    }
}

fn response_item_type(item: &ResponseItemEnvelope) -> &'static str {
    match &item.item {
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
        ResponseItem::ConfigurationUpdate { .. } => "response.configuration_update",
        ResponseItem::CompactionTrigger { .. } => "response.compaction_trigger",
        ResponseItem::ContextCompaction { .. } => "response.context_compaction",
        ResponseItem::Other => "response.other",
    }
}

fn record_item_bytes(
    metrics: &MetricsClient,
    stage: &'static str,
    item: &RolloutItemMeasurement,
    payload_bytes: u64,
) {
    let _ = metrics.histogram_with_boundaries(
        ITEM_BYTES_METRIC,
        saturating_i64(payload_bytes),
        ROLLOUT_BYTES_BOUNDARIES,
        &[
            ("stage", stage),
            ("decision", item.decision.as_str()),
            ("rollout_item_type", item.rollout_item_type.as_str()),
            ("encoding", "rollout_item_json_v1"),
            ("sample_rate", SAMPLE_RATE_LABEL),
        ],
    );
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
                record_item_bytes(metrics, "before_projection", item, payload_bytes);
                let persisted_payload_bytes = match item.persisted_payload_bytes {
                    Some(persisted_payload_bytes) => {
                        record_item_bytes(
                            metrics,
                            "after_projection",
                            item,
                            persisted_payload_bytes,
                        );
                        persisted_payload_bytes
                    }
                    None if item.decision == PersistenceDecision::Dropped => 0,
                    None => {
                        let _ = metrics.counter(
                            MEASUREMENT_ERROR_METRIC,
                            /*inc*/ 1,
                            &[
                                ("rollout_item_type", item.rollout_item_type.as_str()),
                                ("phase", "serialize_projection"),
                            ],
                        );
                        continue;
                    }
                };
                let bytes_removed = payload_bytes.saturating_sub(persisted_payload_bytes);
                if bytes_removed > 0 {
                    let _ = metrics.histogram_with_boundaries(
                        BYTES_REMOVED_METRIC,
                        saturating_i64(bytes_removed),
                        ROLLOUT_BYTES_BOUNDARIES,
                        &[
                            ("decision", item.decision.as_str()),
                            ("rollout_item_type", item.rollout_item_type.as_str()),
                            ("encoding", "rollout_item_json_v1"),
                            ("sample_rate", SAMPLE_RATE_LABEL),
                        ],
                    );
                }
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
                ("before_projection", turn.totals.pre_filter),
                ("after_projection", turn.totals.post_filter),
            ] {
                let _ = metrics.histogram_with_boundaries(
                    TURN_BYTES_METRIC,
                    saturating_i64(totals.payload_bytes),
                    ROLLOUT_BYTES_BOUNDARIES,
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
