//! Speculative execution for optimistic tool execution.
//!
//! This module provides support for speculative execution, where tools are
//! executed optimistically before the full response is confirmed. If the model
//! reconsiders, speculative results can be rolled back.
//!
//! ## Architecture
//!
//! Speculative execution works in three phases:
//!
//! 1. **Speculation Start**: When safe, read-only tools complete during streaming,
//!    they are marked as speculative and execution begins immediately.
//!
//! 2. **Commitment**: When the model's message completes (message_stop) without
//!    reconsidering, speculative results are committed.
//!
//! 3. **Rollback**: If the model reconsiders (changes tool calls or issues new
//!    instructions), speculative results are rolled back and re-executed.
//!
//! ## Tool Safety
//!
//! Only tools marked as `ConcurrencySafety::Safe` (read-only operations) can
//! be executed speculatively. Write operations are never speculative.
//!
//! ## Example
//!
//! ```ignore
//! let mut speculation = SpeculationTracker::new();
//!
//! // During streaming - mark tool as speculative
//! speculation.start_speculation("spec-1", vec!["call-1", "call-2"]);
//!
//! // After message_stop - commit if no reconsideration
//! speculation.commit("spec-1")?;
//!
//! // Or on reconsideration - rollback
//! speculation.rollback("spec-1", "Model changed tool calls");
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use cocode_protocol::LoopEvent;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::info;
use tracing::warn;

// ============================================================================
// Constants
// ============================================================================

/// Maximum time to wait for speculative execution before auto-commit (seconds).
pub const SPECULATION_TIMEOUT_SECS: u64 = 30;

/// Maximum number of pending speculation batches.
pub const MAX_PENDING_SPECULATIONS: usize = 10;

// ============================================================================
// SpeculationState
// ============================================================================

/// State of a speculation batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeculationState {
    /// Speculation is in progress.
    Pending,
    /// Speculation has been committed.
    Committed,
    /// Speculation has been rolled back.
    RolledBack,
}

impl SpeculationState {
    /// Get the state as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            SpeculationState::Pending => "pending",
            SpeculationState::Committed => "committed",
            SpeculationState::RolledBack => "rolled_back",
        }
    }
}

impl std::fmt::Display for SpeculationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// SpeculativeToolCall
// ============================================================================

/// A speculative tool call with its cached result.
#[derive(Debug, Clone)]
pub struct SpeculativeToolCall {
    /// Tool call ID.
    pub call_id: String,
    /// Tool name.
    pub name: String,
    /// Cached result (if execution completed).
    pub result: Option<SpeculativeResult>,
    /// When execution started.
    pub started_at: Instant,
}

/// Result of a speculative tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeResult {
    /// Output content.
    pub content: String,
    /// Whether the tool returned an error.
    pub is_error: bool,
}

// ============================================================================
// SpeculationBatch
// ============================================================================

/// A batch of speculative tool calls.
#[derive(Debug)]
pub struct SpeculationBatch {
    /// Unique identifier for this batch.
    pub id: String,
    /// Current state of the speculation.
    pub state: SpeculationState,
    /// Tool calls in this batch.
    pub tool_calls: HashMap<String, SpeculativeToolCall>,
    /// When the speculation started.
    pub started_at: Instant,
    /// Rollback reason (if rolled back).
    pub rollback_reason: Option<String>,
}

impl SpeculationBatch {
    /// Create a new speculation batch.
    pub fn new(id: impl Into<String>, call_ids: Vec<String>) -> Self {
        let now = Instant::now();
        let tool_calls = call_ids
            .into_iter()
            .map(|call_id| {
                (
                    call_id.clone(),
                    SpeculativeToolCall {
                        call_id: call_id.clone(),
                        name: String::new(), // Will be set when execution starts
                        result: None,
                        started_at: now,
                    },
                )
            })
            .collect();

        Self {
            id: id.into(),
            state: SpeculationState::Pending,
            tool_calls,
            started_at: now,
            rollback_reason: None,
        }
    }

    /// Check if all tool calls have completed.
    pub fn is_complete(&self) -> bool {
        self.tool_calls.values().all(|tc| tc.result.is_some())
    }

    /// Get the number of completed tool calls.
    pub fn completed_count(&self) -> usize {
        self.tool_calls
            .values()
            .filter(|tc| tc.result.is_some())
            .count()
    }

    /// Set the result for a tool call.
    pub fn set_result(&mut self, call_id: &str, result: SpeculativeResult) {
        if let Some(tc) = self.tool_calls.get_mut(call_id) {
            tc.result = Some(result);
        }
    }

    /// Set the tool name for a call.
    pub fn set_tool_name(&mut self, call_id: &str, name: &str) {
        if let Some(tc) = self.tool_calls.get_mut(call_id) {
            tc.name = name.to_string();
        }
    }

    /// Get all call IDs in this batch.
    pub fn call_ids(&self) -> Vec<String> {
        self.tool_calls.keys().cloned().collect()
    }

    /// Check if the speculation has timed out.
    pub fn is_timed_out(&self) -> bool {
        self.started_at.elapsed().as_secs() > SPECULATION_TIMEOUT_SECS
    }
}

// ============================================================================
// SpeculationTracker
// ============================================================================

/// Tracks speculative tool executions across the session.
///
/// The tracker manages speculation batches, allowing tools to be executed
/// optimistically during streaming and then committed or rolled back based
/// on the final model response.
#[derive(Debug)]
pub struct SpeculationTracker {
    /// Active speculation batches keyed by speculation ID.
    batches: Arc<Mutex<HashMap<String, SpeculationBatch>>>,
    /// Event sender for emitting speculation events.
    event_tx: Option<mpsc::Sender<LoopEvent>>,
    /// Counter for generating speculation IDs.
    next_id: Arc<Mutex<u64>>,
}

impl SpeculationTracker {
    /// Create a new speculation tracker.
    pub fn new() -> Self {
        Self {
            batches: Arc::new(Mutex::new(HashMap::new())),
            event_tx: None,
            next_id: Arc::new(Mutex::new(0)),
        }
    }

    /// Create a tracker with an event sender.
    pub fn with_event_tx(mut self, tx: mpsc::Sender<LoopEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Generate a new speculation ID.
    pub async fn next_speculation_id(&self) -> String {
        let mut counter = self.next_id.lock().await;
        *counter += 1;
        format!("spec-{}", *counter)
    }

    /// Start a new speculation batch.
    ///
    /// Returns the speculation ID.
    pub async fn start_speculation(&self, call_ids: Vec<String>) -> String {
        let spec_id = self.next_speculation_id().await;
        let batch = SpeculationBatch::new(&spec_id, call_ids.clone());

        self.batches.lock().await.insert(spec_id.clone(), batch);

        info!(
            speculation_id = %spec_id,
            call_count = call_ids.len(),
            "Started speculation batch"
        );

        // Emit event
        self.emit_event(LoopEvent::SpeculativeStarted {
            speculation_id: spec_id.clone(),
            tool_calls: call_ids,
        })
        .await;

        spec_id
    }

    /// Start a speculation batch with a specific ID.
    pub async fn start_speculation_with_id(&self, spec_id: &str, call_ids: Vec<String>) {
        let batch = SpeculationBatch::new(spec_id, call_ids.clone());
        self.batches.lock().await.insert(spec_id.to_string(), batch);

        info!(
            speculation_id = %spec_id,
            call_count = call_ids.len(),
            "Started speculation batch"
        );

        // Emit event
        self.emit_event(LoopEvent::SpeculativeStarted {
            speculation_id: spec_id.to_string(),
            tool_calls: call_ids,
        })
        .await;
    }

    /// Record a tool result for a speculation batch.
    pub async fn record_result(
        &self,
        speculation_id: &str,
        call_id: &str,
        name: &str,
        result: SpeculativeResult,
    ) {
        let mut batches = self.batches.lock().await;
        if let Some(batch) = batches.get_mut(speculation_id) {
            if batch.state == SpeculationState::Pending {
                batch.set_tool_name(call_id, name);
                batch.set_result(call_id, result);
                debug!(
                    speculation_id = %speculation_id,
                    call_id = %call_id,
                    "Recorded speculative result"
                );
            }
        }
    }

    /// Commit a speculation batch.
    ///
    /// Returns the committed results if successful.
    pub async fn commit(&self, speculation_id: &str) -> Option<Vec<(String, SpeculativeResult)>> {
        let mut batches = self.batches.lock().await;
        let batch = batches.get_mut(speculation_id)?;

        if batch.state != SpeculationState::Pending {
            warn!(
                speculation_id = %speculation_id,
                state = %batch.state,
                "Cannot commit speculation in non-pending state"
            );
            return None;
        }

        batch.state = SpeculationState::Committed;
        let committed_count = batch.completed_count() as i32;

        // Collect results
        let results: Vec<_> = batch
            .tool_calls
            .values()
            .filter_map(|tc| tc.result.clone().map(|r| (tc.call_id.clone(), r)))
            .collect();

        info!(
            speculation_id = %speculation_id,
            committed_count = committed_count,
            "Committed speculation batch"
        );

        // Emit event (drop lock first)
        let event = LoopEvent::SpeculativeCommitted {
            speculation_id: speculation_id.to_string(),
            committed_count,
        };
        drop(batches);
        self.emit_event(event).await;

        Some(results)
    }

    /// Rollback a speculation batch.
    ///
    /// Returns the call IDs that were rolled back.
    pub async fn rollback(&self, speculation_id: &str, reason: &str) -> Vec<String> {
        let mut batches = self.batches.lock().await;
        let batch = match batches.get_mut(speculation_id) {
            Some(b) => b,
            None => return Vec::new(),
        };

        if batch.state != SpeculationState::Pending {
            warn!(
                speculation_id = %speculation_id,
                state = %batch.state,
                "Cannot rollback speculation in non-pending state"
            );
            return Vec::new();
        }

        batch.state = SpeculationState::RolledBack;
        batch.rollback_reason = Some(reason.to_string());

        let rolled_back_calls = batch.call_ids();

        info!(
            speculation_id = %speculation_id,
            reason = %reason,
            calls_count = rolled_back_calls.len(),
            "Rolled back speculation batch"
        );

        // Emit event (drop lock first)
        let event = LoopEvent::SpeculativeRolledBack {
            speculation_id: speculation_id.to_string(),
            reason: reason.to_string(),
            rolled_back_calls: rolled_back_calls.clone(),
        };
        drop(batches);
        self.emit_event(event).await;

        rolled_back_calls
    }

    /// Get the state of a speculation batch.
    pub async fn get_state(&self, speculation_id: &str) -> Option<SpeculationState> {
        self.batches
            .lock()
            .await
            .get(speculation_id)
            .map(|b| b.state)
    }

    /// Check if a call ID is part of any pending speculation.
    pub async fn is_speculative(&self, call_id: &str) -> bool {
        self.batches.lock().await.values().any(|batch| {
            batch.state == SpeculationState::Pending && batch.tool_calls.contains_key(call_id)
        })
    }

    /// Get the speculation ID for a call ID.
    pub async fn get_speculation_id(&self, call_id: &str) -> Option<String> {
        self.batches
            .lock()
            .await
            .iter()
            .find_map(|(spec_id, batch)| {
                if batch.tool_calls.contains_key(call_id) {
                    Some(spec_id.clone())
                } else {
                    None
                }
            })
    }

    /// Commit all pending speculations.
    pub async fn commit_all(&self) -> i32 {
        let pending_ids: Vec<_> = {
            self.batches
                .lock()
                .await
                .iter()
                .filter(|(_, batch)| batch.state == SpeculationState::Pending)
                .map(|(id, _)| id.clone())
                .collect()
        };

        let mut committed = 0;
        for spec_id in pending_ids {
            if self.commit(&spec_id).await.is_some() {
                committed += 1;
            }
        }

        committed
    }

    /// Rollback all pending speculations.
    pub async fn rollback_all(&self, reason: &str) -> i32 {
        let pending_ids: Vec<_> = {
            self.batches
                .lock()
                .await
                .iter()
                .filter(|(_, batch)| batch.state == SpeculationState::Pending)
                .map(|(id, _)| id.clone())
                .collect()
        };

        let mut rolled_back = 0;
        for spec_id in pending_ids {
            if !self.rollback(&spec_id, reason).await.is_empty() {
                rolled_back += 1;
            }
        }

        rolled_back
    }

    /// Clean up completed speculations.
    pub async fn cleanup_completed(&self) {
        let mut batches = self.batches.lock().await;
        batches.retain(|_, batch| batch.state == SpeculationState::Pending);
    }

    /// Get statistics about current speculation state.
    pub async fn stats(&self) -> SpeculationStats {
        let batches = self.batches.lock().await;
        let pending = batches
            .values()
            .filter(|b| b.state == SpeculationState::Pending)
            .count();
        let committed = batches
            .values()
            .filter(|b| b.state == SpeculationState::Committed)
            .count();
        let rolled_back = batches
            .values()
            .filter(|b| b.state == SpeculationState::RolledBack)
            .count();

        SpeculationStats {
            pending,
            committed,
            rolled_back,
            total: batches.len(),
        }
    }

    /// Emit a loop event.
    async fn emit_event(&self, event: LoopEvent) {
        if let Some(tx) = &self.event_tx {
            if let Err(e) = tx.send(event).await {
                debug!("Failed to send speculation event: {e}");
            }
        }
    }
}

impl Default for SpeculationTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SpeculationStats
// ============================================================================

/// Statistics about speculation state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpeculationStats {
    /// Number of pending speculations.
    pub pending: usize,
    /// Number of committed speculations.
    pub committed: usize,
    /// Number of rolled back speculations.
    pub rolled_back: usize,
    /// Total number of speculation batches.
    pub total: usize,
}

impl SpeculationStats {
    /// Check if there are any active speculations.
    pub fn has_pending(&self) -> bool {
        self.pending > 0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_speculation_lifecycle() {
        let tracker = SpeculationTracker::new();

        // Start speculation
        let spec_id = tracker
            .start_speculation(vec!["call-1".to_string(), "call-2".to_string()])
            .await;
        assert!(spec_id.starts_with("spec-"));

        // Record results
        tracker
            .record_result(
                &spec_id,
                "call-1",
                "Read",
                SpeculativeResult {
                    content: "file contents".to_string(),
                    is_error: false,
                },
            )
            .await;

        // Check state
        assert_eq!(
            tracker.get_state(&spec_id).await,
            Some(SpeculationState::Pending)
        );
        assert!(tracker.is_speculative("call-1").await);
        assert!(tracker.is_speculative("call-2").await);
        assert!(!tracker.is_speculative("call-3").await);

        // Commit
        let results = tracker.commit(&spec_id).await;
        assert!(results.is_some());
        assert_eq!(results.unwrap().len(), 1); // Only one result recorded

        // Check committed state
        assert_eq!(
            tracker.get_state(&spec_id).await,
            Some(SpeculationState::Committed)
        );
    }

    #[tokio::test]
    async fn test_speculation_rollback() {
        let tracker = SpeculationTracker::new();

        let spec_id = tracker.start_speculation(vec!["call-1".to_string()]).await;

        tracker
            .record_result(
                &spec_id,
                "call-1",
                "Read",
                SpeculativeResult {
                    content: "data".to_string(),
                    is_error: false,
                },
            )
            .await;

        // Rollback
        let rolled_back = tracker.rollback(&spec_id, "Model reconsideration").await;
        assert_eq!(rolled_back.len(), 1);
        assert_eq!(rolled_back[0], "call-1");

        // Check state
        assert_eq!(
            tracker.get_state(&spec_id).await,
            Some(SpeculationState::RolledBack)
        );
    }

    #[tokio::test]
    async fn test_speculation_stats() {
        let tracker = SpeculationTracker::new();

        let spec_id1 = tracker.start_speculation(vec!["call-1".to_string()]).await;
        let spec_id2 = tracker.start_speculation(vec!["call-2".to_string()]).await;

        let stats = tracker.stats().await;
        assert_eq!(stats.pending, 2);
        assert_eq!(stats.committed, 0);
        assert_eq!(stats.total, 2);

        tracker.commit(&spec_id1).await;

        let stats = tracker.stats().await;
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.committed, 1);

        tracker.rollback(&spec_id2, "test").await;

        let stats = tracker.stats().await;
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.committed, 1);
        assert_eq!(stats.rolled_back, 1);
    }

    #[tokio::test]
    async fn test_commit_all() {
        let tracker = SpeculationTracker::new();

        tracker.start_speculation(vec!["call-1".to_string()]).await;
        tracker.start_speculation(vec!["call-2".to_string()]).await;

        let committed = tracker.commit_all().await;
        assert_eq!(committed, 2);

        let stats = tracker.stats().await;
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.committed, 2);
    }

    #[tokio::test]
    async fn test_rollback_all() {
        let tracker = SpeculationTracker::new();

        tracker.start_speculation(vec!["call-1".to_string()]).await;
        tracker.start_speculation(vec!["call-2".to_string()]).await;

        let rolled_back = tracker.rollback_all("stream error").await;
        assert_eq!(rolled_back, 2);

        let stats = tracker.stats().await;
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.rolled_back, 2);
    }

    #[tokio::test]
    async fn test_cleanup_completed() {
        let tracker = SpeculationTracker::new();

        let spec_id = tracker.start_speculation(vec!["call-1".to_string()]).await;
        tracker.commit(&spec_id).await;

        assert_eq!(tracker.stats().await.total, 1);

        tracker.cleanup_completed().await;

        assert_eq!(tracker.stats().await.total, 0);
    }

    #[test]
    fn test_speculation_state_display() {
        assert_eq!(SpeculationState::Pending.as_str(), "pending");
        assert_eq!(SpeculationState::Committed.as_str(), "committed");
        assert_eq!(SpeculationState::RolledBack.as_str(), "rolled_back");
    }
}
