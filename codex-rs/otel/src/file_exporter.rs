use crate::config::OtelSettings;
use opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema;
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkError;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::SpanData;
use opentelemetry_sdk::trace::SpanExporter;
use std::fmt::Debug;
use std::fs;
use std::fs::File;
use std::io::Error as IoError;
use std::io::LineWriter;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use uuid::Uuid;

const TRACES_SUBDIR: &str = "traces";

#[derive(Debug)]
pub(crate) struct FileExporter<W: Write + Send + Debug> {
    writer: Arc<Mutex<LineWriter<W>>>,
    resource: Resource,
}

impl<W: Write + Send + Debug> FileExporter<W> {
    pub(crate) fn new(writer: W, resource: Resource) -> Self {
        Self {
            writer: Arc::new(Mutex::new(LineWriter::new(writer))),
            resource,
        }
    }
}

impl<W: Write + Send + Debug> SpanExporter for FileExporter<W> {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
        let resource = ResourceAttributesWithSchema::from(&self.resource);
        let resource_spans = group_spans_by_resource_and_scope(batch, &resource);
        let req = ExportTraceServiceRequest { resource_spans };

        let mut writer = match self.writer.lock() {
            Ok(f) => f,
            Err(e) => {
                return Err(OTelSdkError::InternalFailure(e.to_string()));
            }
        };

        serde_json::to_writer(writer.get_mut(), &req)
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))
            .and(writeln!(writer).map_err(|e| OTelSdkError::InternalFailure(e.to_string())))
    }

    fn shutdown_with_timeout(&mut self, _timeout: Duration) -> OTelSdkResult {
        self.force_flush()
    }

    fn force_flush(&mut self) -> OTelSdkResult {
        let mut writer = self
            .writer
            .lock()
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;

        writer
            .flush()
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))
    }

    fn set_resource(&mut self, resource: &Resource) {
        self.resource = resource.clone();
    }
}

pub(crate) fn create_log_file(settings: &OtelSettings) -> std::io::Result<(File, PathBuf)> {
    let run_id = Uuid::new_v4();

    // Resolve ~/.codex/traces/YYYY/MM/DD and create it if missing.
    let timestamp = OffsetDateTime::now_local()
        .map_err(|e| IoError::other(format!("failed to get local time: {e}")))?;

    let mut dir = settings.codex_home.clone();
    dir.push(TRACES_SUBDIR);
    dir.push(timestamp.year().to_string());
    dir.push(format!("{:02}", u8::from(timestamp.month())));
    dir.push(format!("{:02}", timestamp.day()));
    fs::create_dir_all(&dir)?;

    // Custom format for YYYY-MM-DDThh-mm-ss. Use `-` instead of `:` for
    // compatibility with filesystems that do not allow colons in filenames.
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let date_str = timestamp
        .format(format)
        .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

    let filename = format!("trace-{date_str}-{run_id}.jsonl");

    let path = dir.join(filename);
    let file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)?;

    Ok((file, path))
}
