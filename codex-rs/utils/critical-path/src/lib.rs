//! Persistent dependency paths and exclusive attribution over the observed turn interval.
//! History is depth-limited; formatting and destruction never recurse through the chain.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

const MAX_PATH_DEPTH: usize = 16_384;

/// Category attributed to an observed segment of dependency work.
#[derive(Clone, Copy, Debug)]
pub enum WorkKind {
    /// Model inference work on the selected dependency path.
    Inference,
    /// Automatic review work, such as reviewing a tool call before execution.
    AutoReview,
    /// Time awaiting a human answer or approval on the selected dependency path.
    UserWait,
    /// Tool execution work, including work after a dependency join resumes.
    /// This is an aggregate category; tool-specific subtypes such as browser or
    /// computer use are not represented by this calculator.
    Tools,
    /// Time within the observed turn interval not covered by a recorded work segment.
    Overhead,
    /// Work whose category is unavailable, including history collapsed at the depth limit.
    Unknown,
}

/// An immutable observed dependency path. Forks share their prefix; a join
/// retains only the dependency that actually released the waiting operation.
/// Equality compares path identity rather than the contents of each segment.
#[derive(Clone, Default)]
pub struct CriticalPath(Option<Arc<Node>>);

impl std::fmt::Debug for CriticalPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Summarize the tail without traversing shared dependency history.
        f.debug_struct("CriticalPath")
            .field("depth", &self.0.as_ref().map_or(0, |node| node.depth))
            .field("end", &self.end())
            .finish()
    }
}

impl PartialEq for CriticalPath {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        }
    }
}

impl Eq for CriticalPath {}

struct Node {
    previous: CriticalPath,
    kind: WorkKind,
    start: Instant,
    end: Instant,
    origin: Instant,
    depth: usize,
}

/// Exclusive time attributed to each category within the requested interval.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PathDurations {
    pub inference: Duration,
    pub auto_review: Duration,
    pub user_wait: Duration,
    pub tools: Duration,
    pub overhead: Duration,
    pub unknown: Duration,
}

impl PathDurations {
    fn record(&mut self, kind: WorkKind, duration: Duration) {
        let total = match kind {
            WorkKind::Inference => &mut self.inference,
            WorkKind::AutoReview => &mut self.auto_review,
            WorkKind::UserWait => &mut self.user_wait,
            WorkKind::Tools => &mut self.tools,
            WorkKind::Overhead => &mut self.overhead,
            WorkKind::Unknown => &mut self.unknown,
        };
        *total = total.saturating_add(duration);
    }
}

impl CriticalPath {
    /// End of the last recorded segment, or `None` for an empty path.
    pub fn end(&self) -> Option<Instant> {
        self.0.as_ref().map(|node| node.end)
    }

    /// Retain the later-finishing path, choosing `self` on a tie.
    pub fn latest(self, other: Self) -> Self {
        if other.end() > self.end() {
            other
        } else {
            self
        }
    }

    /// Append work after the existing path ends, clipping overlap and ignoring empty segments.
    /// Exceeding the history limit collapses the retained interval into unknown work.
    pub fn work(self, start: Instant, end: Instant, kind: WorkKind) -> Self {
        let start = self.end().map_or(start, |previous| previous.max(start));
        if start >= end {
            return self;
        }
        let origin = self.0.as_ref().map_or(start, |node| node.origin);
        let previous_depth = self.0.as_ref().map_or(0, |node| node.depth);
        let depth = previous_depth + 1;
        if depth > MAX_PATH_DEPTH {
            return Self::default().work(origin, end, WorkKind::Unknown);
        }
        Self(Some(Arc::new(Node {
            previous: self,
            kind,
            start,
            end,
            origin,
            depth,
        })))
    }

    /// Select the full branch that released this join, not just its exposed
    /// waiting tail. Both branches carry their common causal prefix.
    pub fn join(self, start: Instant, end: Instant, dependency: Self) -> Self {
        let path = self.latest(dependency);
        let resumed_at = path.end().unwrap_or(start).max(start);
        path.work(resumed_at, end, WorkKind::Tools)
    }

    /// Attribute the requested interval exclusively, counting unrecorded gaps as overhead.
    pub fn durations(&self, start: Instant, end: Instant) -> PathDurations {
        let mut totals = PathDurations::default();
        let mut path = self;
        let mut end = end;
        while let Some(node) = path.0.as_ref() {
            if end <= start {
                break;
            }
            let segment_end = end.min(node.end);
            let segment_start = start.max(node.start);
            if end > node.end {
                totals.record(WorkKind::Overhead, end.duration_since(start.max(node.end)));
            }
            if segment_start < segment_end {
                totals.record(node.kind, segment_end.duration_since(segment_start));
            }
            end = end.min(node.start);
            path = &node.previous;
        }
        if start < end {
            totals.record(WorkKind::Overhead, end.duration_since(start));
        }
        totals
    }
}

// Long sequential turns must not recursively destroy one Arc per operation.
// into_inner consumes each reference atomically: concurrent releases cannot
// leave an Err(Arc) whose subsequent drop recursively destroys the next node.
impl Drop for Node {
    fn drop(&mut self) {
        let mut previous = self.previous.0.take();
        while let Some(node) = previous {
            let Some(mut node) = Arc::into_inner(node) else {
                break;
            };
            previous = node.previous.0.take();
        }
    }
}

#[cfg(test)]
#[path = "critical_path_tests.rs"]
mod tests;
