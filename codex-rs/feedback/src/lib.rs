use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::collections::btree_map::Entry;
use std::fs;
use std::io::Write;
use std::io::{self};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use tracing::Event;
use tracing::Level;
use tracing::field::Visit;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::fmt::writer::MakeWriter;
use tracing_subscriber::registry::LookupSpan;

const DEFAULT_MAX_BYTES: usize = 4 * 1024 * 1024; // 4 MiB
const MAX_FEEDBACK_LOG_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB hard cap
const SENTRY_DSN: &str =
    "https://ae32ed50620d7a7792c1ce5df38b3e3e@o33249.ingest.us.sentry.io/4510195390611458";
const UPLOAD_TIMEOUT_SECS: u64 = 10;
const FEEDBACK_TAGS_TARGET: &str = "feedback_tags";
const MAX_FEEDBACK_TAGS: usize = 64;

#[derive(Clone)]
pub struct CodexFeedback {
    inner: Arc<FeedbackInner>,
}

impl Default for CodexFeedback {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexFeedback {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_MAX_BYTES)
    }

    pub(crate) fn with_capacity(max_bytes: usize) -> Self {
        Self {
            inner: Arc::new(FeedbackInner::new(max_bytes)),
        }
    }

    pub fn make_writer(&self) -> FeedbackMakeWriter {
        FeedbackMakeWriter {
            inner: self.inner.clone(),
        }
    }

    /// Returns a [`tracing_subscriber`] layer that captures full-fidelity logs into this feedback
    /// ring buffer.
    ///
    /// This is intended for initialization code so call sites don't have to duplicate the exact
    /// `fmt::layer()` configuration and filter logic.
    pub fn logger_layer<S>(&self) -> impl Layer<S> + Send + Sync + 'static
    where
        S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    {
        tracing_subscriber::fmt::layer()
            .with_writer(self.make_writer())
            .with_ansi(false)
            .with_target(false)
            // Capture everything, regardless of the caller's `RUST_LOG`, so feedback includes the
            // full trace when the user uploads a report.
            .with_filter(Targets::new().with_default(Level::TRACE))
    }

    /// Returns a [`tracing_subscriber`] layer that collects structured metadata for feedback.
    ///
    /// Events with `target: "feedback_tags"` are treated as key/value tags to attach to feedback
    /// uploads later.
    pub fn metadata_layer<S>(&self) -> impl Layer<S> + Send + Sync + 'static
    where
        S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    {
        FeedbackMetadataLayer {
            inner: self.inner.clone(),
        }
        .with_filter(Targets::new().with_target(FEEDBACK_TAGS_TARGET, Level::TRACE))
    }

    pub fn snapshot(&self, session_id: Option<ThreadId>) -> CodexLogSnapshot {
        let bytes = {
            let guard = self.inner.ring.lock().expect("mutex poisoned");
            guard.snapshot_bytes()
        };
        let tags = {
            let guard = self.inner.tags.lock().expect("mutex poisoned");
            guard.clone()
        };
        CodexLogSnapshot {
            bytes,
            tags,
            thread_id: session_id
                .map(|id| id.to_string())
                .unwrap_or("no-active-thread-".to_string() + &ThreadId::new().to_string()),
        }
    }
}

struct FeedbackInner {
    ring: Mutex<RingBuffer>,
    tags: Mutex<BTreeMap<String, String>>,
}

impl FeedbackInner {
    fn new(max_bytes: usize) -> Self {
        Self {
            ring: Mutex::new(RingBuffer::new(max_bytes)),
            tags: Mutex::new(BTreeMap::new()),
        }
    }
}

#[derive(Clone)]
pub struct FeedbackMakeWriter {
    inner: Arc<FeedbackInner>,
}

impl<'a> MakeWriter<'a> for FeedbackMakeWriter {
    type Writer = FeedbackWriter;

    fn make_writer(&'a self) -> Self::Writer {
        FeedbackWriter {
            inner: self.inner.clone(),
        }
    }
}

pub struct FeedbackWriter {
    inner: Arc<FeedbackInner>,
}

impl Write for FeedbackWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self.inner.ring.lock().map_err(|_| io::ErrorKind::Other)?;
        guard.push_bytes(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct RingBuffer {
    max: usize,
    buf: VecDeque<u8>,
}

impl RingBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            max: capacity,
            buf: VecDeque::with_capacity(capacity),
        }
    }

    fn len(&self) -> usize {
        self.buf.len()
    }

    fn push_bytes(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        // If the incoming chunk is larger than capacity, keep only the trailing bytes.
        if data.len() >= self.max {
            self.buf.clear();
            let start = data.len() - self.max;
            self.buf.extend(data[start..].iter().copied());
            return;
        }

        // Evict from the front if we would exceed capacity.
        let needed = self.len() + data.len();
        if needed > self.max {
            let to_drop = needed - self.max;
            for _ in 0..to_drop {
                let _ = self.buf.pop_front();
            }
        }

        self.buf.extend(data.iter().copied());
    }

    fn snapshot_bytes(&self) -> Vec<u8> {
        self.buf.iter().copied().collect()
    }
}

pub struct CodexLogSnapshot {
    bytes: Vec<u8>,
    tags: BTreeMap<String, String>,
    pub thread_id: String,
}

impl CodexLogSnapshot {
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn save_to_temp_file(&self) -> io::Result<PathBuf> {
        let dir = std::env::temp_dir();
        let filename = format!("codex-feedback-{}.log", self.thread_id);
        let path = dir.join(filename);
        fs::write(&path, self.as_bytes())?;
        Ok(path)
    }

    /// Upload feedback to Sentry with optional attachments.
    pub fn upload_feedback(
        &self,
        classification: &str,
        reason: Option<&str>,
        include_logs: bool,
        rollout_path: Option<&std::path::Path>,
        session_source: Option<SessionSource>,
    ) -> Result<()> {
        self.upload_feedback_with_logs(
            classification,
            reason,
            include_logs,
            rollout_path,
            session_source,
            None,
        )
    }

    /// Upload feedback to Sentry with optional attachments and an optional
    /// override for the `codex-logs.log` attachment bytes.
    pub fn upload_feedback_with_logs(
        &self,
        classification: &str,
        reason: Option<&str>,
        include_logs: bool,
        rollout_path: Option<&std::path::Path>,
        session_source: Option<SessionSource>,
        logs_override: Option<Vec<u8>>,
    ) -> Result<()> {
        use std::collections::BTreeMap;
        use std::fs;
        use std::str::FromStr;
        use std::sync::Arc;

        use sentry::Client;
        use sentry::ClientOptions;
        use sentry::protocol::Attachment;
        use sentry::protocol::Envelope;
        use sentry::protocol::EnvelopeItem;
        use sentry::protocol::Event;
        use sentry::protocol::Level;
        use sentry::transports::DefaultTransportFactory;
        use sentry::types::Dsn;

        // Build Sentry client
        let client = Client::from_config(ClientOptions {
            dsn: Some(Dsn::from_str(SENTRY_DSN).map_err(|e| anyhow!("invalid DSN: {e}"))?),
            transport: Some(Arc::new(DefaultTransportFactory {})),
            ..Default::default()
        });

        let cli_version = env!("CARGO_PKG_VERSION");
        let mut tags = BTreeMap::from([
            (String::from("thread_id"), self.thread_id.to_string()),
            (String::from("classification"), classification.to_string()),
            (String::from("cli_version"), cli_version.to_string()),
        ]);
        if let Some(source) = session_source.as_ref() {
            tags.insert(String::from("session_source"), source.to_string());
        }
        if let Some(r) = reason {
            tags.insert(String::from("reason"), r.to_string());
        }

        let reserved = [
            "thread_id",
            "classification",
            "cli_version",
            "session_source",
            "reason",
        ];
        for (key, value) in &self.tags {
            if reserved.contains(&key.as_str()) {
                continue;
            }
            if let Entry::Vacant(entry) = tags.entry(key.clone()) {
                entry.insert(value.clone());
            }
        }

        let level = match classification {
            "bug" | "bad_result" => Level::Error,
            _ => Level::Info,
        };

        let mut envelope = Envelope::new();
        let title = format!(
            "[{}]: Codex session {}",
            display_classification(classification),
            self.thread_id
        );

        let mut event = Event {
            level,
            message: Some(title.clone()),
            tags,
            ..Default::default()
        };
        if let Some(r) = reason {
            use sentry::protocol::Exception;
            use sentry::protocol::Values;

            event.exception = Values::from(vec![Exception {
                ty: title.clone(),
                value: Some(r.to_string()),
                ..Default::default()
            }]);
        }
        envelope.add_item(EnvelopeItem::Event(event));

        if include_logs {
            let logs = clamp_feedback_log_attachment_bytes(
                logs_override.unwrap_or_else(|| self.bytes.clone()),
                MAX_FEEDBACK_LOG_ATTACHMENT_BYTES,
            );
            envelope.add_item(EnvelopeItem::Attachment(Attachment {
                buffer: logs,
                filename: String::from("codex-logs.log"),
                content_type: Some("text/plain".to_string()),
                ty: None,
            }));
        }

        if let Some((path, data)) = rollout_path.and_then(|p| fs::read(p).ok().map(|d| (p, d))) {
            let fname = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "rollout.jsonl".to_string());
            let content_type = "text/plain".to_string();
            envelope.add_item(EnvelopeItem::Attachment(Attachment {
                buffer: data,
                filename: fname,
                content_type: Some(content_type),
                ty: None,
            }));
        }

        client.send_envelope(envelope);
        client.flush(Some(Duration::from_secs(UPLOAD_TIMEOUT_SECS)));
        Ok(())
    }
}

fn clamp_feedback_log_attachment_bytes(mut logs: Vec<u8>, max_bytes: usize) -> Vec<u8> {
    if logs.len() <= max_bytes {
        return logs;
    }
    let start = logs.len() - max_bytes;
    logs.split_off(start)
}

fn display_classification(classification: &str) -> String {
    match classification {
        "bug" => "Bug".to_string(),
        "bad_result" => "Bad result".to_string(),
        "good_result" => "Good result".to_string(),
        _ => "Other".to_string(),
    }
}

#[derive(Clone)]
struct FeedbackMetadataLayer {
    inner: Arc<FeedbackInner>,
}

impl<S> Layer<S> for FeedbackMetadataLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        // This layer is filtered by `Targets`, but keep the guard anyway in case it is used without
        // the filter.
        if event.metadata().target() != FEEDBACK_TAGS_TARGET {
            return;
        }

        let mut visitor = FeedbackTagsVisitor::default();
        event.record(&mut visitor);
        if visitor.tags.is_empty() {
            return;
        }

        let mut guard = self.inner.tags.lock().expect("mutex poisoned");
        for (key, value) in visitor.tags {
            if guard.len() >= MAX_FEEDBACK_TAGS && !guard.contains_key(&key) {
                continue;
            }
            guard.insert(key, value);
        }
    }
}

#[derive(Default)]
struct FeedbackTagsVisitor {
    tags: BTreeMap<String, String>,
}

impl Visit for FeedbackTagsVisitor {
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.tags
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.tags
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.tags
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.tags
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.tags
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.tags
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    #[test]
    fn ring_buffer_drops_front_when_full() {
        let fb = CodexFeedback::with_capacity(8);
        {
            let mut w = fb.make_writer().make_writer();
            w.write_all(b"abcdefgh").unwrap();
            w.write_all(b"ij").unwrap();
        }
        let snap = fb.snapshot(None);
        // Capacity 8: after writing 10 bytes, we should keep the last 8.
        pretty_assertions::assert_eq!(std::str::from_utf8(snap.as_bytes()).unwrap(), "cdefghij");
    }

    #[test]
    fn metadata_layer_records_tags_from_feedback_target() {
        let fb = CodexFeedback::new();
        let _guard = tracing_subscriber::registry()
            .with(fb.metadata_layer())
            .set_default();

        tracing::info!(target: FEEDBACK_TAGS_TARGET, model = "gpt-5", cached = true, "tags");

        let snap = fb.snapshot(None);
        pretty_assertions::assert_eq!(snap.tags.get("model").map(String::as_str), Some("gpt-5"));
        pretty_assertions::assert_eq!(snap.tags.get("cached").map(String::as_str), Some("true"));
    }

    #[test]
    fn clamp_feedback_log_attachment_bytes_keeps_trailing_bytes() {
        let capped = clamp_feedback_log_attachment_bytes(vec![1, 2, 3, 4, 5], 3);
        pretty_assertions::assert_eq!(capped, vec![3, 4, 5]);
    }
}
