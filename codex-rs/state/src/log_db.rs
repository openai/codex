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
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tracing::Event;
use tracing::Level;
use tracing::field::Field;
use tracing::field::Visit;
use tracing::span::Attributes;
use tracing::span::Id;
use tracing::span::Record;
use tracing::warn;
use tracing_subscriber::Layer;
use tracing_subscriber::fmt::FormattedFields;
use tracing_subscriber::fmt::format::DefaultFields;
use tracing_subscriber::fmt::format::FormatFields;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::fmt::time::SystemTime as FmtSystemTime;
use tracing_subscriber::registry::LookupSpan;

use crate::LogEntry;
use crate::StateRuntime;
use crate::current_process_log_id;

const LOG_QUEUE_CAPACITY: usize = 512;
const LOG_BATCH_SIZE: usize = 64;
const LOG_FLUSH_INTERVAL: Duration = Duration::from_millis(250);
const LOG_RETENTION_DAYS: i64 = 90;
// Per-scope sqlite feedback log retention policy:
// - We track bytes separately for each session scope (`thread_id`) and for
//   process-scoped threadless logs (`thread_id IS NULL` + `process_id`).
// - On insert, we lazily initialize a scope's byte count once from sqlite and
//   then update it incrementally from appended lines.
// - If a scope exceeds the trigger, we delete oldest rows in that same scope
//   until only the newest target bytes remain.
// This keeps `/feedback` log storage bounded without a full startup scan.
const LOG_SCOPE_TRIM_TRIGGER_BYTES: usize = 20 * 1024 * 1024;
const LOG_SCOPE_TRIM_TARGET_BYTES: usize = 10 * 1024 * 1024;
const LOG_DB_TARGET: &str = "codex_state::log_db";

pub struct LogDbLayer {
    sender: mpsc::Sender<LogEntry>,
    fmt_fields: DefaultFields,
    fmt_timer: FmtSystemTime,
    dropped_log_entries: AtomicUsize,
    process_id: String,
}

#[derive(Default)]
struct LogScopeByteState {
    process_threadless_initialized: bool,
    process_threadless_bytes: usize,
    thread_bytes: HashMap<String, usize>,
}

pub fn start(state_db: std::sync::Arc<StateRuntime>) -> LogDbLayer {
    let process_id = current_process_log_id().to_string();
    let (sender, receiver) = mpsc::channel(LOG_QUEUE_CAPACITY);
    tokio::spawn(run_inserter(
        std::sync::Arc::clone(&state_db),
        receiver,
        process_id.clone(),
    ));
    tokio::spawn(run_retention_cleanup(state_db));

    LogDbLayer {
        sender,
        fmt_fields: DefaultFields::new(),
        fmt_timer: FmtSystemTime,
        dropped_log_entries: AtomicUsize::new(0),
        process_id,
    }
}

impl LogDbLayer {
    fn enqueue_entry(&self, entry: LogEntry) {
        let resume_thread_id = entry.thread_id.clone();
        match self.sender.try_send(entry) {
            Ok(()) => {
                let dropped = self.dropped_log_entries.swap(0, Ordering::Relaxed);
                if dropped == 0 {
                    return;
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_else(|_| Duration::from_secs(0));
                let summary = LogEntry {
                    ts: now.as_secs() as i64,
                    ts_nanos: now.subsec_nanos() as i64,
                    level: Level::WARN.as_str().to_string(),
                    target: LOG_DB_TARGET.to_string(),
                    message: Some(format!(
                        "Dropped {dropped} log entries because sqlite log queue was full; logging resumed."
                    )),
                    thread_id: resume_thread_id,
                    process_id: Some(self.process_id.clone()),
                    module_path: None,
                    file: None,
                    line: None,
                };

                if let Err(TrySendError::Full(_)) = self.sender.try_send(summary) {
                    self.dropped_log_entries
                        .fetch_add(dropped, Ordering::Relaxed);
                }
            }
            Err(TrySendError::Full(_)) => {
                self.dropped_log_entries.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Closed(_)) => {}
        }
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
            let mut extensions = span.extensions_mut();
            extensions.insert(SpanLogContext {
                thread_id: visitor.thread_id,
            });
            if extensions
                .get_mut::<FormattedFields<DefaultFields>>()
                .is_none()
            {
                let mut fields = FormattedFields::<DefaultFields>::new(String::new());
                if self
                    .fmt_fields
                    .format_fields(fields.as_writer(), attrs)
                    .is_ok()
                {
                    extensions.insert(fields);
                }
            }
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

        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            if visitor.thread_id.is_some() {
                if let Some(log_context) = extensions.get_mut::<SpanLogContext>() {
                    log_context.thread_id = visitor.thread_id;
                } else {
                    extensions.insert(SpanLogContext {
                        thread_id: visitor.thread_id,
                    });
                }
            }

            if let Some(fields) = extensions.get_mut::<FormattedFields<DefaultFields>>() {
                let _ = self.fmt_fields.add_fields(fields, values);
            } else {
                let mut fields = FormattedFields::<DefaultFields>::new(String::new());
                if self
                    .fmt_fields
                    .format_fields(fields.as_writer(), values)
                    .is_ok()
                {
                    extensions.insert(fields);
                }
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
        let message = format_feedback_line(&self.fmt_fields, self.fmt_timer, event, &ctx)
            .ok()
            .or(visitor.message);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));
        let entry = LogEntry {
            ts: now.as_secs() as i64,
            ts_nanos: now.subsec_nanos() as i64,
            level: metadata.level().as_str().to_string(),
            target: metadata.target().to_string(),
            message,
            thread_id,
            process_id: Some(self.process_id.clone()),
            module_path: metadata.module_path().map(ToString::to_string),
            file: metadata.file().map(ToString::to_string),
            line: metadata.line().map(|line| line as i64),
        };

        self.enqueue_entry(entry);
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

fn format_feedback_line<S>(
    fmt_fields: &DefaultFields,
    fmt_timer: FmtSystemTime,
    event: &Event<'_>,
    ctx: &tracing_subscriber::layer::Context<'_, S>,
) -> Result<String, fmt::Error>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    let mut line = String::new();
    let mut writer = Writer::new(&mut line);
    if fmt_timer.format_time(&mut writer).is_err() {
        writer.write_str("<unknown time>")?;
    }
    writer.write_char(' ')?;
    writer.write_str(feedback_level(*event.metadata().level()))?;
    writer.write_char(' ')?;

    let mut saw_spans = false;
    if let Some(scope) = ctx.event_scope(event) {
        for span in scope.from_root() {
            writer.write_str(span.metadata().name())?;
            saw_spans = true;

            let extensions = span.extensions();
            if let Some(fields) = extensions.get::<FormattedFields<DefaultFields>>()
                && !fields.is_empty()
            {
                writer.write_char('{')?;
                writer.write_str(fields)?;
                writer.write_char('}')?;
            }
            writer.write_char(':')?;
        }
    }
    if saw_spans {
        writer.write_char(' ')?;
    }

    fmt_fields.format_fields(writer.by_ref(), event)?;
    Ok(line)
}

fn feedback_level(level: Level) -> &'static str {
    match level {
        Level::TRACE => "TRACE",
        Level::DEBUG => "DEBUG",
        Level::INFO => " INFO",
        Level::WARN => " WARN",
        Level::ERROR => "ERROR",
    }
}

async fn run_inserter(
    state_db: std::sync::Arc<StateRuntime>,
    mut receiver: mpsc::Receiver<LogEntry>,
    process_id: String,
) {
    let mut scope_bytes = LogScopeByteState::default();
    let mut buffer = Vec::with_capacity(LOG_BATCH_SIZE);
    let mut ticker = tokio::time::interval(LOG_FLUSH_INTERVAL);
    loop {
        tokio::select! {
            maybe_entry = receiver.recv() => {
                match maybe_entry {
                    Some(entry) => {
                        buffer.push(entry);
                        if buffer.len() >= LOG_BATCH_SIZE {
                            flush(
                                &state_db,
                                &mut buffer,
                                process_id.as_str(),
                                &mut scope_bytes,
                            ).await;
                        }
                    }
                    None => {
                        flush(
                            &state_db,
                            &mut buffer,
                            process_id.as_str(),
                            &mut scope_bytes,
                        ).await;
                        break;
                    }
                }
            }
            _ = ticker.tick() => {
                flush(
                    &state_db,
                    &mut buffer,
                    process_id.as_str(),
                    &mut scope_bytes,
                ).await;
            }
        }
    }
}

async fn flush(
    state_db: &std::sync::Arc<StateRuntime>,
    buffer: &mut Vec<LogEntry>,
    process_id: &str,
    scope_bytes: &mut LogScopeByteState,
) {
    if buffer.is_empty() {
        return;
    }

    // Drain the current batch so producers can keep writing into a fresh buffer
    // while this flush works against an owned vector.
    let entries = buffer.split_off(0);

    // Aggregate the byte delta contributed by this batch, keyed by feedback
    // scope. We use the same newline-normalized byte accounting as `/feedback`.
    let mut thread_added_bytes = HashMap::new();
    let mut process_threadless_added_bytes = 0usize;
    for entry in &entries {
        let Some(message) = entry.message.as_deref() else {
            continue;
        };
        let line_bytes = feedback_line_bytes(message);
        if let Some(thread_id) = entry.thread_id.as_ref() {
            let total = thread_added_bytes
                .entry(thread_id.clone())
                .or_insert(0usize);
            *total = total.saturating_add(line_bytes);
        } else {
            // `entries` comes from this process-local channel, so a threadless
            // row in this batch always belongs to the current process scope.
            process_threadless_added_bytes =
                process_threadless_added_bytes.saturating_add(line_bytes);
        }
    }

    // Persist first; all in-memory counters below are derived from committed DB
    // state plus this batch.
    if let Err(err) = state_db.insert_logs(entries.as_slice()).await {
        warn!("failed to insert sqlite logs batch: {err}");
        return;
    }

    // Process threadless scope:
    // - initialize once from DB (already includes this batch because we insert
    //   first), then
    // - incrementally add only this flush's delta.
    if !scope_bytes.process_threadless_initialized {
        match state_db.process_threadless_log_bytes(process_id).await {
            Ok(total) => {
                scope_bytes.process_threadless_bytes = total;
                scope_bytes.process_threadless_initialized = true;
            }
            Err(err) => {
                warn!("failed to initialize process threadless log bytes ({process_id}): {err}");
                scope_bytes.process_threadless_bytes = scope_bytes
                    .process_threadless_bytes
                    .saturating_add(process_threadless_added_bytes);
            }
        }
    } else {
        scope_bytes.process_threadless_bytes = scope_bytes
            .process_threadless_bytes
            .saturating_add(process_threadless_added_bytes);
    }
    if scope_bytes.process_threadless_bytes > LOG_SCOPE_TRIM_TRIGGER_BYTES {
        match state_db
            .trim_process_threadless_logs_to_target(process_id, LOG_SCOPE_TRIM_TARGET_BYTES)
            .await
        {
            Ok(bytes) => {
                scope_bytes.process_threadless_bytes = bytes;
            }
            Err(err) => {
                warn!("failed to trim process threadless logs ({process_id}): {err}");
            }
        }
    }

    // Session scopes:
    // - initialize unseen sessions once from DB,
    // - otherwise add this flush's delta,
    // - trim touched sessions that exceed trigger.
    for (thread_id, added_bytes) in thread_added_bytes {
        let total = if let Some(total) = scope_bytes.thread_bytes.get_mut(&thread_id) {
            *total = total.saturating_add(added_bytes);
            *total
        } else {
            match state_db.thread_log_bytes(thread_id.as_str()).await {
                Ok(total) => {
                    scope_bytes.thread_bytes.insert(thread_id.clone(), total);
                    total
                }
                Err(err) => {
                    warn!("failed to initialize session log bytes ({thread_id}): {err}");
                    scope_bytes
                        .thread_bytes
                        .insert(thread_id.clone(), added_bytes);
                    added_bytes
                }
            }
        };

        if total <= LOG_SCOPE_TRIM_TRIGGER_BYTES {
            continue;
        }
        match state_db
            .trim_thread_logs_to_target(thread_id.as_str(), LOG_SCOPE_TRIM_TARGET_BYTES)
            .await
        {
            Ok(bytes) => {
                scope_bytes.thread_bytes.insert(thread_id, bytes);
            }
            Err(err) => {
                warn!("failed to trim session logs ({thread_id}): {err}");
            }
        }
    }
}

async fn run_retention_cleanup(state_db: std::sync::Arc<StateRuntime>) {
    let Some(cutoff) = Utc::now().checked_sub_signed(ChronoDuration::days(LOG_RETENTION_DAYS))
    else {
        return;
    };
    let _ = state_db.delete_logs_before(cutoff.timestamp()).await;
}

fn feedback_line_bytes(message: &str) -> usize {
    if message.ends_with('\n') {
        message.len()
    } else {
        message.len() + 1
    }
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
    use pretty_assertions::assert_eq;

    fn log_entry(message: &str) -> LogEntry {
        LogEntry {
            ts: 0,
            ts_nanos: 0,
            level: Level::INFO.as_str().to_string(),
            target: "test".to_string(),
            message: Some(message.to_string()),
            thread_id: None,
            process_id: None,
            module_path: None,
            file: None,
            line: None,
        }
    }

    #[test]
    fn emits_drop_summary_after_queue_recovers() {
        let (sender, mut receiver) = mpsc::channel(2);
        let layer = LogDbLayer {
            sender,
            fmt_fields: DefaultFields::new(),
            fmt_timer: FmtSystemTime,
            dropped_log_entries: AtomicUsize::new(0),
            process_id: "test-process".to_string(),
        };

        layer.enqueue_entry(log_entry("first"));
        layer.enqueue_entry(log_entry("second"));
        layer.enqueue_entry(log_entry("third"));
        assert_eq!(layer.dropped_log_entries.load(Ordering::Relaxed), 1);

        let _ = receiver.try_recv().expect("first log should be queued");
        let _ = receiver.try_recv().expect("second log should be queued");

        layer.enqueue_entry(log_entry("fourth"));

        let resumed = receiver.try_recv().expect("recovery log should be queued");
        assert_eq!(resumed.message.as_deref(), Some("fourth"));

        let summary = receiver.try_recv().expect("drop summary should be queued");
        assert_eq!(summary.level, Level::WARN.as_str());
        assert_eq!(summary.target, LOG_DB_TARGET);
        assert_eq!(
            summary.message.as_deref(),
            Some("Dropped 1 log entries because sqlite log queue was full; logging resumed.")
        );
        assert_eq!(layer.dropped_log_entries.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn preserves_drop_count_when_summary_send_is_still_full() {
        let (sender, mut receiver) = mpsc::channel(1);
        let layer = LogDbLayer {
            sender,
            fmt_fields: DefaultFields::new(),
            fmt_timer: FmtSystemTime,
            dropped_log_entries: AtomicUsize::new(0),
            process_id: "test-process".to_string(),
        };

        layer.enqueue_entry(log_entry("first"));
        layer.enqueue_entry(log_entry("second"));
        assert_eq!(layer.dropped_log_entries.load(Ordering::Relaxed), 1);

        let _ = receiver.try_recv().expect("first log should be queued");
        layer.enqueue_entry(log_entry("third"));

        // Queue is full with "third", so summary cannot be queued yet and count is retained.
        assert_eq!(layer.dropped_log_entries.load(Ordering::Relaxed), 1);
        let third = receiver.try_recv().expect("third log should be queued");
        assert_eq!(third.message.as_deref(), Some("third"));
    }
}
