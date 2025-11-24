use log::Log;

use crate::Filter;

/// Decorate a [`log::Log`] with record [`Filter`]ing.
///
/// Records that match the filter will be forwarded to the wrapped log.
/// Other records will be ignored.
#[derive(Debug)]
pub struct FilteredLog<T> {
    log: T,
    filter: Filter,
}

impl<T: Log> FilteredLog<T> {
    /// Create a new filtered log.
    pub fn new(log: T, filter: Filter) -> Self {
        Self { log, filter }
    }
}

impl<T: Log> Log for FilteredLog<T> {
    /// Determines if a log message with the specified metadata would be logged.
    ///
    /// For the wrapped log, this returns `true` only if both the filter and the wrapped log return `true`.
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.filter.enabled(metadata) && self.log.enabled(metadata)
    }

    /// Logs the record.
    ///
    /// Forwards the record to the wrapped log, but only if the record matches the filter.
    fn log(&self, record: &log::Record<'_>) {
        if self.filter.matches(record) {
            self.log.log(record);
        }
    }

    /// Flushes any buffered records.
    ///
    /// Forwards directly to the wrapped log.
    fn flush(&self) {
        self.log.flush();
    }
}
