//! Tracing log export into the state SQLite database.
//!
//! This module provides a `tracing_subscriber::Layer` that captures events and
//! inserts them into the `logs` table in `state.sqlite`. The writer runs in a
//! background task and batches inserts to keep logging overhead low.
//!
//! ## Usage
//!
//! ```no_run
//! use codex_state::log_db;
//! use tracing_subscriber::prelude::*;
//!
//! # async fn example(state_db: std::sync::Arc<codex_state::StateRuntime>) {
//! let layer = log_db::start(state_db);
//! let _ = tracing_subscriber::registry()
//!     .with(layer)
//!     .try_init();
//! # }
//! ```

use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde_json::Value;
use tokio::sync::mpsc;
use tracing::Event;
use tracing::field::Field;
use tracing::field::Visit;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;

use crate::LogEntry;
use crate::StateRuntime;

const LOG_QUEUE_CAPACITY: usize = 512;
const LOG_BATCH_SIZE: usize = 64;
const LOG_FLUSH_INTERVAL: Duration = Duration::from_millis(250);

pub struct LogDbLayer {
    sender: mpsc::Sender<LogEntry>,
}

pub fn start(state_db: std::sync::Arc<StateRuntime>) -> LogDbLayer {
    let (sender, receiver) = mpsc::channel(LOG_QUEUE_CAPACITY);
    tokio::spawn(run_inserter(state_db, receiver));

    LogDbLayer { sender }
}

impl<S> Layer<S> for LogDbLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = JsonVisitor::default();
        event.record(&mut visitor);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));
        let entry = LogEntry {
            ts: now.as_secs() as i64,
            ts_nanos: now.subsec_nanos() as i64,
            level: metadata.level().as_str().to_string(),
            target: metadata.target().to_string(),
            message: visitor.message,
            fields_json: Value::Object(visitor.fields).to_string(),
            module_path: metadata.module_path().map(ToString::to_string),
            file: metadata.file().map(ToString::to_string),
            line: metadata.line().map(|line| line as i64),
        };

        let _ = self.sender.try_send(entry);
    }
}

async fn run_inserter(
    state_db: std::sync::Arc<StateRuntime>,
    mut receiver: mpsc::Receiver<LogEntry>,
) {
    let mut buffer = Vec::with_capacity(LOG_BATCH_SIZE);
    let mut ticker = tokio::time::interval(LOG_FLUSH_INTERVAL);
    loop {
        tokio::select! {
            maybe_entry = receiver.recv() => {
                match maybe_entry {
                    Some(entry) => {
                        buffer.push(entry);
                        if buffer.len() >= LOG_BATCH_SIZE {
                            flush(&state_db, &mut buffer).await;
                        }
                    }
                    None => {
                        flush(&state_db, &mut buffer).await;
                        break;
                    }
                }
            }
            _ = ticker.tick() => {
                flush(&state_db, &mut buffer).await;
            }
        }
    }
}

async fn flush(state_db: &std::sync::Arc<StateRuntime>, buffer: &mut Vec<LogEntry>) {
    if buffer.is_empty() {
        return;
    }
    let entries = buffer.split_off(0);
    let _ = state_db.insert_logs(entries.as_slice()).await;
}

#[derive(Default)]
struct JsonVisitor {
    fields: serde_json::Map<String, Value>,
    message: Option<String>,
}

impl JsonVisitor {
    fn record_value(&mut self, field: &Field, value: Value) {
        if field.name() == "message" && self.message.is_none() {
            self.message = Some(match &value {
                Value::String(message) => message.clone(),
                _ => value.to_string(),
            });
        }
        self.fields.insert(field.name().to_string(), value);
    }
}

impl Visit for JsonVisitor {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_value(field, Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_value(field, Value::from(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_value(field, Value::from(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_value(field, Value::from(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_value(field, Value::from(value));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.record_value(field, Value::from(value.to_string()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_value(field, Value::from(format!("{value:?}")));
    }
}
