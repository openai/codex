use crate::config::OtelExporter;
use crate::config::OtelHttpProtocol;
use crate::metrics::MetricsError;
use crate::metrics::Result;
use crate::metrics::config::MetricsConfig;
use crate::metrics::config::MetricsExporter;
use crate::metrics::timer::Timer;
use crate::metrics::validation::validate_metric_name;
use crate::metrics::validation::validate_tag_key;
use crate::metrics::validation::validate_tag_value;
use crate::metrics::validation::validate_tags;
use codex_utils_string::sanitize_metric_tag_value;
use opentelemetry::KeyValue;
use opentelemetry::metrics::Counter;
use opentelemetry::metrics::Gauge;
use opentelemetry::metrics::Histogram;
use opentelemetry::metrics::Meter;
use opentelemetry::metrics::MeterProvider as _;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_METRICS_TIMEOUT;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_otlp::WithTonicConfig;
use opentelemetry_otlp::tonic_types::metadata::MetadataMap;
use opentelemetry_otlp::tonic_types::transport::ClientTlsConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::metrics::InstrumentKind;
use opentelemetry_sdk::metrics::ManualReader;
use opentelemetry_sdk::metrics::PeriodicReader;
use opentelemetry_sdk::metrics::Pipeline;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::metrics::Temporality;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::metrics::reader::MetricReader;
use opentelemetry_semantic_conventions as semconv;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::time::Duration;
use tracing::debug;

const ENV_ATTRIBUTE: &str = "env";
const METER_NAME: &str = "codex";
const MILLISECOND_DURATION_UNIT: &str = "ms";
const MILLISECOND_DURATION_DESCRIPTION: &str = "Duration in milliseconds.";
const MILLISECOND_DURATION_BOUNDARIES: &[f64] = &[
    0.0, 5.0, 10.0, 25.0, 50.0, 75.0, 100.0, 250.0, 500.0, 750.0, 1000.0, 2500.0, 5000.0, 7500.0,
    10000.0,
];
const SECOND_DURATION_UNIT: &str = "s";
const SECOND_DURATION_BOUNDARIES: &[f64] = &[
    0.0, 0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
];

#[derive(Debug, Eq, Hash, PartialEq)]
struct InstrumentKey {
    name: String,
    description: Option<String>,
}

#[derive(Clone, Debug)]
struct SharedManualReader {
    inner: Arc<ManualReader>,
}

impl SharedManualReader {
    fn new(inner: Arc<ManualReader>) -> Self {
        Self { inner }
    }
}

impl MetricReader for SharedManualReader {
    fn register_pipeline(&self, pipeline: Weak<Pipeline>) {
        self.inner.register_pipeline(pipeline);
    }

    fn collect(&self, rm: &mut ResourceMetrics) -> opentelemetry_sdk::error::OTelSdkResult {
        self.inner.collect(rm)
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.inner.force_flush()
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> opentelemetry_sdk::error::OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }

    fn temporality(&self, kind: InstrumentKind) -> Temporality {
        self.inner.temporality(kind)
    }
}

#[derive(Debug)]
struct CachedDurationHistogram {
    unit: &'static str,
    description: String,
    histogram: Histogram<f64>,
}

#[derive(Debug)]
struct MetricsClientInner {
    meter_provider: SdkMeterProvider,
    meter: Meter,
    counters: Mutex<HashMap<InstrumentKey, Counter<u64>>>,
    gauges: Mutex<HashMap<InstrumentKey, Gauge<i64>>>,
    histograms: Mutex<HashMap<String, Histogram<f64>>>,
    duration_histograms: Mutex<HashMap<String, CachedDurationHistogram>>,
    runtime_reader: Option<Arc<ManualReader>>,
    default_tags: BTreeMap<String, String>,
}

impl MetricsClientInner {
    fn counter(
        &self,
        name: &str,
        description: Option<&str>,
        inc: i64,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        validate_metric_name(name)?;
        if inc < 0 {
            return Err(MetricsError::NegativeCounterIncrement {
                name: name.to_string(),
                inc,
            });
        }
        let attributes = self.attributes(tags)?;

        let mut counters = self
            .counters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let key = InstrumentKey {
            name: name.to_string(),
            description: description.map(str::to_string),
        };
        let counter = counters.entry(key).or_insert_with(|| {
            let builder = self.meter.u64_counter(name.to_string());
            match description {
                Some(description) => builder.with_description(description.to_string()).build(),
                None => builder.build(),
            }
        });
        counter.add(inc as u64, &attributes);
        Ok(())
    }

    fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) -> Result<()> {
        validate_metric_name(name)?;
        let attributes = self.attributes(tags)?;

        let mut histograms = self
            .histograms
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let histogram = histograms
            .entry(name.to_string())
            .or_insert_with(|| self.meter.f64_histogram(name.to_string()).build());
        histogram.record(value as f64, &attributes);
        Ok(())
    }

    fn gauge(
        &self,
        name: &str,
        description: Option<&str>,
        value: i64,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        validate_metric_name(name)?;
        let attributes = self.attributes(tags)?;

        let mut gauges = self
            .gauges
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let key = InstrumentKey {
            name: name.to_string(),
            description: description.map(str::to_string),
        };
        let gauge = gauges.entry(key).or_insert_with(|| {
            let builder = self.meter.i64_gauge(name.to_string());
            match description {
                Some(description) => builder.with_description(description.to_string()).build(),
                None => builder.build(),
            }
        });
        gauge.record(value, &attributes);
        Ok(())
    }

    fn duration_histogram(
        &self,
        name: &str,
        value: f64,
        unit: &'static str,
        description: &str,
        boundaries: &'static [f64],
        tags: &[(&str, &str)],
    ) -> Result<()> {
        validate_metric_name(name)?;
        let attributes = self.attributes(tags)?;

        let mut histograms = self
            .duration_histograms
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let histogram = match histograms.entry(name.to_string()) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                let cached = entry.into_mut();
                if cached.unit != unit || cached.description != description {
                    return Err(MetricsError::ConflictingDurationHistogram {
                        name: name.to_string(),
                        existing_unit: cached.unit.to_string(),
                        existing_description: cached.description.clone(),
                        requested_unit: unit.to_string(),
                        requested_description: description.to_string(),
                    });
                }
                &cached.histogram
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                &entry
                    .insert(CachedDurationHistogram {
                        unit,
                        description: description.to_string(),
                        histogram: self
                            .meter
                            .f64_histogram(name.to_string())
                            .with_unit(unit)
                            .with_description(description.to_string())
                            .with_boundaries(boundaries.to_vec())
                            .build(),
                    })
                    .histogram
            }
        };
        histogram.record(value, &attributes);
        Ok(())
    }

    fn attributes(&self, tags: &[(&str, &str)]) -> Result<Vec<KeyValue>> {
        if tags.is_empty() {
            return Ok(self
                .default_tags
                .iter()
                .map(|(key, value)| KeyValue::new(key.clone(), value.clone()))
                .collect());
        }

        let mut merged = self.default_tags.clone();
        for (key, value) in tags {
            validate_tag_key(key)?;
            validate_tag_value(value)?;
            merged.insert((*key).to_string(), (*value).to_string());
        }

        Ok(merged
            .into_iter()
            .map(|(key, value)| KeyValue::new(key, value))
            .collect())
    }

    fn shutdown(&self) -> Result<()> {
        debug!("flushing OTEL metrics");
        self.meter_provider
            .force_flush()
            .map_err(|source| MetricsError::ProviderShutdown { source })?;
        self.meter_provider
            .shutdown()
            .map_err(|source| MetricsError::ProviderShutdown { source })?;
        Ok(())
    }
}

/// OpenTelemetry metrics client used by Codex.
#[derive(Clone, Debug)]
pub struct MetricsClient(std::sync::Arc<MetricsClientInner>);

impl MetricsClient {
    /// Build a metrics client from configuration and validate defaults.
    pub fn new(config: MetricsConfig) -> Result<Self> {
        let MetricsConfig {
            environment,
            service_name,
            service_version,
            exporter,
            export_interval,
            runtime_reader,
            default_tags,
        } = config;

        validate_tags(&default_tags)?;

        let mut resource_attributes = Vec::with_capacity(4);
        resource_attributes.push(KeyValue::new(
            semconv::attribute::SERVICE_VERSION,
            service_version,
        ));
        resource_attributes.push(KeyValue::new(ENV_ATTRIBUTE, environment));
        resource_attributes.extend(os_resource_attributes());

        let resource = Resource::builder()
            .with_service_name(service_name)
            .with_attributes(resource_attributes)
            .build();

        let runtime_reader = runtime_reader.then(|| {
            Arc::new(
                ManualReader::builder()
                    .with_temporality(Temporality::Delta)
                    .build(),
            )
        });

        let (meter_provider, meter) = match exporter {
            MetricsExporter::InMemory(exporter) => {
                build_provider(resource, exporter, export_interval, runtime_reader.clone())
            }
            MetricsExporter::Otlp(exporter) => {
                let exporter = build_otlp_metric_exporter(exporter, Temporality::Delta)?;
                build_provider(resource, exporter, export_interval, runtime_reader.clone())
            }
        };

        Ok(Self(std::sync::Arc::new(MetricsClientInner {
            meter_provider,
            meter,
            counters: Mutex::new(HashMap::new()),
            gauges: Mutex::new(HashMap::new()),
            histograms: Mutex::new(HashMap::new()),
            duration_histograms: Mutex::new(HashMap::new()),
            runtime_reader,
            default_tags,
        })))
    }

    /// Send a single counter increment.
    pub fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) -> Result<()> {
        self.0.counter(name, /*description*/ None, inc, tags)
    }

    /// Send a single counter increment with an instrument description.
    pub fn counter_with_description(
        &self,
        name: &str,
        description: &str,
        inc: i64,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        self.0.counter(name, Some(description), inc, tags)
    }

    /// Send a single histogram sample.
    pub fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) -> Result<()> {
        self.0.histogram(name, value, tags)
    }

    /// Send a single gauge measurement.
    pub fn gauge(&self, name: &str, value: i64, tags: &[(&str, &str)]) -> Result<()> {
        self.0.gauge(name, /*description*/ None, value, tags)
    }

    /// Send a single gauge measurement with an instrument description.
    pub fn gauge_with_description(
        &self,
        name: &str,
        description: &str,
        value: i64,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        self.0.gauge(name, Some(description), value, tags)
    }

    /// Record a duration in milliseconds using a histogram.
    pub fn record_duration(
        &self,
        name: &str,
        duration: Duration,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        self.0.duration_histogram(
            name,
            duration.as_millis().min(i64::MAX as u128) as f64,
            MILLISECOND_DURATION_UNIT,
            MILLISECOND_DURATION_DESCRIPTION,
            MILLISECOND_DURATION_BOUNDARIES,
            tags,
        )
    }

    /// Record a duration in seconds using a histogram with an instrument description.
    pub fn record_duration_seconds_with_description(
        &self,
        name: &str,
        description: &str,
        duration: Duration,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        self.0.duration_histogram(
            name,
            duration.as_secs_f64(),
            SECOND_DURATION_UNIT,
            description,
            SECOND_DURATION_BOUNDARIES,
            tags,
        )
    }

    pub fn start_timer(
        &self,
        name: &str,
        tags: &[(&str, &str)],
    ) -> std::result::Result<Timer, MetricsError> {
        Ok(Timer::new(name, tags, self))
    }

    /// Collect a runtime metrics snapshot without shutting down the provider.
    pub fn snapshot(&self) -> Result<ResourceMetrics> {
        let Some(reader) = &self.0.runtime_reader else {
            return Err(MetricsError::RuntimeSnapshotUnavailable);
        };
        let mut snapshot = ResourceMetrics::default();
        reader
            .collect(&mut snapshot)
            .map_err(|source| MetricsError::RuntimeSnapshotCollect { source })?;
        Ok(snapshot)
    }

    /// Flush metrics and stop the underlying OTEL meter provider.
    pub fn shutdown(&self) -> Result<()> {
        self.0.shutdown()
    }
}

fn os_resource_attributes() -> Vec<KeyValue> {
    let os_info = os_info::get();
    let os_type_raw = os_info.os_type().to_string();
    let os_type = sanitize_metric_tag_value(os_type_raw.as_str());
    let os_version_raw = os_info.version().to_string();
    let os_version = sanitize_metric_tag_value(os_version_raw.as_str());
    let mut attributes = Vec::new();
    if os_type != "unspecified" {
        attributes.push(KeyValue::new("os", os_type));
    }
    if os_version != "unspecified" {
        attributes.push(KeyValue::new("os_version", os_version));
    }
    attributes
}

fn build_provider<E>(
    resource: Resource,
    exporter: E,
    interval: Option<Duration>,
    runtime_reader: Option<Arc<ManualReader>>,
) -> (SdkMeterProvider, Meter)
where
    E: opentelemetry_sdk::metrics::exporter::PushMetricExporter + 'static,
{
    let mut reader_builder = PeriodicReader::builder(exporter);
    if let Some(interval) = interval {
        reader_builder = reader_builder.with_interval(interval);
    }
    let reader = reader_builder.build();
    let mut provider_builder = SdkMeterProvider::builder().with_resource(resource);
    if let Some(reader) = runtime_reader {
        provider_builder = provider_builder.with_reader(SharedManualReader::new(reader));
    }
    let provider = provider_builder.with_reader(reader).build();
    let meter = provider.meter(METER_NAME);
    (provider, meter)
}

fn build_otlp_metric_exporter(
    exporter: OtelExporter,
    temporality: Temporality,
) -> Result<opentelemetry_otlp::MetricExporter> {
    match exporter {
        OtelExporter::None => Err(MetricsError::ExporterDisabled),
        OtelExporter::Statsig => build_otlp_metric_exporter(
            crate::config::resolve_exporter(&OtelExporter::Statsig),
            temporality,
        ),
        OtelExporter::OtlpGrpc {
            endpoint,
            headers,
            tls,
        } => {
            debug!("Using OTLP Grpc exporter for metrics: {endpoint}");

            let header_map = crate::otlp::build_header_map(&headers);

            let base_tls_config = ClientTlsConfig::new()
                .with_enabled_roots()
                .assume_http2(true);

            let tls_config = match tls.as_ref() {
                Some(tls) => crate::otlp::build_grpc_tls_config(&endpoint, base_tls_config, tls)
                    .map_err(|err| MetricsError::InvalidConfig {
                        message: err.to_string(),
                    })?,
                None => base_tls_config,
            };

            opentelemetry_otlp::MetricExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .with_temporality(temporality)
                .with_metadata(MetadataMap::from_headers(header_map))
                .with_tls_config(tls_config)
                .build()
                .map_err(|source| MetricsError::ExporterBuild { source })
        }
        OtelExporter::OtlpHttp {
            endpoint,
            headers,
            protocol,
            tls,
        } => {
            debug!("Using OTLP Http exporter for metrics: {endpoint}");

            let protocol = match protocol {
                OtelHttpProtocol::Binary => Protocol::HttpBinary,
                OtelHttpProtocol::Json => Protocol::HttpJson,
            };

            let mut exporter_builder = opentelemetry_otlp::MetricExporter::builder()
                .with_http()
                .with_endpoint(endpoint)
                .with_temporality(temporality)
                .with_protocol(protocol)
                .with_headers(headers);

            if let Some(tls) = tls.as_ref() {
                let client =
                    crate::otlp::build_http_client(tls, OTEL_EXPORTER_OTLP_METRICS_TIMEOUT)
                        .map_err(|err| MetricsError::InvalidConfig {
                            message: err.to_string(),
                        })?;
                exporter_builder = exporter_builder.with_http_client(client);
            }

            exporter_builder
                .build()
                .map_err(|source| MetricsError::ExporterBuild { source })
        }
    }
}
