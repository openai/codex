use std::{borrow::Cow, time::SystemTime};

use crate::{
    logs::{AnyValue, LogRecord, Logger, LoggerProvider, Severity},
    InstrumentationScope, Key,
};

/// A no-op implementation of a [`LoggerProvider`].
#[derive(Clone, Debug, Default)]
pub struct NoopLoggerProvider(());

impl NoopLoggerProvider {
    /// Create a new no-op logger provider.
    pub fn new() -> Self {
        NoopLoggerProvider(())
    }
}

impl LoggerProvider for NoopLoggerProvider {
    type Logger = NoopLogger;

    fn logger_with_scope(&self, _scope: InstrumentationScope) -> Self::Logger {
        NoopLogger(())
    }
}

#[derive(Debug, Clone, Default)]
/// A no-operation log record that implements the LogRecord trait.
pub struct NoopLogRecord;

impl LogRecord for NoopLogRecord {
    // Implement the LogRecord trait methods with empty bodies.
    #[inline]
    fn set_event_name(&mut self, _name: &'static str) {}
    #[inline]
    fn set_timestamp(&mut self, _timestamp: SystemTime) {}
    #[inline]
    fn set_observed_timestamp(&mut self, _timestamp: SystemTime) {}
    #[inline]
    fn set_severity_text(&mut self, _text: &'static str) {}
    #[inline]
    fn set_severity_number(&mut self, _number: Severity) {}
    #[inline]
    fn set_body(&mut self, _body: AnyValue) {}
    #[inline]
    fn add_attributes<I, K, V>(&mut self, _attributes: I)
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<Key>,
        V: Into<AnyValue>,
    {
    }
    #[inline]
    fn add_attribute<K, V>(&mut self, _key: K, _value: V)
    where
        K: Into<Key>,
        V: Into<AnyValue>,
    {
    }

    #[inline]
    // Sets the `target` of a record
    fn set_target<T>(&mut self, _target: T)
    where
        T: Into<Cow<'static, str>>,
    {
    }
}

/// A no-op implementation of a [`Logger`]
#[derive(Clone, Debug)]
pub struct NoopLogger(());

impl Logger for NoopLogger {
    type LogRecord = NoopLogRecord;

    fn create_log_record(&self) -> Self::LogRecord {
        NoopLogRecord {}
    }
    fn emit(&self, _record: Self::LogRecord) {}
    #[cfg(feature = "spec_unstable_logs_enabled")]
    fn event_enabled(&self, _level: super::Severity, _target: &str, _name: Option<&str>) -> bool {
        false
    }
}
