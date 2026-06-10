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
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;

pub const LOCAL_ANALYTICS_SCHEMA_VERSION: u32 = 1;
const LOCAL_ANALYTICS_SINK_PATH_ENV_VAR: &str = "CODEX_ANALYTICS_LOCAL_SINK_PATH";

type LocalAnalyticsWriter = BufWriter<File>;
pub(crate) type SharedLocalAnalyticsSink = Arc<Mutex<LocalAnalyticsWriter>>;

static PROCESS_LOCAL_SINKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<LocalAnalyticsWriter>>>>> =
    OnceLock::new();

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalAnalyticsRecordType {
    CodexAnalyticsEvent,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
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

pub(super) fn write(sink: &SharedLocalAnalyticsSink, events: &[TrackEventRequest]) {
    let mut writer = sink
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for event in events {
        let Some(record) = LocalAnalyticsRecord::from_codex_analytics_event(event) else {
            continue;
        };
        if let Err(err) = append_record(&mut writer, &record) {
            tracing::warn!(error = %err, "failed to append local analytics record");
        }
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

    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => {
            let sink = Arc::new(Mutex::new(BufWriter::new(file)));
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

fn append_record(
    writer: &mut BufWriter<File>,
    record: &LocalAnalyticsRecord,
) -> std::io::Result<()> {
    serde_json::to_writer(&mut *writer, record)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn string_field(value: Option<&JsonValue>, field: &str) -> Option<String> {
    value?
        .get(field)?
        .as_str()
        .map(std::string::ToString::to_string)
}

#[cfg(test)]
#[path = "local_tests.rs"]
mod tests;
