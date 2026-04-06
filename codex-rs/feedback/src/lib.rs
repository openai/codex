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
use codex_login::AuthEnvTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use feedback_diagnostics::FEEDBACK_DIAGNOSTICS_ATTACHMENT_FILENAME;
use feedback_diagnostics::FeedbackDiagnostics;
use serde::Serialize;
use tracing::Event;
use tracing::Level;
use tracing::field::Visit;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::fmt::writer::MakeWriter;
use tracing_subscriber::registry::LookupSpan;

pub mod feedback_diagnostics;

const DEFAULT_MAX_BYTES: usize = 4 * 1024 * 1024; // 4 MiB
const SENTRY_DSN: &str =
    "https://ae32ed50620d7a7792c1ce5df38b3e3e@o33249.ingest.us.sentry.io/4510195390611458";
const UPLOAD_TIMEOUT_SECS: u64 = 10;
const FEEDBACK_MIRROR_URL_ENV: &str = "CODEX_FEEDBACK_MIRROR_URL";
const FEEDBACK_MIRROR_AUTHORIZATION_ENV: &str = "CODEX_FEEDBACK_MIRROR_AUTHORIZATION";
const FEEDBACK_TAGS_TARGET: &str = "feedback_tags";
const MAX_FEEDBACK_TAGS: usize = 64;

/// Structured request/auth fields that should be attached to feedback uploads.
pub struct FeedbackRequestTags<'a> {
    pub endpoint: &'a str,
    pub auth_header_attached: bool,
    pub auth_header_name: Option<&'a str>,
    pub auth_mode: Option<&'a str>,
    pub auth_retry_after_unauthorized: Option<bool>,
    pub auth_recovery_mode: Option<&'a str>,
    pub auth_recovery_phase: Option<&'a str>,
    pub auth_connection_reused: Option<bool>,
    pub auth_request_id: Option<&'a str>,
    pub auth_cf_ray: Option<&'a str>,
    pub auth_error: Option<&'a str>,
    pub auth_error_code: Option<&'a str>,
    pub auth_recovery_followup_success: Option<bool>,
    pub auth_recovery_followup_status: Option<u16>,
}

struct FeedbackRequestSnapshot<'a> {
    endpoint: &'a str,
    auth_header_attached: bool,
    auth_header_name: &'a str,
    auth_mode: &'a str,
    auth_retry_after_unauthorized: String,
    auth_recovery_mode: &'a str,
    auth_recovery_phase: &'a str,
    auth_connection_reused: String,
    auth_request_id: &'a str,
    auth_cf_ray: &'a str,
    auth_error: &'a str,
    auth_error_code: &'a str,
    auth_recovery_followup_success: String,
    auth_recovery_followup_status: String,
}

impl<'a> FeedbackRequestSnapshot<'a> {
    fn from_tags(tags: &'a FeedbackRequestTags<'a>) -> Self {
        Self {
            endpoint: tags.endpoint,
            auth_header_attached: tags.auth_header_attached,
            auth_header_name: tags.auth_header_name.unwrap_or(""),
            auth_mode: tags.auth_mode.unwrap_or(""),
            auth_retry_after_unauthorized: tags
                .auth_retry_after_unauthorized
                .map_or_else(String::new, |value| value.to_string()),
            auth_recovery_mode: tags.auth_recovery_mode.unwrap_or(""),
            auth_recovery_phase: tags.auth_recovery_phase.unwrap_or(""),
            auth_connection_reused: tags
                .auth_connection_reused
                .map_or_else(String::new, |value| value.to_string()),
            auth_request_id: tags.auth_request_id.unwrap_or(""),
            auth_cf_ray: tags.auth_cf_ray.unwrap_or(""),
            auth_error: tags.auth_error.unwrap_or(""),
            auth_error_code: tags.auth_error_code.unwrap_or(""),
            auth_recovery_followup_success: tags
                .auth_recovery_followup_success
                .map_or_else(String::new, |value| value.to_string()),
            auth_recovery_followup_status: tags
                .auth_recovery_followup_status
                .map_or_else(String::new, |value| value.to_string()),
        }
    }
}

pub fn emit_feedback_request_tags(tags: &FeedbackRequestTags<'_>) {
    let snapshot = FeedbackRequestSnapshot::from_tags(tags);
    tracing::info!(
        target: FEEDBACK_TAGS_TARGET,
        endpoint = tracing::field::debug(snapshot.endpoint),
        auth_header_attached = tracing::field::debug(snapshot.auth_header_attached),
        auth_header_name = tracing::field::debug(snapshot.auth_header_name),
        auth_mode = tracing::field::debug(snapshot.auth_mode),
        auth_retry_after_unauthorized = tracing::field::debug(&snapshot.auth_retry_after_unauthorized),
        auth_recovery_mode = tracing::field::debug(snapshot.auth_recovery_mode),
        auth_recovery_phase = tracing::field::debug(snapshot.auth_recovery_phase),
        auth_connection_reused = tracing::field::debug(&snapshot.auth_connection_reused),
        auth_request_id = tracing::field::debug(snapshot.auth_request_id),
        auth_cf_ray = tracing::field::debug(snapshot.auth_cf_ray),
        auth_error = tracing::field::debug(snapshot.auth_error),
        auth_error_code = tracing::field::debug(snapshot.auth_error_code),
        auth_recovery_followup_success = tracing::field::debug(&snapshot.auth_recovery_followup_success),
        auth_recovery_followup_status = tracing::field::debug(&snapshot.auth_recovery_followup_status),
    );
}

pub fn emit_feedback_request_tags_with_auth_env(
    tags: &FeedbackRequestTags<'_>,
    auth_env: &AuthEnvTelemetry,
) {
    let snapshot = FeedbackRequestSnapshot::from_tags(tags);
    tracing::info!(
        target: FEEDBACK_TAGS_TARGET,
        endpoint = tracing::field::debug(snapshot.endpoint),
        auth_header_attached = tracing::field::debug(snapshot.auth_header_attached),
        auth_header_name = tracing::field::debug(snapshot.auth_header_name),
        auth_mode = tracing::field::debug(snapshot.auth_mode),
        auth_retry_after_unauthorized = tracing::field::debug(&snapshot.auth_retry_after_unauthorized),
        auth_recovery_mode = tracing::field::debug(snapshot.auth_recovery_mode),
        auth_recovery_phase = tracing::field::debug(snapshot.auth_recovery_phase),
        auth_connection_reused = tracing::field::debug(&snapshot.auth_connection_reused),
        auth_request_id = tracing::field::debug(snapshot.auth_request_id),
        auth_cf_ray = tracing::field::debug(snapshot.auth_cf_ray),
        auth_error = tracing::field::debug(snapshot.auth_error),
        auth_error_code = tracing::field::debug(snapshot.auth_error_code),
        auth_recovery_followup_success = tracing::field::debug(&snapshot.auth_recovery_followup_success),
        auth_recovery_followup_status = tracing::field::debug(&snapshot.auth_recovery_followup_status),
        auth_env_openai_api_key_present = tracing::field::debug(auth_env.openai_api_key_env_present),
        auth_env_codex_api_key_present = tracing::field::debug(auth_env.codex_api_key_env_present),
        auth_env_codex_api_key_enabled = tracing::field::debug(auth_env.codex_api_key_env_enabled),
        // Custom provider `env_key` is arbitrary config text, so emit only a safe bucket.
        auth_env_provider_key_name = tracing::field::debug(
            auth_env.provider_env_key_name.as_deref().unwrap_or("")
        ),
        auth_env_provider_key_present = tracing::field::debug(
            &auth_env.provider_env_key_present.map_or_else(String::new, |value| value.to_string())
        ),
        auth_env_refresh_token_url_override_present = tracing::field::debug(
            auth_env.refresh_token_url_override_present
        ),
    );
}

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
            .with_timer(tracing_subscriber::fmt::time::SystemTime)
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

    pub fn snapshot(&self, session_id: Option<ThreadId>) -> FeedbackSnapshot {
        let bytes = {
            #[allow(clippy::expect_used)]
            let guard = self.inner.ring.lock().expect("mutex poisoned");
            guard.snapshot_bytes()
        };
        let tags = {
            #[allow(clippy::expect_used)]
            let guard = self.inner.tags.lock().expect("mutex poisoned");
            guard.clone()
        };
        FeedbackSnapshot {
            bytes,
            tags,
            feedback_diagnostics: FeedbackDiagnostics::collect_from_env(),
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

pub struct FeedbackSnapshot {
    bytes: Vec<u8>,
    tags: BTreeMap<String, String>,
    feedback_diagnostics: FeedbackDiagnostics,
    pub thread_id: String,
}

pub struct FeedbackUploadOptions<'a> {
    pub include_logs: bool,
    pub extra_attachment_paths: &'a [PathBuf],
    pub session_source: Option<SessionSource>,
    pub logs_override: Option<Vec<u8>>,
    pub allow_feedback_mirror_upload: bool,
}

impl FeedbackSnapshot {
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn feedback_diagnostics(&self) -> &FeedbackDiagnostics {
        &self.feedback_diagnostics
    }

    pub fn with_feedback_diagnostics(mut self, feedback_diagnostics: FeedbackDiagnostics) -> Self {
        self.feedback_diagnostics = feedback_diagnostics;
        self
    }

    pub fn feedback_diagnostics_attachment_text(&self, include_logs: bool) -> Option<String> {
        if !include_logs {
            return None;
        }

        self.feedback_diagnostics.attachment_text()
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
        options: FeedbackUploadOptions<'_>,
    ) -> Result<()> {
        use std::collections::BTreeMap;
        use std::str::FromStr;
        use std::sync::Arc;

        use sentry::Client;
        use sentry::ClientOptions;
        use sentry::protocol::Envelope;
        use sentry::protocol::EnvelopeItem;
        use sentry::protocol::Event;
        use sentry::protocol::Level;
        use sentry::transports::DefaultTransportFactory;
        use sentry::types::Dsn;

        let FeedbackUploadOptions {
            include_logs,
            extra_attachment_paths,
            session_source,
            logs_override,
            allow_feedback_mirror_upload,
        } = options;

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
            "bug" | "bad_result" | "safety_check" => Level::Error,
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
                ty: title,
                value: Some(r.to_string()),
                ..Default::default()
            }]);
        }
        envelope.add_item(EnvelopeItem::Event(event));

        for attachment in
            self.feedback_attachments(include_logs, extra_attachment_paths, logs_override)
        {
            envelope.add_item(EnvelopeItem::Attachment(attachment));
        }

        client.send_envelope(envelope);
        client.flush(Some(Duration::from_secs(UPLOAD_TIMEOUT_SECS)));
        if allow_feedback_mirror_upload
            && let Err(err) =
                self.upload_feedback_to_mirror(classification, reason, include_logs, session_source)
        {
            tracing::warn!(
                error = %err,
                thread_id = %self.thread_id,
                "failed to mirror feedback to configured endpoint"
            );
        }
        Ok(())
    }

    fn feedback_attachments(
        &self,
        include_logs: bool,
        extra_attachment_paths: &[PathBuf],
        logs_override: Option<Vec<u8>>,
    ) -> Vec<sentry::protocol::Attachment> {
        use sentry::protocol::Attachment;

        let mut attachments = Vec::new();

        if include_logs {
            attachments.push(Attachment {
                buffer: logs_override.unwrap_or_else(|| self.bytes.clone()),
                filename: String::from("codex-logs.log"),
                content_type: Some("text/plain".to_string()),
                ty: None,
            });
        }

        if let Some(text) = self.feedback_diagnostics_attachment_text(include_logs) {
            attachments.push(Attachment {
                buffer: text.into_bytes(),
                filename: FEEDBACK_DIAGNOSTICS_ATTACHMENT_FILENAME.to_string(),
                content_type: Some("text/plain".to_string()),
                ty: None,
            });
        }

        for path in extra_attachment_paths {
            let data = match fs::read(path) {
                Ok(data) => data,
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %err,
                        "failed to read log attachment; skipping"
                    );
                    continue;
                }
            };
            let filename = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "extra-log.log".to_string());
            attachments.push(Attachment {
                buffer: data,
                filename,
                content_type: Some("text/plain".to_string()),
                ty: None,
            });
        }

        attachments
    }

    fn upload_feedback_to_mirror(
        &self,
        classification: &str,
        reason: Option<&str>,
        include_logs: bool,
        session_source: Option<SessionSource>,
    ) -> Result<()> {
        let Some(submit_url) = feedback_mirror_url() else {
            return Ok(());
        };
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(UPLOAD_TIMEOUT_SECS))
            .build()?;
        let mut request = client
            .post(submit_url)
            .json(&create_feedback_mirror_payload(
                &self.thread_id,
                classification,
                reason,
                include_logs,
                session_source,
            ));
        if let Some(authorization) = feedback_mirror_authorization() {
            request = request.header(reqwest::header::AUTHORIZATION, authorization);
        }
        let response = request.send()?;
        if response.status().is_success() {
            return Ok(());
        }
        let status = response.status();
        let body = response.text().unwrap_or_default();
        Err(anyhow!(
            "feedback mirror upload failed with status {status}{}",
            if body.is_empty() {
                String::new()
            } else {
                format!(": {body}")
            }
        ))
    }
}

fn feedback_mirror_url() -> Option<String> {
    std::env::var(FEEDBACK_MIRROR_URL_ENV)
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

fn feedback_mirror_authorization() -> Option<String> {
    std::env::var(FEEDBACK_MIRROR_AUTHORIZATION_ENV)
        .ok()
        .map(|authorization| authorization.trim().to_string())
        .filter(|authorization| !authorization.is_empty())
}

pub fn is_openai_employee_email(account_email: Option<&str>) -> bool {
    let Some(email) = account_email.map(str::trim) else {
        return false;
    };
    let Some((_, domain)) = email.rsplit_once('@') else {
        return false;
    };
    domain.eq_ignore_ascii_case("openai.com")
}

#[derive(Serialize)]
struct FeedbackMirrorPayload<'a> {
    feedback: FeedbackMirrorBody<'a>,
    thread_id: &'a str,
    client: FeedbackMirrorClient,
    source: FeedbackMirrorSource<'a>,
}

#[derive(Serialize)]
struct FeedbackMirrorBody<'a> {
    text: &'a str,
    classification: &'a str,
}

#[derive(Serialize)]
struct FeedbackMirrorClient {
    client_name: &'static str,
    style: &'static str,
    payload_mode: &'static str,
}

#[derive(Serialize)]
struct FeedbackMirrorSource<'a> {
    source: &'static str,
    cli_version: &'static str,
    session_source: Option<String>,
    sentry_correlation_id: &'a str,
    include_logs_requested: bool,
}

fn create_feedback_mirror_payload<'a>(
    thread_id: &'a str,
    classification: &'a str,
    reason: Option<&'a str>,
    include_logs: bool,
    session_source: Option<SessionSource>,
) -> FeedbackMirrorPayload<'a> {
    let text = reason
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .unwrap_or(classification);
    FeedbackMirrorPayload {
        feedback: FeedbackMirrorBody {
            text,
            classification,
        },
        thread_id,
        client: FeedbackMirrorClient {
            client_name: "codex-feedback",
            style: "feedback_mirror",
            payload_mode: "raw_feedback_without_observed_artifacts",
        },
        source: FeedbackMirrorSource {
            source: "codex_feedback_upload",
            cli_version: env!("CARGO_PKG_VERSION"),
            session_source: session_source.map(|source| source.to_string()),
            sentry_correlation_id: thread_id,
            include_logs_requested: include_logs,
        },
    }
}

fn display_classification(classification: &str) -> String {
    match classification {
        "bug" => "Bug".to_string(),
        "bad_result" => "Bad result".to_string(),
        "good_result" => "Good result".to_string(),
        "safety_check" => "Safety check".to_string(),
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

        #[allow(clippy::expect_used)]
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
    use std::ffi::OsStr;
    use std::fs;

    use super::*;
    use feedback_diagnostics::FeedbackDiagnostic;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    #[test]
    fn ring_buffer_drops_front_when_full() {
        let fb = CodexFeedback::with_capacity(/*max_bytes*/ 8);
        {
            let mut w = fb.make_writer().make_writer();
            w.write_all(b"abcdefgh").unwrap();
            w.write_all(b"ij").unwrap();
        }
        let snap = fb.snapshot(/*session_id*/ None);
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

        let snap = fb.snapshot(/*session_id*/ None);
        pretty_assertions::assert_eq!(snap.tags.get("model").map(String::as_str), Some("gpt-5"));
        pretty_assertions::assert_eq!(snap.tags.get("cached").map(String::as_str), Some("true"));
    }

    #[test]
    fn feedback_attachments_gate_connectivity_diagnostics() {
        let extra_filename = format!("codex-feedback-extra-{}.jsonl", ThreadId::new());
        let extra_path = std::env::temp_dir().join(&extra_filename);
        fs::write(&extra_path, "rollout").expect("extra attachment should be written");

        let snapshot_with_diagnostics = CodexFeedback::new()
            .snapshot(/*session_id*/ None)
            .with_feedback_diagnostics(FeedbackDiagnostics::new(vec![FeedbackDiagnostic {
                headline: "Proxy environment variables are set and may affect connectivity."
                    .to_string(),
                details: vec!["HTTPS_PROXY = https://example.com:443".to_string()],
            }]));

        let attachments_with_diagnostics = snapshot_with_diagnostics.feedback_attachments(
            /*include_logs*/ true,
            std::slice::from_ref(&extra_path),
            Some(vec![1]),
        );

        assert_eq!(
            attachments_with_diagnostics
                .iter()
                .map(|attachment| attachment.filename.as_str())
                .collect::<Vec<_>>(),
            vec![
                "codex-logs.log",
                FEEDBACK_DIAGNOSTICS_ATTACHMENT_FILENAME,
                extra_filename.as_str()
            ]
        );
        assert_eq!(attachments_with_diagnostics[0].buffer, vec![1]);
        assert_eq!(
            attachments_with_diagnostics[1].buffer,
            b"Connectivity diagnostics\n\n- Proxy environment variables are set and may affect connectivity.\n  - HTTPS_PROXY = https://example.com:443".to_vec()
        );
        assert_eq!(attachments_with_diagnostics[2].buffer, b"rollout".to_vec());
        assert_eq!(
            OsStr::new(attachments_with_diagnostics[2].filename.as_str()),
            OsStr::new(extra_filename.as_str())
        );
        let attachments_without_diagnostics = CodexFeedback::new()
            .snapshot(/*session_id*/ None)
            .feedback_attachments(/*include_logs*/ true, &[], Some(vec![1]));

        assert_eq!(
            attachments_without_diagnostics
                .iter()
                .map(|attachment| attachment.filename.as_str())
                .collect::<Vec<_>>(),
            vec!["codex-logs.log"]
        );
        assert_eq!(attachments_without_diagnostics[0].buffer, vec![1]);
        fs::remove_file(extra_path).expect("extra attachment should be removed");
    }

    #[test]
    fn is_openai_employee_email_only_for_openai_email() {
        pretty_assertions::assert_eq!(
            is_openai_employee_email(/*account_email*/ Some("user@openai.com")),
            true
        );
        pretty_assertions::assert_eq!(
            is_openai_employee_email(/*account_email*/ Some(" User@OpenAI.com ")),
            true
        );
        pretty_assertions::assert_eq!(
            is_openai_employee_email(/*account_email*/ Some("user@example.com")),
            false
        );
        pretty_assertions::assert_eq!(is_openai_employee_email(/*account_email*/ None), false);
    }

    #[test]
    fn feedback_mirror_payload_omits_logs_and_email() {
        let payload = create_feedback_mirror_payload(
            "thread-123",
            "bug",
            Some("  bad patch  "),
            /*include_logs*/ true,
            /*session_source*/ Some(SessionSource::Cli),
        );

        pretty_assertions::assert_eq!(
            serde_json::to_value(payload).unwrap(),
            json!({
                "feedback": {
                    "text": "bad patch",
                    "classification": "bug",
                },
                "thread_id": "thread-123",
                "client": {
                    "client_name": "codex-feedback",
                    "style": "feedback_mirror",
                    "payload_mode": "raw_feedback_without_observed_artifacts",
                },
                "source": {
                    "source": "codex_feedback_upload",
                    "cli_version": env!("CARGO_PKG_VERSION"),
                    "session_source": "cli",
                    "sentry_correlation_id": "thread-123",
                    "include_logs_requested": true,
                },
            })
        );
    }

    #[test]
    fn feedback_mirror_payload_uses_classification_when_reason_is_empty() {
        let payload = create_feedback_mirror_payload(
            "thread-123",
            "bug",
            Some(" "),
            /*include_logs*/ false,
            /*session_source*/ None,
        );

        pretty_assertions::assert_eq!(
            serde_json::to_value(payload).unwrap()["feedback"],
            json!({
                "text": "bug",
                "classification": "bug",
            })
        );
    }
}
