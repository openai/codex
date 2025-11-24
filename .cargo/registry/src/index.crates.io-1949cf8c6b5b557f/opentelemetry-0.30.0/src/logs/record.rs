use crate::{Key, StringValue};

use crate::{SpanId, TraceFlags, TraceId};

use std::{borrow::Cow, collections::HashMap, time::SystemTime};

/// SDK implemented trait for managing log records
pub trait LogRecord {
    /// Sets the `event_name` of a record
    fn set_event_name(&mut self, name: &'static str);

    /// Sets the `target` of a record.
    /// Currently, both `opentelemetry-appender-tracing` and `opentelemetry-appender-log` create a single logger
    /// with a scope that doesn't accurately reflect the component emitting the logs.
    /// Exporters MAY use this field to override the `instrumentation_scope.name`.
    fn set_target<T>(&mut self, _target: T)
    where
        T: Into<Cow<'static, str>>;

    /// Sets the time when the event occurred measured by the origin clock, i.e. the time at the source.
    fn set_timestamp(&mut self, timestamp: SystemTime);

    /// Sets the observed event timestamp.
    fn set_observed_timestamp(&mut self, timestamp: SystemTime);

    /// Sets severity as text.
    fn set_severity_text(&mut self, text: &'static str);

    /// Sets severity as a numeric value.
    fn set_severity_number(&mut self, number: Severity);

    /// Sets the message body of the log.
    fn set_body(&mut self, body: AnyValue);

    /// Adds multiple attributes.
    fn add_attributes<I, K, V>(&mut self, attributes: I)
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<Key>,
        V: Into<AnyValue>;

    /// Adds a single attribute.
    fn add_attribute<K, V>(&mut self, key: K, value: V)
    where
        K: Into<Key>,
        V: Into<AnyValue>;

    /// Sets the trace context of the log.
    fn set_trace_context(
        &mut self,
        trace_id: TraceId,
        span_id: SpanId,
        trace_flags: Option<TraceFlags>,
    ) {
        let _ = trace_id;
        let _ = span_id;
        let _ = trace_flags;
    }
}

/// Value types for representing arbitrary values in a log record.
/// Note: The `tracing` and `log` crates only support basic types that can be
/// converted to these core variants: `i64`, `f64`, `StringValue`, and `bool`.
/// Any complex and custom types are supported through their Debug implementation,
/// and converted to String. More complex types (`Bytes`, `ListAny`, and `Map`) are
/// included here to meet specification requirements and are available to support
/// custom appenders that may be implemented for other logging crates.
/// These types allow for handling dynamic data structures, so keep in mind the
/// potential performance overhead of using boxed vectors and maps in appenders.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AnyValue {
    /// An integer value
    Int(i64),
    /// A double value
    Double(f64),
    /// A string value
    String(StringValue),
    /// A boolean value
    Boolean(bool),
    /// A byte array
    Bytes(Box<Vec<u8>>),
    /// An array of `Any` values
    ListAny(Box<Vec<AnyValue>>),
    /// A map of string keys to `Any` values, arbitrarily nested.
    Map(Box<HashMap<Key, AnyValue>>),
}

macro_rules! impl_trivial_from {
    ($t:ty, $variant:path) => {
        impl From<$t> for AnyValue {
            fn from(val: $t) -> AnyValue {
                $variant(val.into())
            }
        }
    };
}

impl_trivial_from!(i8, AnyValue::Int);
impl_trivial_from!(i16, AnyValue::Int);
impl_trivial_from!(i32, AnyValue::Int);
impl_trivial_from!(i64, AnyValue::Int);

impl_trivial_from!(u8, AnyValue::Int);
impl_trivial_from!(u16, AnyValue::Int);
impl_trivial_from!(u32, AnyValue::Int);

impl_trivial_from!(f64, AnyValue::Double);
impl_trivial_from!(f32, AnyValue::Double);

impl_trivial_from!(String, AnyValue::String);
impl_trivial_from!(Cow<'static, str>, AnyValue::String);
impl_trivial_from!(&'static str, AnyValue::String);
impl_trivial_from!(StringValue, AnyValue::String);

impl_trivial_from!(bool, AnyValue::Boolean);

impl From<&[u8]> for AnyValue {
    fn from(val: &[u8]) -> AnyValue {
        AnyValue::Bytes(Box::new(val.to_vec()))
    }
}

impl<T: Into<AnyValue>> FromIterator<T> for AnyValue {
    /// Creates an [`AnyValue::ListAny`] value from a sequence of `Into<AnyValue>` values.
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        AnyValue::ListAny(Box::new(iter.into_iter().map(Into::into).collect()))
    }
}

impl<K: Into<Key>, V: Into<AnyValue>> FromIterator<(K, V)> for AnyValue {
    /// Creates an [`AnyValue::Map`] value from a sequence of key-value pairs
    /// that can be converted into a `Key` and `AnyValue` respectively.
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        AnyValue::Map(Box::new(HashMap::from_iter(
            iter.into_iter().map(|(k, v)| (k.into(), v.into())),
        )))
    }
}

/// A normalized severity value.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub enum Severity {
    /// TRACE
    Trace = 1,
    /// TRACE2
    Trace2 = 2,
    /// TRACE3
    Trace3 = 3,
    /// TRACE4
    Trace4 = 4,
    /// DEBUG
    Debug = 5,
    /// DEBUG2
    Debug2 = 6,
    /// DEBUG3
    Debug3 = 7,
    /// DEBUG4
    Debug4 = 8,
    /// INFO
    Info = 9,
    /// INFO2
    Info2 = 10,
    /// INFO3
    Info3 = 11,
    /// INFO4
    Info4 = 12,
    /// WARN
    Warn = 13,
    /// WARN2
    Warn2 = 14,
    /// WARN3
    Warn3 = 15,
    /// WARN4
    Warn4 = 16,
    /// ERROR
    Error = 17,
    /// ERROR2
    Error2 = 18,
    /// ERROR3
    Error3 = 19,
    /// ERROR4
    Error4 = 20,
    /// FATAL
    Fatal = 21,
    /// FATAL2
    Fatal2 = 22,
    /// FATAL3
    Fatal3 = 23,
    /// FATAL4
    Fatal4 = 24,
}

impl Severity {
    /// Return the string representing the short name for the `Severity`
    /// value as specified by the OpenTelemetry logs data model.
    pub const fn name(&self) -> &'static str {
        match &self {
            Severity::Trace => "TRACE",
            Severity::Trace2 => "TRACE2",
            Severity::Trace3 => "TRACE3",
            Severity::Trace4 => "TRACE4",

            Severity::Debug => "DEBUG",
            Severity::Debug2 => "DEBUG2",
            Severity::Debug3 => "DEBUG3",
            Severity::Debug4 => "DEBUG4",

            Severity::Info => "INFO",
            Severity::Info2 => "INFO2",
            Severity::Info3 => "INFO3",
            Severity::Info4 => "INFO4",

            Severity::Warn => "WARN",
            Severity::Warn2 => "WARN2",
            Severity::Warn3 => "WARN3",
            Severity::Warn4 => "WARN4",

            Severity::Error => "ERROR",
            Severity::Error2 => "ERROR2",
            Severity::Error3 => "ERROR3",
            Severity::Error4 => "ERROR4",

            Severity::Fatal => "FATAL",
            Severity::Fatal2 => "FATAL2",
            Severity::Fatal3 => "FATAL3",
            Severity::Fatal4 => "FATAL4",
        }
    }
}
