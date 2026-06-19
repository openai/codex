//! Turn-local, model-visible tool latency exported through Responses client metadata.

use std::mem;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;

const TOOL_TIMING_VERSION: u8 = 1;
pub(crate) const TOOL_TIMING_KEY: &str = "tool_timing";

#[derive(Clone, Debug)]
pub(crate) enum ToolTimingSource {
    Direct,
    CodeMode {
        cell_id: String,
        runtime_tool_call_id: String,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct ToolTimingCall {
    pub(crate) call_id: String,
    pub(crate) tool_name: String,
    pub(crate) source: ToolTimingSource,
}

#[derive(Clone, Debug)]
pub(crate) struct ToolTimingState {
    inner: Arc<Mutex<ToolTimingStateInner>>,
}

#[derive(Debug)]
struct ToolTimingStateInner {
    origin: Instant,
    next_entry_id: u64,
    next_report_id: u64,
    calls: Vec<ToolTimingCallState>,
}

#[derive(Debug)]
struct ToolTimingCallState {
    entry_id: u64,
    call_id: String,
    tool_name: String,
    source: ToolTimingSource,
    started_us: u64,
    execution_started_us: Option<u64>,
    completed_us: Option<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct ToolTimingMarker {
    state: ToolTimingState,
    entry_id: u64,
}

#[derive(Debug)]
pub(crate) struct ToolTimingGuard {
    marker: ToolTimingMarker,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ToolTimingReport {
    version: u8,
    report_id: u64,
    tool_active_s: f64,
    calls: Vec<ToolCallTimingReport>,
}

#[derive(Clone, Debug, Serialize)]
struct ToolCallTimingReport {
    call_id: String,
    tool_name: String,
    source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    cell_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_tool_call_id: Option<String>,
    started_s: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_started_s: Option<f64>,
    completed_s: f64,
    dispatch_s: f64,
    handler_s: f64,
    total_s: f64,
}

impl Default for ToolTimingState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ToolTimingStateInner {
                origin: Instant::now(),
                next_entry_id: 0,
                next_report_id: 0,
                calls: Vec::new(),
            })),
        }
    }
}

impl ToolTimingState {
    pub(crate) fn start_call(&self, call: ToolTimingCall) -> ToolTimingGuard {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entry_id = state.next_entry_id;
        state.next_entry_id += 1;
        let started_us = state.elapsed_us();
        state.calls.push(ToolTimingCallState {
            entry_id,
            call_id: call.call_id,
            tool_name: call.tool_name,
            source: call.source,
            started_us,
            execution_started_us: None,
            completed_us: None,
        });
        ToolTimingGuard {
            marker: ToolTimingMarker {
                state: self.clone(),
                entry_id,
            },
        }
    }

    pub(crate) fn take_report(&self) -> ToolTimingReport {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let report_id = state.next_report_id;
        state.next_report_id += 1;

        let mut completed_calls = Vec::new();
        let mut pending_calls = Vec::new();
        for call in mem::take(&mut state.calls) {
            match call.completed_us {
                Some(completed_us) => completed_calls.push((call, completed_us)),
                None => pending_calls.push(call),
            }
        }
        state.calls = pending_calls;

        // Nested code-mode calls are diagnostic; their enclosing direct call owns the latency.
        let mut intervals = completed_calls
            .iter()
            .filter(|(call, _)| matches!(&call.source, ToolTimingSource::Direct))
            .map(|(call, completed_us)| (call.started_us, *completed_us))
            .collect::<Vec<_>>();
        intervals.sort_unstable_by_key(|interval| interval.0);
        let mut tool_active_us = 0_u64;
        if let Some((mut current_start_us, mut current_end_us)) = intervals.first().copied() {
            for (started_us, completed_us) in intervals.into_iter().skip(1) {
                if started_us <= current_end_us {
                    current_end_us = current_end_us.max(completed_us);
                } else {
                    tool_active_us += current_end_us - current_start_us;
                    current_start_us = started_us;
                    current_end_us = completed_us;
                }
            }
            tool_active_us += current_end_us - current_start_us;
        }

        let mut calls = completed_calls
            .into_iter()
            .map(|(call, completed_us)| {
                let execution_started_us = call.execution_started_us;
                let dispatch_completed_us = execution_started_us.unwrap_or(completed_us);
                let (source, cell_id, runtime_tool_call_id) = match call.source {
                    ToolTimingSource::Direct => ("direct", None, None),
                    ToolTimingSource::CodeMode {
                        cell_id,
                        runtime_tool_call_id,
                    } => ("code_mode", Some(cell_id), Some(runtime_tool_call_id)),
                };
                ToolCallTimingReport {
                    call_id: call.call_id,
                    tool_name: call.tool_name,
                    source,
                    cell_id,
                    runtime_tool_call_id,
                    started_s: micros_to_seconds(call.started_us),
                    execution_started_s: execution_started_us.map(micros_to_seconds),
                    completed_s: micros_to_seconds(completed_us),
                    dispatch_s: micros_to_seconds(dispatch_completed_us - call.started_us),
                    handler_s: micros_to_seconds(completed_us - dispatch_completed_us),
                    total_s: micros_to_seconds(completed_us - call.started_us),
                }
            })
            .collect::<Vec<_>>();
        calls.sort_by(|left, right| left.started_s.total_cmp(&right.started_s));

        ToolTimingReport {
            version: TOOL_TIMING_VERSION,
            report_id,
            tool_active_s: micros_to_seconds(tool_active_us),
            calls,
        }
    }

    fn mark_execution_started(&self, entry_id: u64) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let execution_started_us = state.elapsed_us();
        if let Some(call) = state
            .calls
            .iter_mut()
            .find(|call| call.entry_id == entry_id)
            && call.completed_us.is_none()
            && call.execution_started_us.is_none()
        {
            call.execution_started_us = Some(execution_started_us);
        }
    }

    fn complete_call(&self, entry_id: u64) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let completed_us = state.elapsed_us();
        if let Some(call) = state
            .calls
            .iter_mut()
            .find(|call| call.entry_id == entry_id)
        {
            call.completed_us = Some(completed_us);
        }
    }
}

impl ToolTimingStateInner {
    fn elapsed_us(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_micros()).unwrap_or(u64::MAX)
    }
}

impl ToolTimingGuard {
    pub(crate) fn marker(&self) -> ToolTimingMarker {
        self.marker.clone()
    }
}

impl Drop for ToolTimingGuard {
    fn drop(&mut self) {
        self.marker.state.complete_call(self.marker.entry_id);
    }
}

impl ToolTimingMarker {
    pub(crate) fn mark_execution_started(&self) {
        self.state.mark_execution_started(self.entry_id);
    }
}

fn micros_to_seconds(micros: u64) -> f64 {
    micros as f64 / 1_000_000.0
}

#[cfg(test)]
#[path = "tool_timing_tests.rs"]
mod tests;
