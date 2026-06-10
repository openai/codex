use crate::events::TrackEventRequest;
use crate::now_unix_millis;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;

pub const LOCAL_ANALYTICS_SCHEMA_VERSION: u32 = 1;
pub const LOCAL_ANALYTICS_SINK_PATH_ENV_VAR: &str = "CODEX_ANALYTICS_LOCAL_SINK_PATH";

pub(crate) type SharedLocalAnalyticsSink = Arc<Mutex<LocalAnalyticsSink>>;

static PROCESS_LOCAL_SINKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<LocalAnalyticsSink>>>>> =
    OnceLock::new();

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalAnalyticsRecordType {
    CodexAnalyticsEvent,
    ResponsesApiCall,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LocalAnalyticsRecord {
    pub schema_version: u32,
    pub recorded_at_epoch_millis: u64,
    pub record_type: LocalAnalyticsRecordType,
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub payload: JsonValue,
}

pub(crate) fn local_analytics_sink_from_env() -> Option<SharedLocalAnalyticsSink> {
    let path = std::env::var_os(LOCAL_ANALYTICS_SINK_PATH_ENV_VAR)?;
    if path.is_empty() {
        return None;
    }

    local_analytics_sink_for_path(PathBuf::from(path))
}

pub(crate) fn append_codex_analytics_event_best_effort(
    sink: &SharedLocalAnalyticsSink,
    event: &TrackEventRequest,
) {
    let Some(record) = LocalAnalyticsRecord::from_codex_analytics_event(event) else {
        return;
    };
    append_local_analytics_record_best_effort(sink, &record);
}

pub(crate) fn append_local_analytics_record_best_effort(
    sink: &SharedLocalAnalyticsSink,
    record: &LocalAnalyticsRecord,
) {
    let result = sink
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .append(record);
    if let Err(err) = result {
        tracing::warn!(error = %err, "failed to append local analytics record");
    }
}

pub(crate) fn local_analytics_sink_for_path(path: PathBuf) -> Option<SharedLocalAnalyticsSink> {
    let sinks = PROCESS_LOCAL_SINKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut sinks = sinks
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    sinks.retain(|_, sink| sink.strong_count() > 0);
    if let Some(sink) = sinks.get(&path).and_then(Weak::upgrade) {
        return Some(sink);
    }

    match LocalAnalyticsSink::open(path.clone()) {
        Ok(sink) => {
            let sink = Arc::new(Mutex::new(sink));
            sinks.insert(path, Arc::downgrade(&sink));
            Some(sink)
        }
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "failed to initialize local analytics sink"
            );
            None
        }
    }
}

impl LocalAnalyticsRecord {
    fn from_codex_analytics_event(event: &TrackEventRequest) -> Option<Self> {
        let payload = match serde_json::to_value(event) {
            Ok(payload) => payload,
            Err(err) => {
                tracing::warn!(error = %err, "failed to serialize local analytics event");
                return None;
            }
        };
        let event_params = payload.get("event_params");
        Some(Self {
            schema_version: LOCAL_ANALYTICS_SCHEMA_VERSION,
            recorded_at_epoch_millis: now_unix_millis(),
            record_type: LocalAnalyticsRecordType::CodexAnalyticsEvent,
            session_id: string_field(event_params, "session_id"),
            thread_id: string_field(event_params, "thread_id"),
            turn_id: string_field(event_params, "turn_id"),
            payload,
        })
    }
}

pub(crate) struct LocalAnalyticsSink {
    writer: BufWriter<File>,
}

impl LocalAnalyticsSink {
    fn open(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    fn append(&mut self, record: &LocalAnalyticsRecord) -> std::io::Result<()> {
        serde_json::to_writer(&mut self.writer, record)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()
    }
}

fn string_field(value: Option<&JsonValue>, field: &str) -> Option<String> {
    value?
        .get(field)?
        .as_str()
        .map(std::string::ToString::to_string)
}

#[cfg(test)]
#[path = "local_sink_tests.rs"]
mod tests;
