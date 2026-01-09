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
use opentelemetry::KeyValue;
use opentelemetry::metrics::Counter;
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
use opentelemetry_sdk::metrics::PeriodicReader;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::metrics::Temporality;
use opentelemetry_semantic_conventions as semconv;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tracing::debug;

const ENV_ATTRIBUTE: &str = "env";
const METER_NAME: &str = "codex";

#[derive(Debug)]
struct MetricsClientInner {
    meter_provider: SdkMeterProvider,
    meter: Meter,
    counters: Mutex<HashMap<String, Counter<u64>>>,
    histograms: Mutex<HashMap<String, Histogram<f64>>>,
    default_tags: BTreeMap<String, String>,
}

impl MetricsClientInner {
    fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) -> Result<()> {
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
        let counter = counters
            .entry(name.to_string())
            .or_insert_with(|| self.meter.u64_counter(name.to_string()).build());
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
        validate_tags(&config.default_tags)?;

        let resource = Resource::builder()
            .with_service_name(config.service_name.clone())
            .with_attributes(vec![
                KeyValue::new(
                    semconv::attribute::SERVICE_VERSION,
                    config.service_version.clone(),
                ),
                KeyValue::new(ENV_ATTRIBUTE, config.environment.clone()),
            ])
            .build();

        let (meter_provider, meter) = match config.exporter {
            MetricsExporter::InMemory(exporter) => {
                build_provider(resource, exporter, config.export_interval)
            }
            MetricsExporter::Otlp(exporter) => {
                let exporter = build_otlp_metric_exporter(exporter, Temporality::Delta)?;
                build_provider(resource, exporter, config.export_interval)
            }
        };

        Ok(Self(std::sync::Arc::new(MetricsClientInner {
            meter_provider,
            meter,
            counters: Mutex::new(HashMap::new()),
            histograms: Mutex::new(HashMap::new()),
            default_tags: config.default_tags,
        })))
    }

    /// Send a single counter increment.
    pub fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) -> Result<()> {
        self.0.counter(name, inc, tags)
    }

    /// Send a single histogram sample.
    pub fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) -> Result<()> {
        self.0.histogram(name, value, tags)
    }

    /// Record a duration in milliseconds using a histogram.
    pub fn record_duration(
        &self,
        name: &str,
        duration: Duration,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        self.histogram(
            name,
            duration.as_millis().min(i64::MAX as u128) as i64,
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

    /// Flush metrics and stop the underlying OTEL meter provider.
    pub fn shutdown(&self) -> Result<()> {
        self.0.shutdown()
    }
}

fn build_provider<E>(
    resource: Resource,
    exporter: E,
    interval: Option<Duration>,
) -> (SdkMeterProvider, Meter)
where
    E: opentelemetry_sdk::metrics::exporter::PushMetricExporter + 'static,
{
    let mut reader_builder = PeriodicReader::builder(exporter);
    if let Some(interval) = interval {
        reader_builder = reader_builder.with_interval(interval);
    }
    let reader = reader_builder.build();
    let provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(reader)
        .build();
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
