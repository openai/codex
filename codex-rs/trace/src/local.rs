use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::PoisonError;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use serde::Serialize;
use serde_json::json;
use tracing::Level;
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::registry::LookupSpan;

pub const CODEX_TRACE_ROOT_ENV: &str = "CODEX_TRACE_ROOT";
pub const LOCAL_TRACE_TARGET: &str = "codex_otel.trace_safe";

static PAYLOAD_WRITER: OnceLock<Mutex<PayloadWriter>> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RawPayloadRef {
    pub raw_payload_id: String,
    pub kind: String,
    pub path: String,
}

#[derive(Debug)]
struct PayloadWriter {
    payloads_dir: PathBuf,
    next_payload_ordinal: u64,
}

pub fn local_layer_from_env<S>() -> Result<Option<impl Layer<S> + Send + Sync + 'static>>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    let Some(root) = std::env::var_os(CODEX_TRACE_ROOT_ENV) else {
        return Ok(None);
    };
    local_layer_for_root(root).map(Some)
}

fn local_layer_for_root<S>(
    root: impl Into<PathBuf>,
) -> Result<impl Layer<S> + Send + Sync + 'static>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    let root = root.into();
    std::fs::create_dir_all(&root)
        .with_context(|| format!("create trace root {}", root.display()))?;
    let started_at_unix_ms = unix_time_ms();
    let process_id = std::process::id();
    let trace_id = format!("trace-{started_at_unix_ms}-{process_id}");
    let bundle_dir = root.join(&trace_id);
    let payloads_dir = bundle_dir.join("payloads");
    std::fs::create_dir_all(&payloads_dir)
        .with_context(|| format!("create trace payload dir {}", payloads_dir.display()))?;
    write_json_file(
        &bundle_dir.join("manifest.json"),
        &json!({
            "schema_version": 1,
            "trace_id": trace_id,
            "started_at_unix_ms": started_at_unix_ms,
            "event_log": {
                "format": "tracing_subscriber_fmt_json",
                "path": "events.jsonl"
            },
        }),
    )?;

    let writer = PAYLOAD_WRITER.get_or_init(|| {
        Mutex::new(PayloadWriter {
            payloads_dir: PathBuf::new(),
            next_payload_ordinal: 1,
        })
    });
    *writer.lock().unwrap_or_else(PoisonError::into_inner) = PayloadWriter {
        payloads_dir,
        next_payload_ordinal: 1,
    };

    let event_log = tracing_appender::rolling::never(&bundle_dir, "events.jsonl");
    let layer = tracing_subscriber::fmt::layer()
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_writer(event_log)
        .with_filter(Targets::new().with_target(LOCAL_TRACE_TARGET, Level::INFO));
    Ok(layer)
}

pub fn next_id(prefix: &str) -> String {
    let ordinal = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}:{ordinal}")
}

pub fn write_payload(kind: &str, value: &impl Serialize) -> Option<RawPayloadRef> {
    let writer = PAYLOAD_WRITER.get()?;
    let mut writer = writer.lock().unwrap_or_else(PoisonError::into_inner);
    write_payload_locked(&mut writer, kind, value)
}

pub fn write_payload_lazy<T>(kind: &str, build_value: impl FnOnce() -> T) -> Option<RawPayloadRef>
where
    T: Serialize,
{
    let writer = PAYLOAD_WRITER.get()?;
    let value = build_value();
    let mut writer = writer.lock().unwrap_or_else(PoisonError::into_inner);
    write_payload_locked(&mut writer, kind, &value)
}

fn write_payload_locked(
    writer: &mut PayloadWriter,
    kind: &str,
    value: &impl Serialize,
) -> Option<RawPayloadRef> {
    let ordinal = writer.next_payload_ordinal;
    writer.next_payload_ordinal += 1;
    let raw_payload_id = format!("raw_payload:{ordinal}");
    let path = format!("payloads/{ordinal}.json");
    let absolute_path = writer.payloads_dir.join(format!("{ordinal}.json"));
    if write_json_file(&absolute_path, value).is_err() {
        return None;
    }
    Some(RawPayloadRef {
        raw_payload_id,
        kind: kind.to_string(),
        path,
    })
}

fn write_json_file(path: &Path, value: &impl Serialize) -> Result<()> {
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    serde_json::to_writer_pretty(file, value)
        .with_context(|| format!("write JSON {}", path.display()))
}

pub(crate) fn unix_time_ms() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_millis(0));
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;
    use tracing_subscriber::prelude::*;

    #[test]
    fn local_layer_can_be_composed_after_assignment() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let layer = super::local_layer_for_root(temp.path())?;
        let _subscriber = tracing_subscriber::registry().with(layer);
        Ok(())
    }

    #[test]
    fn local_layer_writes_standard_json_events() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let layer = super::local_layer_for_root(temp.path())?;
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::event!(
                target: super::LOCAL_TRACE_TARGET,
                tracing::Level::INFO,
                event.name = %"codex.turn.started",
                thread.id = %"thread-1",
                turn.id = %"turn-1",
            );
        });

        let trace_dir = std::fs::read_dir(temp.path())?
            .next()
            .transpose()?
            .expect("trace directory should be created")
            .path();
        let event_log = std::fs::read_to_string(trace_dir.join("events.jsonl"))?;
        let event: serde_json::Value = serde_json::from_str(event_log.trim())?;
        assert_eq!(event["target"], super::LOCAL_TRACE_TARGET);
        assert_eq!(event["event.name"], "codex.turn.started");
        assert_eq!(event["thread.id"], "thread-1");
        assert_eq!(event["turn.id"], "turn-1");
        assert!(
            event
                .get("timestamp")
                .and_then(serde_json::Value::as_str)
                .is_some()
        );
        Ok(())
    }
}
