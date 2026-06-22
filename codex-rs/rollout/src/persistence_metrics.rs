use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_otel::MetricsClient;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;

use crate::policy::is_persisted_rollout_item;

const ITEM_BYTES_METRIC: &str = "codex.rollout.persistence.item_bytes";
const THREAD_BYTES_METRIC: &str = "codex.rollout.persistence.thread_bytes";
const THREAD_ITEMS_METRIC: &str = "codex.rollout.persistence.thread_items";
const MEASUREMENT_ERROR_METRIC: &str = "codex.rollout.persistence.measurement_error";
const SAMPLE_DENOMINATOR: u64 = 100;
const SAMPLE_RATE_LABEL: &str = "0.01";

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
    pub rollout_item_type: &'static str,
    pub payload_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RolloutPersistenceBatchMeasurement {
    pub pre_filter: RolloutSizeTotals,
    pub post_filter: RolloutSizeTotals,
    pub items: Vec<RolloutItemMeasurement>,
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

fn rollout_item_type(item: &RolloutItem) -> &'static str {
    match item {
        RolloutItem::SessionMeta(_) => "session_meta",
        RolloutItem::ResponseItem(item) => response_item_type(item),
        RolloutItem::InterAgentCommunication(_) => "inter_agent_communication",
        RolloutItem::Compacted(_) => "compacted",
        RolloutItem::TurnContext(_) => "turn_context",
        RolloutItem::EventMsg(_) => "event_msg",
    }
}

fn response_item_type(item: &ResponseItem) -> &'static str {
    match item {
        ResponseItem::Message { .. } | ResponseItem::AgentMessage { .. } => "response.message",
        ResponseItem::Reasoning { .. } => "response.reasoning",
        ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. } => "response.tool_call",
        ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::CustomToolCallOutput { .. } => "response.tool_output",
        ResponseItem::Compaction { .. }
        | ResponseItem::CompactionTrigger { .. }
        | ResponseItem::ContextCompaction { .. } => "response.compaction",
        ResponseItem::Other => "response.other",
    }
}

#[derive(Clone)]
pub struct RolloutPersistenceTelemetry {
    metrics: Option<MetricsClient>,
    sampled: bool,
    totals: Arc<Mutex<ThreadTotals>>,
    finalized: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ThreadTotals {
    pre_filter: RolloutSizeTotals,
    post_filter: RolloutSizeTotals,
}

impl RolloutPersistenceTelemetry {
    pub fn new(thread_id: ThreadId) -> Self {
        let metrics = codex_otel::global();
        let sampled = metrics.is_some() && is_thread_sampled(thread_id);
        Self {
            metrics,
            sampled,
            totals: Arc::new(Mutex::new(ThreadTotals::default())),
            finalized: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled_metrics().is_some()
    }

    pub fn record_batch(&self, measurement: &RolloutPersistenceBatchMeasurement) {
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
                        ("rollout_item_type", item.rollout_item_type),
                        ("encoding", "rollout_item_json_v1"),
                        ("sample_rate", SAMPLE_RATE_LABEL),
                    ],
                );
            } else {
                let _ = metrics.counter(
                    MEASUREMENT_ERROR_METRIC,
                    /*inc*/ 1,
                    &[
                        ("rollout_item_type", item.rollout_item_type),
                        ("phase", "serialize"),
                    ],
                );
            }
        }
        let mut totals = self
            .totals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        add_totals(&mut totals.pre_filter, measurement.pre_filter);
        add_totals(&mut totals.post_filter, measurement.post_filter);
    }

    pub fn record_shutdown(&self) {
        let Some(metrics) = self.enabled_metrics() else {
            return;
        };
        if self.finalized.swap(true, Ordering::Relaxed) {
            return;
        }
        let totals = *self
            .totals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (stage, values) in [
            ("pre_filter", totals.pre_filter),
            ("post_filter", totals.post_filter),
        ] {
            let _ = metrics.histogram(
                THREAD_BYTES_METRIC,
                saturating_i64(values.payload_bytes),
                &[
                    ("stage", stage),
                    ("encoding", "rollout_item_json_v1"),
                    ("sample_rate", SAMPLE_RATE_LABEL),
                    ("finalization", "shutdown"),
                ],
            );
            let _ = metrics.histogram(
                THREAD_ITEMS_METRIC,
                saturating_i64(values.items),
                &[
                    ("stage", stage),
                    ("encoding", "rollout_item_json_v1"),
                    ("sample_rate", SAMPLE_RATE_LABEL),
                    ("finalization", "shutdown"),
                ],
            );
        }
    }

    fn enabled_metrics(&self) -> Option<&MetricsClient> {
        self.sampled.then_some(self.metrics.as_ref()).flatten()
    }
}

fn add_totals(destination: &mut RolloutSizeTotals, source: RolloutSizeTotals) {
    destination.items = destination.items.saturating_add(source.items);
    destination.payload_bytes = destination
        .payload_bytes
        .saturating_add(source.payload_bytes);
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
