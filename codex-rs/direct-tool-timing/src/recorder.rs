//! Records direct-tool invocations, automated reviews, human waits, and batch completion.
//!
//! Prefixes and dependency paths can contain any work kind; this recorder adds tool and
//! review and human-wait intervals, or Unknown for a dependency wait whose history is unavailable.
//! Review callbacks must retain their invocation guard and bracket blocking automated
//! reviews with the review guard's lifetime. Execution-lock admission timing is separate.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use codex_utils_critical_path::CriticalPath;
use codex_utils_critical_path::WorkKind;

/// Completion time and dependency history for one direct invocation.
#[derive(Clone, Debug)]
pub struct CompletedWork {
    pub ended_at: Instant,
    pub path: Option<CriticalPath>,
}

/// Tracks the last-finishing invocation after every invocation in a batch completes.
#[derive(Clone, Default)]
pub struct ToolBatchTiming(Arc<Mutex<BatchState>>);

#[derive(Default)]
struct BatchState {
    pending: usize,
    latest: Option<CompletedWork>,
}

impl ToolBatchTiming {
    /// Returns the batch result only when no invocation remains active.
    pub fn completed(&self) -> Option<CompletedWork> {
        let state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (state.pending == 0).then(|| state.latest.clone()).flatten()
    }

    fn started(&self) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pending += 1;
    }

    fn record(&self, work: CompletedWork) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.pending -= 1;
        if state
            .latest
            .as_ref()
            .is_none_or(|previous| previous.ended_at < work.ended_at)
        {
            state.latest = Some(work);
        }
    }
}

/// Owns direct invocations; duplicate external call IDs invalidate attribution.
#[derive(Clone, Default)]
pub struct DirectToolTimings(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    calls: HashMap<String, Vec<DirectToolInvocation>>,
}

struct DirectToolInvocation {
    identity: Arc<()>,
    path: Option<CriticalPath>,
    segment_started_at: Instant,
    active_auto_reviews: usize,
    active_user_waits: usize,
    batch: ToolBatchTiming,
}

/// Finishes this invocation on drop; later blocking-guard drops cannot affect another call.
pub struct ToolTimingGuard {
    identity: Arc<()>,
    timings: DirectToolTimings,
    call_id: String,
}

#[derive(Clone, Copy)]
enum BlockingKind {
    AutoReview,
    UserWait,
}

/// Records a blocking interval until dropped; user waits take precedence over reviews.
pub struct ToolBlockingTimingGuard {
    kind: BlockingKind,
    identity: Arc<()>,
    timings: DirectToolTimings,
    call_id: String,
}

impl DirectToolInvocation {
    fn record_until(&mut self, now: Instant) {
        // A human answer can block an automated review. Attribute that overlap once,
        // to the human wait; all remaining invocation time is elapsed tool work.
        let kind = if self.active_user_waits > 0 {
            WorkKind::UserWait
        } else if self.active_auto_reviews > 0 {
            WorkKind::AutoReview
        } else {
            WorkKind::Tools
        };
        self.path = self
            .path
            .take()
            .map(|path| path.work(self.segment_started_at, now, kind));
        self.segment_started_at = now;
    }

    fn join(&mut self, start: Instant, end: Instant, dependency: Option<CriticalPath>) {
        self.record_until(start);
        self.path = self.path.take().map(|path| match dependency {
            Some(dependency) => path.join(start, end, dependency),
            None => path.work(start, end, WorkKind::Unknown),
        });
        self.segment_started_at = end;
    }
}

impl DirectToolTimings {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Starts an invocation with its causal prefix, or no attribution when unavailable.
    pub fn start_direct_call(
        &self,
        call_id: &str,
        batch: ToolBatchTiming,
        started_at: Instant,
        prefix: Option<CriticalPath>,
    ) -> ToolTimingGuard {
        batch.started();
        let path = prefix.map(|path| {
            let previous = path.end().unwrap_or(started_at);
            path.work(previous, started_at, WorkKind::Tools)
        });
        let identity = Arc::new(());
        let mut state = self.state();
        let calls = state.calls.entry(call_id.to_string()).or_default();
        let path = if calls.is_empty() {
            path
        } else {
            // External call IDs are not guaranteed unique. Keep each guard's
            // completion separate, but do not guess which invocation a lookup names.
            for work in calls.iter_mut() {
                work.path = None;
            }
            None
        };
        calls.push(DirectToolInvocation {
            identity: Arc::clone(&identity),
            path,
            segment_started_at: started_at,
            active_auto_reviews: 0,
            active_user_waits: 0,
            batch,
        });
        ToolTimingGuard {
            identity,
            timings: self.clone(),
            call_id: call_id.to_string(),
        }
    }

    /// Records a dependency wait for a unique call ID; missing history becomes unknown time.
    pub fn join(
        &self,
        call_id: &str,
        start: Instant,
        end: Instant,
        dependency: Option<CriticalPath>,
    ) {
        if let Some(work) = self.state().unique_work(call_id) {
            work.join(start, end, dependency);
        }
    }

    fn finish(&self, call_id: &str, identity: &Arc<()>) -> Option<CompletedWork> {
        let mut state = self.state();
        let calls = state.calls.get_mut(call_id)?;
        let index = calls
            .iter()
            .position(|work| Arc::ptr_eq(&work.identity, identity))?;
        let mut work = calls.swap_remove(index);
        if calls.is_empty() {
            state.calls.remove(call_id);
        }
        let ended_at = Instant::now();
        work.record_until(ended_at);
        let completed = CompletedWork {
            ended_at,
            path: work.path,
        };
        work.batch.record(completed.clone());
        Some(completed)
    }
}

impl State {
    fn unique_work(&mut self, call_id: &str) -> Option<&mut DirectToolInvocation> {
        let [work] = self.calls.get_mut(call_id)?.as_mut_slice() else {
            return None;
        };
        Some(work)
    }
}

impl ToolTimingGuard {
    /// Starts a review only for this invocation, including after external call ID reuse.
    pub fn start_review(&self) -> Option<ToolBlockingTimingGuard> {
        self.start_blocking(BlockingKind::AutoReview)
    }

    /// Starts a human wait for this invocation, taking precedence over automated review.
    pub fn start_user_wait(&self) -> Option<ToolBlockingTimingGuard> {
        self.start_blocking(BlockingKind::UserWait)
    }

    fn start_blocking(&self, kind: BlockingKind) -> Option<ToolBlockingTimingGuard> {
        let mut state = self.timings.state();
        let work = state
            .calls
            .get_mut(&self.call_id)?
            .iter_mut()
            .find(|work| Arc::ptr_eq(&work.identity, &self.identity))?;
        work.record_until(Instant::now());
        match kind {
            BlockingKind::AutoReview => work.active_auto_reviews += 1,
            BlockingKind::UserWait => work.active_user_waits += 1,
        }
        Some(ToolBlockingTimingGuard {
            kind,
            identity: Arc::clone(&self.identity),
            timings: self.timings.clone(),
            call_id: self.call_id.clone(),
        })
    }

    /// Completes this invocation once; subsequent calls and drop have no effect.
    pub fn finish(&self) -> Option<CompletedWork> {
        self.timings.finish(&self.call_id, &self.identity)
    }
}

impl Drop for ToolTimingGuard {
    fn drop(&mut self) {
        self.finish();
    }
}

impl Drop for ToolBlockingTimingGuard {
    fn drop(&mut self) {
        if let Some(calls) = self.timings.state().calls.get_mut(&self.call_id)
            && let Some(work) = calls
                .iter_mut()
                .find(|work| Arc::ptr_eq(&work.identity, &self.identity))
        {
            work.record_until(Instant::now());
            let counter = match self.kind {
                BlockingKind::AutoReview => &mut work.active_auto_reviews,
                BlockingKind::UserWait => &mut work.active_user_waits,
            };
            *counter = counter.saturating_sub(1);
        }
    }
}

#[cfg(test)]
#[path = "direct_tool_timing_tests.rs"]
mod tests;
