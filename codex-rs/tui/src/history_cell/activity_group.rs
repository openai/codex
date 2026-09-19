//! Ordered calls and transcript details for adjacent activity groups.
//!
//! Call positions stay fixed as in-flight calls complete. Prepending older calls shifts the
//! shared detail positions without recreating live clocks or discarding raw terminal input.

use super::ActivityDetails;
use super::HistoryCell;
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct ActivityGroup<T> {
    pub(crate) calls: Vec<T>,
    pub(crate) details: ActivityDetails,
}

impl<T> ActivityGroup<T> {
    pub(crate) fn new(calls: Vec<T>) -> Self {
        Self {
            calls,
            details: ActivityDetails::default(),
        }
    }

    pub(crate) fn push_detail(&mut self, cell: Arc<dyn HistoryCell>) {
        self.details.push(self.calls.len(), cell);
    }

    pub(crate) fn prepend(&mut self, older: Self) {
        self.details.prepend(older.details, older.calls.len());
        self.calls.splice(0..0, older.calls);
    }
}
