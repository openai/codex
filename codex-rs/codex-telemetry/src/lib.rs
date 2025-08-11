//! Telemetry initialization and OTLP File Exporter for Codex.

#[cfg(feature = "otel")]
mod imp {
    use std::fs::File;
    use std::fs::OpenOptions;
    use std::io::BufWriter;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::SystemTime;

    use opentelemetry::global;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry::KeyValue;
    use opentelemetry::Value;
    use opentelemetry_otlp as otlp;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::export::trace::ExportResult;
    use opentelemetry_sdk::export::trace::SpanData;
    use opentelemetry_sdk::export::trace::SpanExporter;
    use opentelemetry_sdk::resource::Resource;
    use opentelemetry_sdk::trace::TracerProvider;
    use opentelemetry_sdk::trace::{self as sdktrace};
    use serde::Serialize;
    use tracing_opentelemetry::OpenTelemetryLayer;
    use tracing_subscriber::Registry;

    use std::collections::HashMap;

    #[derive(Clone, Debug)]
    pub enum TelemetryExporter {
        None,
        OtlpFile {
            path: PathBuf,
            rotate_mb: Option<u64>,
        },
        OtlpGrpc {
            endpoint: String,
            headers: Vec<(String, String)>,
        },
        OtlpHttp {
            endpoint: String,
            headers: Vec<(String, String)>,
        },
    }

    #[derive(Clone, Debug)]
    pub struct TelemetrySettings {
        pub enabled: bool,
        pub exporter: TelemetryExporter,
        pub service_name: String,
        pub service_version: String,
        pub codex_home: Option<PathBuf>,
    }

    pub struct TelemetryGuard {
        provider: TracerProvider,
    }

    impl Drop for TelemetryGuard {
        fn drop(&mut self) {
            let _ = self.provider.shutdown();
        }
    }

    fn debug_enabled() -> bool {
        std::env::var("CODEX_TELEMETRY_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    pub fn init_from_settings(settings: &TelemetrySettings) -> Option<TelemetryGuard> {
        if !settings.enabled {
            return None;
        }

        let resource = Resource::default().merge(&Resource::new(vec![
            KeyValue::new("service.name", settings.service_name.clone()),
            KeyValue::new("service.version", settings.service_version.clone()),
        ]));

        let trace_config = sdktrace::Config::default().with_resource(resource.clone());
        let mut provider_builder = sdktrace::TracerProvider::builder().with_config(trace_config);

        // Build base subscriber; add a console fmt layer when CODEX_TELEMETRY_DEBUG is enabled
        use tracing_subscriber::prelude::*;
        let subscriber = Registry::default().with(if debug_enabled() {
            eprintln!("[codex-telemetry] debug enabled");
            Some(
                tracing_subscriber::fmt::layer()
                    .with_ansi(true)
                    .with_target(true)
                    .with_level(true),
            )
        } else {
            None
        });

        match &settings.exporter {
            TelemetryExporter::None => {
                if debug_enabled() {
                    eprintln!("[codex-telemetry] exporter=None");
                }
                let provider = provider_builder.build();
                let tracer = provider.tracer(settings.service_name.clone());
                let otel_layer = OpenTelemetryLayer::new(tracer);
                let subscriber = subscriber.with(otel_layer);
                if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
                    eprintln!("[codex-telemetry] ERROR: failed to set global subscriber: {e}");
                }
                global::set_tracer_provider(provider.clone());
                return Some(TelemetryGuard { provider });
            }
            TelemetryExporter::OtlpFile {
                path: _ignored,
                rotate_mb,
            } => {
                let final_path = determine_traces_file_path(settings.codex_home.as_ref());
                if debug_enabled() {
                    eprintln!(
                        "[codex-telemetry] using OTLP JSON file exporter: {} (rotate_mb={:?})",
                        final_path.display(),
                        rotate_mb
                    );
                }
                let mut resource_attributes: Vec<JsonKeyValue> = Vec::new();
                for (k, v) in resource.iter() {
                    resource_attributes.push(JsonKeyValue {
                        key: k.as_str().to_string(),
                        value: json_any_from(v.clone()),
                    });
                }
                let exporter =
                    OtlpJsonFileExporter::new(final_path, *rotate_mb, resource_attributes)
                        .expect("create OTLP JSON file exporter");
                let batch = sdktrace::BatchSpanProcessor::builder(
                    exporter,
                    opentelemetry_sdk::runtime::Tokio,
                )
                .build();
                let trace_config2 = sdktrace::Config::default().with_resource(resource);
                provider_builder = sdktrace::TracerProvider::builder()
                    .with_span_processor(batch)
                    .with_config(trace_config2);
            }
            TelemetryExporter::OtlpGrpc { endpoint, headers } => {
                if debug_enabled() {
                    eprintln!(
                        "[codex-telemetry] using OTLP gRPC exporter: endpoint={} headers={} pairs",
                        endpoint,
                        headers.len()
                    );
                }
                let mut exp = otlp::new_exporter().tonic().with_endpoint(endpoint.clone());
                if !headers.is_empty() {
                    let mut map = tonic::metadata::MetadataMap::new();
                    for (k, v) in headers {
                        let key = k.parse::<tonic::metadata::MetadataKey<tonic::metadata::Ascii>>();
                        let val =
                            v.parse::<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>();
                        if let (Ok(key), Ok(val)) = (key, val) {
                            let _ = map.insert(key, val);
                        }
                    }
                    exp = exp.with_metadata(map);
                }
                let exporter = exp
                    .build_span_exporter()
                    .expect("install OTLP gRPC exporter");
                let batch = sdktrace::BatchSpanProcessor::builder(
                    exporter,
                    opentelemetry_sdk::runtime::Tokio,
                )
                .build();
                provider_builder = sdktrace::TracerProvider::builder()
                    .with_span_processor(batch)
                    .with_config(sdktrace::Config::default().with_resource(resource));
            }
            TelemetryExporter::OtlpHttp { endpoint, headers } => {
                if debug_enabled() {
                    eprintln!(
                        "[codex-telemetry] using OTLP HTTP exporter: endpoint={} headers={} pairs",
                        endpoint,
                        headers.len()
                    );
                }
                let mut exp = otlp::new_exporter().http().with_endpoint(endpoint.clone());
                if !headers.is_empty() {
                    let mut map: HashMap<String, String> = HashMap::new();
                    for (k, v) in headers {
                        map.insert(k.clone(), v.clone());
                    }
                    exp = exp.with_headers(map);
                }
                let exporter = exp
                    .build_span_exporter()
                    .expect("install OTLP HTTP exporter");
                let batch = sdktrace::BatchSpanProcessor::builder(
                    exporter,
                    opentelemetry_sdk::runtime::Tokio,
                )
                .build();
                provider_builder = sdktrace::TracerProvider::builder()
                    .with_span_processor(batch)
                    .with_config(sdktrace::Config::default().with_resource(resource));
            }
        }

        let provider = provider_builder.build();
        let tracer = provider.tracer(settings.service_name.clone());
        let otel_layer = OpenTelemetryLayer::new(tracer);
        let subscriber = subscriber.with(otel_layer);
        if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
            eprintln!("[codex-telemetry] ERROR: failed to set global subscriber: {e}");
        }
        global::set_tracer_provider(provider.clone());
        Some(TelemetryGuard { provider })
    }

    /// Build an OpenTelemetry Layer without installing a global subscriber.
    /// Caller should attach the returned Layer to their existing subscriber and
    /// hold onto the Guard for the process lifetime to ensure clean shutdown.
    pub fn build_layer(
        settings: &TelemetrySettings,
    ) -> Option<(TelemetryGuard, opentelemetry_sdk::trace::Tracer)> {
        if !settings.enabled {
            return None;
        }

        let resource = Resource::default().merge(&Resource::new(vec![
            KeyValue::new("service.name", settings.service_name.clone()),
            KeyValue::new("service.version", settings.service_version.clone()),
        ]));

        let trace_config = sdktrace::Config::default().with_resource(resource.clone());
        let mut provider_builder = sdktrace::TracerProvider::builder().with_config(trace_config);

        match &settings.exporter {
            TelemetryExporter::None => {
                if debug_enabled() {
                    eprintln!("[codex-telemetry] build_layer: exporter=None");
                }
                let provider = provider_builder.build();
                let tracer = provider.tracer(settings.service_name.clone());
                return Some((TelemetryGuard { provider }, tracer));
            }
            TelemetryExporter::OtlpFile {
                path: _ignored,
                rotate_mb,
            } => {
                let final_path = determine_traces_file_path(settings.codex_home.as_ref());
                if debug_enabled() {
                    eprintln!(
                        "[codex-telemetry] build_layer: file exporter at {} (rotate_mb={:?})",
                        final_path.display(),
                        rotate_mb
                    );
                }
                let mut resource_attributes: Vec<JsonKeyValue> = Vec::new();
                for (k, v) in resource.iter() {
                    resource_attributes.push(JsonKeyValue {
                        key: k.as_str().to_string(),
                        value: json_any_from(v.clone()),
                    });
                }
                let exporter =
                    OtlpJsonFileExporter::new(final_path, *rotate_mb, resource_attributes)
                        .expect("create OTLP JSON file exporter");
                let batch = sdktrace::BatchSpanProcessor::builder(
                    exporter,
                    opentelemetry_sdk::runtime::Tokio,
                )
                .build();
                provider_builder = sdktrace::TracerProvider::builder()
                    .with_span_processor(batch)
                    .with_config(sdktrace::Config::default().with_resource(resource));
            }
            TelemetryExporter::OtlpGrpc { endpoint, headers } => {
                if debug_enabled() {
                    eprintln!(
                        "[codex-telemetry] build_layer: grpc exporter endpoint={} headers={} pairs",
                        endpoint,
                        headers.len()
                    );
                }
                let mut exp = otlp::new_exporter().tonic().with_endpoint(endpoint.clone());
                if !headers.is_empty() {
                    let mut map = tonic::metadata::MetadataMap::new();
                    for (k, v) in headers {
                        let key = k.parse::<tonic::metadata::MetadataKey<tonic::metadata::Ascii>>();
                        let val =
                            v.parse::<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>();
                        if let (Ok(key), Ok(val)) = (key, val) {
                            let _ = map.insert(key, val);
                        }
                    }
                    exp = exp.with_metadata(map);
                }
                let exporter = exp
                    .build_span_exporter()
                    .expect("install OTLP gRPC exporter");
                let batch = sdktrace::BatchSpanProcessor::builder(
                    exporter,
                    opentelemetry_sdk::runtime::Tokio,
                )
                .build();
                provider_builder = sdktrace::TracerProvider::builder()
                    .with_span_processor(batch)
                    .with_config(sdktrace::Config::default().with_resource(resource));
            }
            TelemetryExporter::OtlpHttp { endpoint, headers } => {
                if debug_enabled() {
                    eprintln!(
                        "[codex-telemetry] build_layer: http exporter endpoint={} headers={} pairs",
                        endpoint,
                        headers.len()
                    );
                }
                let mut exp = otlp::new_exporter().http().with_endpoint(endpoint.clone());
                if !headers.is_empty() {
                    let mut map: HashMap<String, String> = HashMap::new();
                    for (k, v) in headers {
                        map.insert(k.clone(), v.clone());
                    }
                    exp = exp.with_headers(map);
                }
                let exporter = exp
                    .build_span_exporter()
                    .expect("install OTLP HTTP exporter");
                let batch = sdktrace::BatchSpanProcessor::builder(
                    exporter,
                    opentelemetry_sdk::runtime::Tokio,
                )
                .build();
                provider_builder = sdktrace::TracerProvider::builder()
                    .with_span_processor(batch)
                    .with_config(sdktrace::Config::default().with_resource(resource));
            }
        }

        let provider = provider_builder.build();
        let tracer = provider.tracer(settings.service_name.clone());
        Some((TelemetryGuard { provider }, tracer))
    }

    /// Create a span representing a Codex session; store and drop it to delimit the session.
    pub fn make_session_span(session_id: &str, model: &str, provider: &str) -> tracing::Span {
        tracing::info_span!(
            "codex.session",
            session.id = %session_id,
            model = %model,
            provider = %provider
        )
    }

    #[derive(Debug)]
    struct OtlpJsonFileExporter {
        writer: Mutex<BufWriter<File>>,
        path: PathBuf,
        rotate_bytes: Option<u64>,
        resource_attributes: Vec<JsonKeyValue>,
    }

    impl OtlpJsonFileExporter {
        fn new(
            path: PathBuf,
            rotate_mb: Option<u64>,
            resource_attributes: Vec<JsonKeyValue>,
        ) -> std::io::Result<Self> {
            if debug_enabled() {
                eprintln!("[codex-telemetry] opening trace file: {}", path.display());
            }
            let file = OpenOptions::new().create(true).append(true).open(&path)?;
            Ok(Self {
                writer: Mutex::new(BufWriter::new(file)),
                path,
                rotate_bytes: rotate_mb.map(|mb| mb.saturating_mul(1024 * 1024)),
                resource_attributes,
            })
        }

        fn maybe_rotate(&self) -> std::io::Result<()> {
            let Some(limit) = self.rotate_bytes else {
                return Ok(());
            };
            let meta = std::fs::metadata(&self.path)?;
            if meta.len() as u64 >= limit {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let rotated = self.path.with_extension(format!("json.{secs}"));
                {
                    let mut w = self.writer.lock().unwrap();
                    w.flush()?;
                }
                std::fs::rename(&self.path, rotated)?;
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?;
                let mut guard = self.writer.lock().unwrap();
                *guard = BufWriter::new(file);
            }
            Ok(())
        }

        fn write_traces_line(&self, batch: Vec<SpanData>) -> std::io::Result<()> {
            if debug_enabled() {
                eprintln!(
                    "[codex-telemetry] exporting {} span(s) to {}",
                    batch.len(),
                    self.path.display()
                );
            }
            let spans_json: Vec<JsonSpan> = batch.into_iter().map(span_to_json).collect();
            let traces = TracesData {
                resource_spans: vec![JsonResourceSpans {
                    resource: Some(JsonResource {
                        attributes: self.resource_attributes.clone(),
                    }),
                    scope_spans: vec![JsonScopeSpans {
                        scope: JsonScope {},
                        spans: spans_json,
                        schema_url: String::new(),
                    }],
                    schema_url: String::new(),
                }],
            };
            let mut w = self.writer.lock().unwrap();
            let line = serde_json::to_string(&traces).unwrap_or_else(|_| String::from("{}"));
            w.write_all(line.as_bytes())?;
            w.write_all(b"\n")?;
            w.flush()
        }
    }

    impl SpanExporter for OtlpJsonFileExporter {
        fn export(
            &mut self,
            batch: Vec<SpanData>,
        ) -> futures_util::future::BoxFuture<'static, ExportResult> {
            if batch.is_empty() {
                return Box::pin(async { Ok(()) });
            }
            if let Err(e) = self.maybe_rotate() {
                tracing::warn!("otlp-json-file: rotate failed: {}", e);
            }
            let res = self.write_traces_line(batch);
            if let Err(e) = res {
                tracing::warn!("otlp-json-file: write failed: {}", e);
            }
            Box::pin(async { Ok(()) })
        }

        fn shutdown(&mut self) {
            if let Ok(mut w) = self.writer.lock() {
                let _ = w.flush();
            }
        }
    }

    // ===== OTLP JSON model =====
    #[derive(Serialize, Clone, Debug)]
    struct TracesData {
        #[serde(rename = "resourceSpans")]
        resource_spans: Vec<JsonResourceSpans>,
    }

    #[derive(Serialize, Clone, Debug)]
    struct JsonResourceSpans {
        #[serde(skip_serializing_if = "Option::is_none")]
        resource: Option<JsonResource>,
        #[serde(rename = "scopeSpans")]
        scope_spans: Vec<JsonScopeSpans>,
        #[serde(default, rename = "schemaUrl")]
        schema_url: String,
    }

    #[derive(Serialize, Clone, Debug)]
    struct JsonResource {
        attributes: Vec<JsonKeyValue>,
    }

    #[derive(Serialize, Clone, Debug)]
    struct JsonScopeSpans {
        scope: JsonScope,
        spans: Vec<JsonSpan>,
        #[serde(default, rename = "schemaUrl")]
        schema_url: String,
    }

    #[derive(Serialize, Clone, Debug, Default)]
    struct JsonScope {}

    #[derive(Serialize, Clone, Debug)]
    struct JsonSpan {
        #[serde(rename = "traceId")]
        trace_id: String,
        #[serde(rename = "spanId")]
        span_id: String,
        #[serde(default, rename = "parentSpanId")]
        parent_span_id: String,
        name: String,
        kind: i32,
        #[serde(rename = "startTimeUnixNano")]
        start_time_unix_nano: String,
        #[serde(rename = "endTimeUnixNano")]
        end_time_unix_nano: String,
        #[serde(default)]
        attributes: Vec<JsonKeyValue>,
        #[serde(default, rename = "droppedAttributesCount")]
        dropped_attributes_count: i32,
        #[serde(default)]
        events: Vec<JsonEvent>,
        #[serde(default, rename = "droppedEventsCount")]
        dropped_events_count: i32,
        #[serde(default)]
        links: Vec<JsonLink>,
        #[serde(default, rename = "droppedLinksCount")]
        dropped_links_count: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<JsonStatus>,
    }

    #[derive(Serialize, Clone, Debug)]
    struct JsonKeyValue {
        key: String,
        value: JsonAnyValue,
    }

    #[derive(Serialize, Clone, Debug)]
    #[serde(untagged)]
    enum JsonAnyValue {
        String {
            #[serde(rename = "stringValue")]
            string_value: String,
        },
        Bool {
            #[serde(rename = "boolValue")]
            bool_value: bool,
        },
        Int {
            #[serde(rename = "intValue")]
            int_value: i64,
        },
        Double {
            #[serde(rename = "doubleValue")]
            double_value: f64,
        },
    }

    #[derive(Serialize, Clone, Debug)]
    struct JsonEvent {
        #[serde(rename = "timeUnixNano")]
        time_unix_nano: String,
        name: String,
        #[serde(default)]
        attributes: Vec<JsonKeyValue>,
        #[serde(default, rename = "droppedAttributesCount")]
        dropped_attributes_count: i32,
    }

    #[derive(Serialize, Clone, Debug)]
    struct JsonLink {
        #[serde(rename = "traceId")]
        trace_id: String,
        #[serde(rename = "spanId")]
        span_id: String,
        #[serde(default, rename = "traceState")]
        trace_state: String,
        #[serde(default)]
        attributes: Vec<JsonKeyValue>,
        #[serde(default, rename = "droppedAttributesCount")]
        dropped_attributes_count: i32,
        #[serde(default)]
        flags: i32,
    }

    #[derive(Serialize, Clone, Debug)]
    struct JsonStatus {
        #[serde(default)]
        message: String,
        code: i32, // 0=UNSET,1=OK,2=ERROR
    }

    // ===== mapping helpers =====
    fn json_kv_from(kv: &KeyValue) -> JsonKeyValue {
        JsonKeyValue {
            key: kv.key.as_str().to_string(),
            value: json_any_from(kv.value.clone()),
        }
    }

    fn json_any_from(val: Value) -> JsonAnyValue {
        match val {
            Value::String(s) => JsonAnyValue::String {
                string_value: s.to_string(),
            },
            Value::Bool(b) => JsonAnyValue::Bool { bool_value: b },
            Value::I64(i) => JsonAnyValue::Int { int_value: i },
            Value::F64(f) => JsonAnyValue::Double { double_value: f },
            other => JsonAnyValue::String {
                string_value: format!("{other:?}"),
            },
        }
    }

    fn status_to_json(status: &opentelemetry::trace::Status) -> JsonStatus {
        use opentelemetry::trace::Status::Error;
        use opentelemetry::trace::Status::Ok;
        use opentelemetry::trace::Status::Unset;
        match status {
            Unset => JsonStatus {
                code: 0,
                message: String::new(),
            },
            Ok => JsonStatus {
                code: 1,
                message: String::new(),
            },
            Error { description } => JsonStatus {
                code: 2,
                message: description.to_string(),
            },
        }
    }

    fn span_to_json(sd: SpanData) -> JsonSpan {
        let trace_id = hex::encode(sd.span_context.trace_id().to_bytes());
        let span_id = hex::encode(sd.span_context.span_id().to_bytes());
        let parent_span_id = hex::encode(sd.parent_span_id.to_bytes());

        let start = to_unix_nanos(sd.start_time).to_string();
        let end = to_unix_nanos(sd.end_time).to_string();

        let attributes = sd.attributes.iter().map(json_kv_from).collect::<Vec<_>>();

        let events = sd
            .events
            .into_iter()
            .map(|ev| JsonEvent {
                time_unix_nano: to_unix_nanos(ev.timestamp).to_string(),
                name: ev.name.to_string(),
                attributes: ev
                    .attributes
                    .into_iter()
                    .map(|kv| json_kv_from(&kv))
                    .collect(),
                dropped_attributes_count: 0,
            })
            .collect();

        let links = sd
            .links
            .into_iter()
            .map(|lnk| JsonLink {
                trace_id: hex::encode(lnk.span_context.trace_id().to_bytes()),
                span_id: hex::encode(lnk.span_context.span_id().to_bytes()),
                trace_state: lnk.span_context.trace_state().header(),
                attributes: lnk
                    .attributes
                    .into_iter()
                    .map(|kv| json_kv_from(&kv))
                    .collect(),
                dropped_attributes_count: 0,
                flags: 0,
            })
            .collect();

        let kind = match sd.span_kind {
            opentelemetry::trace::SpanKind::Internal => 1,
            opentelemetry::trace::SpanKind::Server => 2,
            opentelemetry::trace::SpanKind::Client => 3,
            opentelemetry::trace::SpanKind::Producer => 4,
            opentelemetry::trace::SpanKind::Consumer => 5,
        };

        JsonSpan {
            trace_id,
            span_id,
            parent_span_id,
            name: sd.name.into_owned(),
            kind,
            start_time_unix_nano: start,
            end_time_unix_nano: end,
            attributes,
            dropped_attributes_count: 0,
            events,
            dropped_events_count: 0,
            links,
            dropped_links_count: 0,
            status: Some(status_to_json(&sd.status)),
        }
    }

    fn to_unix_nanos(t: SystemTime) -> u128 {
        t.duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    }

    fn determine_traces_file_path(codex_home: Option<&PathBuf>) -> PathBuf {
        use chrono::Utc;
        use rand::RngCore;
        use std::fs;

        let base = if let Some(h) = codex_home {
            h.clone()
        } else {
            if debug_enabled() {
                eprintln!("[codex-telemetry] WARNING: codex_home not provided; defaulting to current directory");
            }
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        let traces_dir = base.join("traces");
        match fs::create_dir_all(&traces_dir) {
            Ok(()) => {
                if debug_enabled() {
                    eprintln!(
                        "[codex-telemetry] ensured traces dir exists: {}",
                        traces_dir.display()
                    );
                }
            }
            Err(err) => {
                eprintln!(
                    "[codex-telemetry] ERROR: failed to create traces dir {}: {}",
                    traces_dir.display(),
                    err
                );
            }
        }

        let ts = Utc::now().format("%Y%m%d_%H%M%S");
        let mut bytes = [0u8; 8];
        rand::thread_rng().fill_bytes(&mut bytes);
        let hex_id = hex::encode(bytes);
        traces_dir.join(format!("codex_traces_{ts}_{hex_id}.jsonl"))
    }

    // Re-exports for consumers
    pub use TelemetryExporter as Exporter;
    pub use TelemetryGuard as Guard;
    pub use TelemetrySettings as Settings;
}

#[cfg(not(feature = "otel"))]
mod imp {
    #[derive(Clone, Debug)]
    pub enum TelemetryExporter {
        None,
    }

    #[derive(Clone, Debug)]
    pub struct TelemetrySettings {
        pub enabled: bool,
        pub exporter: TelemetryExporter,
        pub service_name: String,
        pub service_version: String,
    }

    pub struct TelemetryGuard;

    pub fn init_from_settings(_settings: &TelemetrySettings) -> Option<TelemetryGuard> {
        None
    }

    pub fn make_session_span(_session_id: &str, _model: &str, _provider: &str) -> tracing::Span {
        tracing::Span::none()
    }

    pub use TelemetryExporter as Exporter;
    pub use TelemetryGuard as Guard;
    pub use TelemetrySettings as Settings;
}

pub use imp::*;
