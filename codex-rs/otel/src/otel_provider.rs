use crate::config::OtelExporter;
use crate::config::OtelHttpProtocol;
use crate::config::OtelSettings;
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry_otlp::LogExporter;
use opentelemetry_otlp::MetricExporter;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_otlp::WithTonicConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_semantic_conventions as semconv;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use std::error::Error;
use std::time::Duration;
use tonic::metadata::MetadataMap;
use tracing::debug;

const ENV_ATTRIBUTE: &str = "env";
const DEFAULT_METRIC_EXPORT_INTERVAL: Duration = Duration::from_secs(30);

pub struct OtelProvider {
    pub logger: SdkLoggerProvider,
    meter: Option<SdkMeterProvider>,
}

impl OtelProvider {
    pub fn shutdown(&self) {
        let _ = self.logger.shutdown();
        if let Some(meter) = &self.meter {
            let _ = meter.shutdown();
        }
    }

    pub fn from(settings: &OtelSettings) -> Result<Option<Self>, Box<dyn Error>> {
        let resource = Resource::builder()
            .with_service_name(settings.service_name.clone())
            .with_attributes(vec![
                KeyValue::new(
                    semconv::attribute::SERVICE_VERSION,
                    settings.service_version.clone(),
                ),
                KeyValue::new(ENV_ATTRIBUTE, settings.environment.clone()),
            ])
            .build();

        let mut builder = SdkLoggerProvider::builder().with_resource(resource.clone());

        let meter_provider = match &settings.exporter {
            OtelExporter::None => {
                debug!("No exporter enabled in OTLP settings.");
                return Ok(None);
            }
            OtelExporter::OtlpGrpc { endpoint, headers } => {
                debug!("Using OTLP Grpc exporter: {}", endpoint);

                let mut header_map = HeaderMap::new();
                for (key, value) in headers {
                    if let Ok(name) = HeaderName::from_bytes(key.as_bytes())
                        && let Ok(val) = HeaderValue::from_str(value)
                    {
                        header_map.insert(name, val);
                    }
                }

                let exporter = LogExporter::builder()
                    .with_tonic()
                    .with_endpoint(endpoint)
                    .with_metadata(MetadataMap::from_headers(header_map.clone()))
                    .build()?;

                builder = builder.with_batch_exporter(exporter);

                let metric_exporter = MetricExporter::builder()
                    .with_tonic()
                    .with_endpoint(endpoint)
                    .with_metadata(MetadataMap::from_headers(header_map))
                    .build()?;

                let reader = PeriodicReader::builder(metric_exporter)
                    .with_interval(DEFAULT_METRIC_EXPORT_INTERVAL)
                    .build();

                #[allow(clippy::redundant_clone)]
                let metrics_resource = resource.clone();
                let provider = SdkMeterProvider::builder()
                    .with_resource(metrics_resource)
                    .with_reader(reader)
                    .build();

                global::set_meter_provider(provider.clone());
                Some(provider)
            }
            OtelExporter::OtlpHttp {
                endpoint,
                headers,
                protocol,
            } => {
                debug!("Using OTLP Http exporter: {}", endpoint);

                let protocol = match protocol {
                    OtelHttpProtocol::Binary => Protocol::HttpBinary,
                    OtelHttpProtocol::Json => Protocol::HttpJson,
                };

                let exporter = LogExporter::builder()
                    .with_http()
                    .with_endpoint(endpoint)
                    .with_protocol(protocol)
                    .with_headers(headers.clone())
                    .build()?;

                builder = builder.with_batch_exporter(exporter);

                let metric_exporter = MetricExporter::builder()
                    .with_http()
                    .with_endpoint(endpoint)
                    .with_protocol(protocol)
                    .with_headers(headers.clone())
                    .build()?;

                let reader = PeriodicReader::builder(metric_exporter)
                    .with_interval(DEFAULT_METRIC_EXPORT_INTERVAL)
                    .build();

                #[allow(clippy::redundant_clone)]
                let metrics_resource = resource.clone();
                let provider = SdkMeterProvider::builder()
                    .with_resource(metrics_resource)
                    .with_reader(reader)
                    .build();

                global::set_meter_provider(provider.clone());
                Some(provider)
            }
        };

        Ok(Some(Self {
            logger: builder.build(),
            meter: meter_provider,
        }))
    }
}

impl Drop for OtelProvider {
    fn drop(&mut self) {
        let _ = self.logger.shutdown();
        if let Some(meter) = &self.meter {
            let _ = meter.shutdown();
        }
    }
}
