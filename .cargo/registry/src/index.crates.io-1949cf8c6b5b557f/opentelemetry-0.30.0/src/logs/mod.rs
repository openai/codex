//! # OpenTelemetry Logs Bridge API
///  This API is not intended to be called by application developers directly.
///  It is provided for logging library authors to build log appenders, that
///  bridges existing logging systems with OpenTelemetry.
mod logger;
mod noop;
mod record;

pub use logger::{Logger, LoggerProvider};
pub use noop::NoopLoggerProvider;
pub use record::{AnyValue, LogRecord, Severity};
