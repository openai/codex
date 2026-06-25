use std::collections::HashSet;
use std::sync::LazyLock;
use std::sync::Mutex;

use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStatus;
use codex_otel::MetricsClient;
use codex_protocol::models::MessagePhase;

const RESPONSE_BYTES_METRIC: &str = "codex.app_server.thread_read.response_bytes";
const COMPLETED_TURNS_METRIC: &str = "codex.app_server.thread_read.completed_turns";
const COMPLETED_TURN_ITEMS_METRIC: &str = "codex.app_server.thread_read.completed_turn_items";
const SAMPLE_DENOMINATOR: u64 = 100;
const SAMPLE_RATE_LABEL: &str = "0.01";
static MEASURED_THREADS: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ThreadReadCounts {
    completed_turns: u64,
    completed_turn_items: u64,
}

pub(crate) struct ThreadReadMeasurement {
    metrics: MetricsClient,
    counts: ThreadReadCounts,
}

impl ThreadReadMeasurement {
    pub(crate) fn prepare(thread: &Thread) -> Option<Self> {
        if !matches!(thread.status, ThreadStatus::NotLoaded) || !is_thread_sampled(&thread.id) {
            return None;
        }
        let metrics = codex_otel::global()?;
        let counts = count_completed_turns(&thread.turns);
        if counts.completed_turns == 0
            || !MEASURED_THREADS
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(thread.id.clone())
        {
            return None;
        }
        Some(Self { metrics, counts })
    }

    pub(crate) fn record_serialized_response(self, response_bytes: usize) {
        let tags = [
            ("encoding", "app_server_transport_json"),
            ("sample_rate", SAMPLE_RATE_LABEL),
            ("source", "cold_thread_read"),
        ];
        let _ = self.metrics.histogram(
            RESPONSE_BYTES_METRIC,
            saturating_i64(response_bytes as u64),
            &tags,
        );
        let _ = self.metrics.histogram(
            COMPLETED_TURNS_METRIC,
            saturating_i64(self.counts.completed_turns),
            &tags,
        );
        let _ = self.metrics.histogram(
            COMPLETED_TURN_ITEMS_METRIC,
            saturating_i64(self.counts.completed_turn_items),
            &tags,
        );
    }
}

fn count_completed_turns(turns: &[Turn]) -> ThreadReadCounts {
    let mut counts = ThreadReadCounts::default();
    for turn in turns
        .iter()
        .filter(|turn| is_completed_user_assistant_turn(turn))
    {
        counts.completed_turns = counts.completed_turns.saturating_add(1);
        counts.completed_turn_items = counts
            .completed_turn_items
            .saturating_add(turn.items.len() as u64);
    }
    counts
}

fn is_completed_user_assistant_turn(turn: &Turn) -> bool {
    turn.status == TurnStatus::Completed
        && turn
            .items
            .iter()
            .any(|item| matches!(item, ThreadItem::UserMessage { .. }))
        && turn.items.iter().any(|item| {
            matches!(
                item,
                ThreadItem::AgentMessage {
                    phase: None | Some(MessagePhase::FinalAnswer),
                    ..
                }
            )
        })
}

fn is_thread_sampled(thread_id: &str) -> bool {
    let hash = thread_id
        .bytes()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    hash % SAMPLE_DENOMINATOR == 0
}

fn saturating_i64(value: u64) -> i64 {
    value.try_into().unwrap_or(i64::MAX)
}

#[cfg(test)]
#[path = "thread_read_metrics_tests.rs"]
mod tests;
