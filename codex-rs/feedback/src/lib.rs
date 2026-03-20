use std::collections::BTreeMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::collections::btree_map::Entry;
use std::fs;
use std::io::Write;
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use feedback_diagnostics::FEEDBACK_DIAGNOSTICS_ATTACHMENT_FILENAME;
use feedback_diagnostics::FeedbackDiagnostics;
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
const FEEDBACK_TAGS_TARGET: &str = "feedback_tags";
const MAX_FEEDBACK_TAGS: usize = 64;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeedbackUploadArtifacts {
    pub attachment_paths: Vec<PathBuf>,
    pub logs_override: Option<Vec<u8>>,
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
        self.snapshot_with_sqlite_home(session_id, /*sqlite_home*/ None)
    }

    pub fn snapshot_with_sqlite_home(
        &self,
        session_id: Option<ThreadId>,
        sqlite_home: Option<PathBuf>,
    ) -> FeedbackSnapshot {
        let bytes = {
            let guard = self.inner.ring.lock().expect("mutex poisoned");
            guard.snapshot_bytes()
        };
        let tags = {
            let guard = self.inner.tags.lock().expect("mutex poisoned");
            guard.clone()
        };
        FeedbackSnapshot {
            bytes,
            tags,
            feedback_diagnostics: FeedbackDiagnostics::collect_from_env(),
            sqlite_home,
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
    sqlite_home: Option<PathBuf>,
    pub thread_id: String,
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
        include_logs: bool,
        extra_attachment_paths: &[PathBuf],
        session_source: Option<SessionSource>,
        logs_override: Option<Vec<u8>>,
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
                ty: title.clone(),
                value: Some(r.to_string()),
                ..Default::default()
            }]);
        }
        envelope.add_item(EnvelopeItem::Event(event));

        let discovered_upload_artifacts = if include_logs {
            discover_feedback_upload_artifacts(&self.thread_id, self.sqlite_home.as_deref())
        } else {
            FeedbackUploadArtifacts::default()
        };
        let merged_attachment_paths = merge_attachment_paths(
            extra_attachment_paths,
            &discovered_upload_artifacts.attachment_paths,
        );
        let merged_logs_override = logs_override.or(discovered_upload_artifacts.logs_override);

        for attachment in
            self.feedback_attachments(include_logs, &merged_attachment_paths, merged_logs_override)
        {
            envelope.add_item(EnvelopeItem::Attachment(attachment));
        }

        client.send_envelope(envelope);
        client.flush(Some(Duration::from_secs(UPLOAD_TIMEOUT_SECS)));
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
}

fn merge_attachment_paths(
    extra_attachment_paths: &[PathBuf],
    discovered_attachment_paths: &[PathBuf],
) -> Vec<PathBuf> {
    let mut merged = Vec::with_capacity(
        extra_attachment_paths
            .len()
            .saturating_add(discovered_attachment_paths.len()),
    );
    let mut seen_paths = HashSet::<PathBuf>::new();
    for path in extra_attachment_paths
        .iter()
        .chain(discovered_attachment_paths.iter())
    {
        if seen_paths.insert(path.clone()) {
            merged.push(path.clone());
        }
    }
    merged
}

fn discover_feedback_upload_artifacts(
    thread_id: &str,
    sqlite_home: Option<&Path>,
) -> FeedbackUploadArtifacts {
    let Ok(root_thread_id) = ThreadId::from_string(thread_id) else {
        return FeedbackUploadArtifacts::default();
    };
    let sqlite_home = match sqlite_home {
        Some(sqlite_home) => sqlite_home.to_path_buf(),
        None => match codex_utils_home_dir::find_codex_home() {
            Ok(codex_home) => codex_home,
            Err(_) => return FeedbackUploadArtifacts::default(),
        },
    };
    if !codex_state::state_db_path(sqlite_home.as_path()).exists() {
        return FeedbackUploadArtifacts::default();
    }

    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(err) => {
                tracing::warn!(
                    "failed to build tokio runtime for feedback artifact discovery: {err}"
                );
                return FeedbackUploadArtifacts::default();
            }
        };
        runtime.block_on(collect_feedback_upload_artifacts(
            sqlite_home,
            root_thread_id,
        ))
    })
    .join()
    .unwrap_or_else(|_| FeedbackUploadArtifacts::default())
}

async fn collect_feedback_upload_artifacts(
    codex_home: PathBuf,
    root_thread_id: ThreadId,
) -> FeedbackUploadArtifacts {
    let mut artifacts = FeedbackUploadArtifacts::default();
    let state_db = match open_feedback_state_db(codex_home.as_path()).await {
        Some(state_db) => state_db,
        None => return artifacts,
    };

    if let Some(root_rollout_path) =
        find_rollout_path_by_id(state_db.as_ref(), root_thread_id).await
    {
        artifacts.attachment_paths.push(root_rollout_path);
    }

    let descendant_ids = match state_db.list_thread_spawn_descendants(root_thread_id).await {
        Ok(descendant_ids) => descendant_ids,
        Err(err) => {
            tracing::warn!("failed to list descendant threads for feedback upload: {err}");
            Vec::new()
        }
    };
    let mut seen_paths = artifacts
        .attachment_paths
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    for descendant_id in &descendant_ids {
        if let Some(path) = find_rollout_path_by_id(state_db.as_ref(), *descendant_id).await
            && seen_paths.insert(path.clone())
        {
            artifacts.attachment_paths.push(path);
        }
    }

    let mut log_thread_ids = Vec::with_capacity(descendant_ids.len().saturating_add(1));
    log_thread_ids.push(root_thread_id.to_string());
    log_thread_ids.extend(
        descendant_ids
            .into_iter()
            .map(|thread_id| thread_id.to_string()),
    );
    artifacts.logs_override = match state_db
        .query_feedback_logs_for_threads(&log_thread_ids)
        .await
    {
        Ok(logs) if logs.is_empty() => None,
        Ok(logs) => Some(logs),
        Err(err) => {
            tracing::warn!("failed to query feedback logs from sqlite for upload: {err}");
            None
        }
    };

    artifacts
}

async fn open_feedback_state_db(codex_home: &Path) -> Option<Arc<codex_state::StateRuntime>> {
    codex_state::StateRuntime::init(codex_home.to_path_buf(), String::new())
        .await
        .ok()
}

async fn find_rollout_path_by_id(
    state_db: &codex_state::StateRuntime,
    thread_id: ThreadId,
) -> Option<PathBuf> {
    state_db
        .find_rollout_path_by_id(thread_id, /*archived_only*/ None)
        .await
        .unwrap_or_else(|err| {
            tracing::warn!("failed to resolve rollout path during feedback upload: {err}");
            None
        })
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
    use chrono::TimeZone;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::SubAgentSource;
    use codex_state::LogEntry;
    use codex_state::ThreadMetadataBuilder;
    use feedback_diagnostics::FeedbackDiagnostic;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;
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
    fn feedback_attachments_gate_connectivity_diagnostics() {
        let extra_filename = format!("codex-feedback-extra-{}.jsonl", ThreadId::new());
        let extra_path = std::env::temp_dir().join(&extra_filename);
        fs::write(&extra_path, "rollout").expect("extra attachment should be written");

        let snapshot_with_diagnostics = CodexFeedback::new()
            .snapshot(None)
            .with_feedback_diagnostics(FeedbackDiagnostics::new(vec![FeedbackDiagnostic {
                headline: "OPENAI_BASE_URL is set and may affect connectivity.".to_string(),
                details: vec!["OPENAI_BASE_URL = https://example.com/v1".to_string()],
            }]));

        let attachments_with_diagnostics = snapshot_with_diagnostics.feedback_attachments(
            true,
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
            b"Connectivity diagnostics\n\n- OPENAI_BASE_URL is set and may affect connectivity.\n  - OPENAI_BASE_URL = https://example.com/v1".to_vec()
        );
        assert_eq!(attachments_with_diagnostics[2].buffer, b"rollout".to_vec());
        assert_eq!(
            OsStr::new(attachments_with_diagnostics[2].filename.as_str()),
            OsStr::new(extra_filename.as_str())
        );
        let attachments_without_diagnostics = CodexFeedback::new()
            .snapshot(None)
            .with_feedback_diagnostics(FeedbackDiagnostics::new(vec![]))
            .feedback_attachments(true, &[], Some(vec![1]));

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

    #[tokio::test]
    async fn collect_feedback_upload_artifacts_includes_child_rollouts_and_logs() {
        let temp_dir = tempdir().expect("temp dir should exist");
        let codex_home = temp_dir.path().to_path_buf();
        let state_db = codex_state::StateRuntime::init(codex_home.clone(), String::new())
            .await
            .expect("state db should initialize");
        state_db
            .mark_backfill_complete(None)
            .await
            .expect("backfill should be marked complete");

        let root_thread_id = ThreadId::from_string("00000000-0000-0000-0000-000000000111")
            .expect("root thread id should parse");
        let child_thread_id = ThreadId::from_string("00000000-0000-0000-0000-000000000222")
            .expect("child thread id should parse");
        let created_at = chrono::Utc
            .timestamp_opt(1_742_050_000, 0)
            .single()
            .expect("valid ts");
        let root_rollout_path = codex_home.join("rollout-root.jsonl");
        let child_rollout_path = codex_home.join("rollout-child.jsonl");

        let root_metadata = ThreadMetadataBuilder::new(
            root_thread_id,
            root_rollout_path.clone(),
            created_at,
            SessionSource::Cli,
        )
        .build("");
        let child_metadata = ThreadMetadataBuilder::new(
            child_thread_id,
            child_rollout_path.clone(),
            created_at,
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: root_thread_id,
                depth: 1,
                agent_nickname: None,
                agent_role: None,
            }),
        )
        .build("");
        state_db
            .upsert_thread(&root_metadata)
            .await
            .expect("root thread should persist");
        state_db
            .upsert_thread(&child_metadata)
            .await
            .expect("child thread should persist");
        state_db
            .upsert_thread_spawn_edge(
                root_thread_id,
                child_thread_id,
                codex_state::DirectionalThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect("spawn edge should persist");
        state_db
            .insert_logs(&[
                LogEntry {
                    ts: 1,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("root-log".to_string()),
                    feedback_log_body: None,
                    thread_id: Some(root_thread_id.to_string()),
                    process_uuid: Some("proc-shared".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 2,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("child-log".to_string()),
                    feedback_log_body: None,
                    thread_id: Some(child_thread_id.to_string()),
                    process_uuid: Some("proc-shared".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
            ])
            .await
            .expect("feedback logs should persist");

        let artifacts = collect_feedback_upload_artifacts(codex_home, root_thread_id).await;

        assert_eq!(
            artifacts.attachment_paths,
            vec![root_rollout_path, child_rollout_path]
        );
        let logs = String::from_utf8(
            artifacts
                .logs_override
                .expect("feedback logs should be collected"),
        )
        .expect("logs should be utf8");
        assert!(logs.contains("root-log"));
        assert!(logs.contains("child-log"));
    }

    #[test]
    fn discover_feedback_upload_artifacts_uses_explicit_sqlite_home() {
        let temp_dir = tempdir().expect("temp dir should exist");
        let codex_home = temp_dir.path().join("codex-home");
        let sqlite_home = temp_dir.path().join("sqlite-home");
        fs::create_dir_all(&codex_home).expect("codex home should exist");
        fs::create_dir_all(&sqlite_home).expect("sqlite home should exist");

        let root_thread_id = ThreadId::from_string("00000000-0000-0000-0000-000000000333")
            .expect("root thread id should parse");
        let child_thread_id = ThreadId::from_string("00000000-0000-0000-0000-000000000444")
            .expect("child thread id should parse");
        let created_at = chrono::Utc
            .timestamp_opt(1_742_050_100, 0)
            .single()
            .expect("valid ts");
        let root_rollout_path = codex_home.join("rollout-root.jsonl");
        let child_rollout_path = codex_home.join("rollout-child.jsonl");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime should build");
        runtime.block_on(async {
            let state_db = codex_state::StateRuntime::init(sqlite_home.clone(), String::new())
                .await
                .expect("state db should initialize");
            state_db
                .mark_backfill_complete(None)
                .await
                .expect("backfill should be marked complete");

            let root_metadata = ThreadMetadataBuilder::new(
                root_thread_id,
                root_rollout_path.clone(),
                created_at,
                SessionSource::Cli,
            )
            .build("");
            let child_metadata = ThreadMetadataBuilder::new(
                child_thread_id,
                child_rollout_path.clone(),
                created_at,
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: root_thread_id,
                    depth: 1,
                    agent_nickname: None,
                    agent_role: None,
                }),
            )
            .build("");
            state_db
                .upsert_thread(&root_metadata)
                .await
                .expect("root thread should persist");
            state_db
                .upsert_thread(&child_metadata)
                .await
                .expect("child thread should persist");
            state_db
                .upsert_thread_spawn_edge(
                    root_thread_id,
                    child_thread_id,
                    codex_state::DirectionalThreadSpawnEdgeStatus::Open,
                )
                .await
                .expect("spawn edge should persist");
            state_db
                .insert_logs(&[
                    LogEntry {
                        ts: 1,
                        ts_nanos: 0,
                        level: "INFO".to_string(),
                        target: "cli".to_string(),
                        message: Some("root-log".to_string()),
                        feedback_log_body: None,
                        thread_id: Some(root_thread_id.to_string()),
                        process_uuid: Some("proc-shared".to_string()),
                        file: None,
                        line: None,
                        module_path: None,
                    },
                    LogEntry {
                        ts: 2,
                        ts_nanos: 0,
                        level: "INFO".to_string(),
                        target: "cli".to_string(),
                        message: Some("child-log".to_string()),
                        feedback_log_body: None,
                        thread_id: Some(child_thread_id.to_string()),
                        process_uuid: Some("proc-shared".to_string()),
                        file: None,
                        line: None,
                        module_path: None,
                    },
                ])
                .await
                .expect("feedback logs should persist");
        });
        fs::write(&root_rollout_path, "root rollout").expect("root rollout should be written");
        fs::write(&child_rollout_path, "child rollout").expect("child rollout should be written");

        let artifacts =
            discover_feedback_upload_artifacts(&root_thread_id.to_string(), Some(&sqlite_home));

        assert_eq!(
            artifacts.attachment_paths,
            vec![root_rollout_path, child_rollout_path]
        );
        let logs = String::from_utf8(
            artifacts
                .logs_override
                .expect("feedback logs should be collected"),
        )
        .expect("logs should be utf8");
        assert!(logs.contains("root-log"));
        assert!(logs.contains("child-log"));
    }
}
