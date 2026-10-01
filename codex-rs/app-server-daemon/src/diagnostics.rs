//! Default-on local lifecycle records contain only fixed events and typed OS errors.
//! Error messages and chains can contain raw child stderr or protocol data; never print them.

use std::io::IsTerminal;
use std::io::Write;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Serialize, PartialEq, Eq)]
struct Failure {
    kind: String,
    os_code: Option<i32>,
}

impl From<&anyhow::Error> for Failure {
    fn from(error: &anyhow::Error) -> Self {
        match error.downcast_ref::<std::io::Error>() {
            Some(error) => Self {
                kind: format!("{:?}", error.kind()),
                os_code: error.raw_os_error(),
            },
            None => Self {
                kind: "other".to_string(),
                os_code: None,
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Record<D> {
    timestamp_ms: u128,
    process_pid: u32,
    #[serde(flatten)]
    payload: Payload<D>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Payload<D> {
    Event {
        event: &'static str,
        details: D,
    },
    Result {
        stage: &'static str,
        outcome: Outcome,
        #[serde(rename = "elapsedMs")]
        elapsed_ms: u128,
        failure: Option<Failure>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Outcome {
    Succeeded,
    Failed,
}

pub(crate) fn event(name: &'static str, details: impl Serialize) {
    emit(Payload::Event {
        event: name,
        details,
    });
}

pub(crate) fn result<T>(stage: &'static str, started: Instant, result: Result<T>) -> Result<T> {
    emit(Payload::<()>::Result {
        stage,
        outcome: if result.is_ok() {
            Outcome::Succeeded
        } else {
            Outcome::Failed
        },
        elapsed_ms: started.elapsed().as_millis(),
        failure: result.as_ref().err().map(Failure::from),
    });
    result
}

fn emit(payload: Payload<impl Serialize>) {
    let failed = matches!(
        &payload,
        Payload::Result {
            outcome: Outcome::Failed,
            ..
        }
    );
    let record = Record {
        timestamp_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        process_pid: std::process::id(),
        payload,
    };
    let Ok(record) = serde_json::to_string(&record) else {
        return;
    };
    if tracing::dispatcher::has_been_set() {
        // Use the configured logger whenever one is available.
        if failed {
            tracing::warn!(target: "codex_app_server_daemon", "{record}");
        } else {
            tracing::info!(target: "codex_app_server_daemon", "{record}");
        }
    } else if !std::io::stderr().is_terminal() {
        // The detached updater has redirected stderr and no tracing subscriber.
        // A TUI can own a live terminal before installing its subscriber.
        // Never let a log write failure interrupt the lifecycle operation.
        let _ = writeln!(std::io::stderr().lock(), "{record}");
    }
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
