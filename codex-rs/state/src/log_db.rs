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

use chrono::Duration as ChronoDuration;
use chrono::Utc;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use tokio::sync::mpsc;
use tracing::Event;
use tracing::field::Field;
use tracing::field::Visit;
use tracing::span::Attributes;
use tracing::span::Id;
use tracing::span::Record;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;
use uuid::Uuid;

use crate::LogEntry;
use crate::StateRuntime;

const LOG_QUEUE_CAPACITY: usize = 512;
const LOG_BATCH_SIZE: usize = 64;
const LOG_FLUSH_INTERVAL: Duration = Duration::from_millis(250);
const LOG_RETENTION_DAYS: i64 = 90;
const LOG_RETENTION_CLEANUP_INTERVAL: Duration = Duration::from_secs(60 * 60);

pub struct LogDbLayer {
    sender: mpsc::Sender<LogEntry>,
    process_uuid: String,
}

pub fn start(state_db: std::sync::Arc<StateRuntime>) -> LogDbLayer {
    let process_uuid = current_process_log_uuid().to_string();
    let (sender, receiver) = mpsc::channel(LOG_QUEUE_CAPACITY);
    tokio::spawn(run_inserter(std::sync::Arc::clone(&state_db), receiver));
    tokio::spawn(run_retention_cleanup(state_db));

    LogDbLayer {
        sender,
        process_uuid,
    }
}

impl<S> Layer<S> for LogDbLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &Attributes<'_>,
        id: &Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = SpanFieldVisitor::default();
        attrs.record(&mut visitor);

        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(SpanLogContext {
                thread_id: visitor.thread_id,
            });
        }
    }

    fn on_record(
        &self,
        id: &Id,
        values: &Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = SpanFieldVisitor::default();
        values.record(&mut visitor);

        if visitor.thread_id.is_none() {
            return;
        }

        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            if let Some(log_context) = extensions.get_mut::<SpanLogContext>() {
                log_context.thread_id = visitor.thread_id;
            } else {
                extensions.insert(SpanLogContext {
                    thread_id: visitor.thread_id,
                });
            }
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let thread_id = visitor
            .thread_id
            .clone()
            .or_else(|| event_thread_id(event, &ctx));

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));
        let entry = LogEntry {
            ts: now.as_secs() as i64,
            ts_nanos: now.subsec_nanos() as i64,
            level: metadata.level().as_str().to_string(),
            target: metadata.target().to_string(),
            message: visitor.message,
            thread_id,
            process_uuid: Some(self.process_uuid.clone()),
            module_path: metadata.module_path().map(ToString::to_string),
            file: metadata.file().map(ToString::to_string),
            line: metadata.line().map(|line| line as i64),
        };

        let _ = self.sender.try_send(entry);
    }
}

#[derive(Clone, Debug, Default)]
struct SpanLogContext {
    thread_id: Option<String>,
}

#[derive(Default)]
struct SpanFieldVisitor {
    thread_id: Option<String>,
}

impl SpanFieldVisitor {
    fn record_field(&mut self, field: &Field, value: String) {
        if field.name() == "thread_id" && self.thread_id.is_none() {
            self.thread_id = Some(value);
        }
    }
}

impl Visit for SpanFieldVisitor {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_field(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_field(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_field(field, value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_field(field, value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_field(field, value.to_string());
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.record_field(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_field(field, format!("{value:?}"));
    }
}

fn event_thread_id<S>(
    event: &Event<'_>,
    ctx: &tracing_subscriber::layer::Context<'_, S>,
) -> Option<String>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    let mut thread_id = None;
    if let Some(scope) = ctx.event_scope(event) {
        for span in scope.from_root() {
            let extensions = span.extensions();
            if let Some(log_context) = extensions.get::<SpanLogContext>()
                && log_context.thread_id.is_some()
            {
                thread_id = log_context.thread_id.clone();
            }
        }
    }
    thread_id
}

fn current_process_log_uuid() -> &'static str {
    static PROCESS_LOG_UUID: OnceLock<String> = OnceLock::new();
    PROCESS_LOG_UUID.get_or_init(|| {
        let pid = std::process::id();
        let process_uuid = Uuid::new_v4();
        format!("pid:{pid}:{process_uuid}")
    })
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

async fn run_retention_cleanup(state_db: std::sync::Arc<StateRuntime>) {
    run_retention_cleanup_with_interval(state_db, LOG_RETENTION_CLEANUP_INTERVAL).await;
}

async fn run_retention_cleanup_with_interval(
    state_db: std::sync::Arc<StateRuntime>,
    interval: Duration,
) {
    run_retention_cleanup_once(&state_db).await;
    loop {
        tokio::time::sleep(interval).await;
        run_retention_cleanup_once(&state_db).await;
    }
}

async fn run_retention_cleanup_once(state_db: &StateRuntime) {
    let Some(cutoff) = Utc::now().checked_sub_signed(ChronoDuration::days(LOG_RETENTION_DAYS))
    else {
        return;
    };
    let _ = state_db.delete_logs_before(cutoff.timestamp()).await;
}

#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
    thread_id: Option<String>,
}

impl MessageVisitor {
    fn record_field(&mut self, field: &Field, value: String) {
        if field.name() == "message" && self.message.is_none() {
            self.message = Some(value.clone());
        }
        if field.name() == "thread_id" && self.thread_id.is_none() {
            self.thread_id = Some(value);
        }
    }
}

impl Visit for MessageVisitor {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_field(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_field(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_field(field, value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_field(field, value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_field(field, value.to_string());
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.record_field(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_field(field, format!("{value:?}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stale_log(message: &str) -> LogEntry {
        let stale_ts = Utc::now()
            .checked_sub_signed(ChronoDuration::days(LOG_RETENTION_DAYS + 1))
            .expect("stale timestamp should be representable")
            .timestamp();
        LogEntry {
            ts: stale_ts,
            ts_nanos: 0,
            level: "INFO".to_string(),
            target: "log-db-test".to_string(),
            message: Some(message.to_string()),
            thread_id: None,
            module_path: None,
            file: None,
            line: None,
        }
    }

    async fn wait_for_log_count(state_db: &StateRuntime, expected_count: usize) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            let logs = state_db
                .query_logs(&crate::LogQuery::default())
                .await
                .expect("query logs should succeed");
            if logs.len() == expected_count {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for log count {expected_count}, saw {}",
                logs.len()
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn retention_cleanup_runs_periodically_for_long_lived_processes() {
        let temp = std::env::temp_dir().join(format!(
            "codex-state-log-db-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after epoch")
                .as_nanos()
        ));
        let state_db = StateRuntime::init(temp.clone(), "test-provider".to_string(), None)
            .await
            .expect("state db should initialize");

        state_db
            .insert_log(&stale_log("old-startup-log"))
            .await
            .expect("log insert should succeed");
        let cleanup_task = tokio::spawn(run_retention_cleanup_with_interval(
            std::sync::Arc::clone(&state_db),
            Duration::from_millis(20),
        ));

        wait_for_log_count(&state_db, 0).await;

        state_db
            .insert_log(&stale_log("old-runtime-log"))
            .await
            .expect("log insert should succeed");
        wait_for_log_count(&state_db, 0).await;

        cleanup_task.abort();
        let _ = tokio::fs::remove_dir_all(temp).await;
    }
}
