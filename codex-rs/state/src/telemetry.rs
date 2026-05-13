use std::borrow::Cow;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use crate::DB_FALLBACK_METRIC;
use crate::DB_INIT_DURATION_METRIC;
use crate::DB_INIT_METRIC;
use tracing::debug;

/// Low-cardinality sink for SQLite startup and fallback telemetry.
///
/// Implementations should absorb delivery failures locally. Database behavior
/// must not depend on whether telemetry export succeeds.
pub trait DbTelemetry: Send + Sync + 'static {
    fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]);
    fn record_duration(&self, name: &str, duration: Duration, tags: &[(&str, &str)]);
}

pub type DbTelemetryHandle = Arc<dyn DbTelemetry>;

static PROCESS_DB_TELEMETRY: OnceLock<DbTelemetryHandle> = OnceLock::new();

/// Install the process-wide SQLite telemetry sink.
///
/// Startup owners should call this once after OTEL initialization. Low-level
/// database paths will use the registered sink unless an explicit sink is
/// provided. Subsequent installs are ignored and keep the first installed sink.
pub fn install_process_db_telemetry(telemetry: DbTelemetryHandle) -> bool {
    if PROCESS_DB_TELEMETRY.set(telemetry).is_ok() {
        true
    } else {
        debug!("process SQLite telemetry sink already installed; ignoring duplicate install");
        false
    }
}

#[derive(Clone, Copy)]
pub(crate) enum DbKind {
    State,
    Logs,
}

impl DbKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Logs => "logs",
        }
    }
}

pub(crate) fn record_init_result<T>(
    telemetry: Option<&dyn DbTelemetry>,
    db: DbKind,
    phase: &'static str,
    duration: Duration,
    result: &anyhow::Result<T>,
) {
    let outcome = DbOutcomeTags::from_result(result);
    let tags = [
        ("status", outcome.status),
        ("phase", phase),
        ("db", db.as_str()),
        ("error", outcome.error),
    ];
    record_counter(telemetry, DB_INIT_METRIC, &tags);
    record_duration(telemetry, DB_INIT_DURATION_METRIC, duration, &tags);
}

pub fn record_backfill_gate(
    telemetry: Option<&dyn DbTelemetry>,
    duration: Duration,
    result: &anyhow::Result<()>,
) {
    record_init_result(telemetry, DbKind::State, "backfill_gate", duration, result);
}

pub fn record_fallback(
    caller: &'static str,
    reason: &'static str,
    telemetry_override: Option<&dyn DbTelemetry>,
) {
    record_counter(
        telemetry_override,
        DB_FALLBACK_METRIC,
        &[("caller", caller), ("reason", reason)],
    );
}

fn record_counter(telemetry: Option<&dyn DbTelemetry>, name: &str, tags: &[(&str, &str)]) {
    if let Some(telemetry) = resolve_telemetry(telemetry) {
        telemetry.counter(name, /*inc*/ 1, tags);
    }
}

fn record_duration(
    telemetry: Option<&dyn DbTelemetry>,
    name: &str,
    duration: Duration,
    tags: &[(&str, &str)],
) {
    if let Some(telemetry) = resolve_telemetry(telemetry) {
        telemetry.record_duration(name, duration, tags);
    }
}

fn resolve_telemetry(telemetry: Option<&dyn DbTelemetry>) -> Option<&dyn DbTelemetry> {
    telemetry.or_else(|| PROCESS_DB_TELEMETRY.get().map(AsRef::as_ref))
}

struct DbOutcomeTags {
    status: &'static str,
    error: &'static str,
}

impl DbOutcomeTags {
    fn from_result<T>(result: &anyhow::Result<T>) -> Self {
        match result {
            Ok(_) => Self {
                status: "success",
                error: "none",
            },
            Err(err) => Self {
                status: "failed",
                error: classify_error(err),
            },
        }
    }
}

fn classify_error(err: &anyhow::Error) -> &'static str {
    for cause in err.chain() {
        if let Some(sqlx_err) = cause.downcast_ref::<sqlx::Error>() {
            return classify_sqlx_error(sqlx_err);
        }
        if cause
            .downcast_ref::<sqlx::migrate::MigrateError>()
            .is_some()
        {
            return "migration";
        }
        if cause.downcast_ref::<serde_json::Error>().is_some() {
            return "serde";
        }
        if cause.downcast_ref::<std::io::Error>().is_some() {
            return "io";
        }
    }
    "unknown"
}

fn classify_sqlx_error(err: &sqlx::Error) -> &'static str {
    match err {
        sqlx::Error::Database(database_error) => {
            let code = database_error
                .code()
                .unwrap_or(Cow::Borrowed("none"))
                .to_string();
            classify_sqlite_code(code.as_str())
        }
        sqlx::Error::PoolTimedOut => "pool_timeout",
        sqlx::Error::Io(_) => "io",
        sqlx::Error::ColumnDecode { source, .. } if source.is::<serde_json::Error>() => "serde",
        sqlx::Error::Decode(source) if source.is::<serde_json::Error>() => "serde",
        _ => "unknown",
    }
}

fn classify_sqlite_code(code: &str) -> &'static str {
    // SQLite result codes are documented at https://www.sqlite.org/rescode.html.
    // Extended codes preserve the primary code in the low byte.
    let primary_code = code.parse::<i32>().ok().map(|code| code & 0xff);
    match primary_code {
        Some(5) => "busy",
        Some(6) => "locked",
        Some(8) => "readonly",
        Some(10) => "io",
        Some(11) => "corrupt",
        Some(13) => "full",
        Some(14) => "cantopen",
        Some(17) => "schema",
        Some(19) => "constraint",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn classifies_extended_sqlite_codes() {
        assert_eq!(classify_sqlite_code("5"), "busy");
        assert_eq!(classify_sqlite_code("6"), "locked");
        assert_eq!(classify_sqlite_code("2067"), "constraint");
    }
}
