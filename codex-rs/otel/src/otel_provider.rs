use crate::config::OtelExporter;
use crate::config::OtelHttpProtocol;
use crate::config::OtelSampler;
use crate::config::OtelSettings;
use crate::file_exporter::FileExporter;
use crate::file_exporter::create_log_file;
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_otlp::WithTonicConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::Sampler;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::Tracer;
use opentelemetry_semantic_conventions as semconv;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use std::error::Error;
use tonic::metadata::MetadataMap;
use tracing::debug;

const ENV_ATTRIBUTE: &str = "env";

pub struct OtelProvider {
    pub name: String,
    pub provider: SdkTracerProvider,
}

impl OtelProvider {
    pub fn tracer(&self) -> Tracer {
        self.provider.tracer(self.name.clone())
    }

    pub fn shutdown(&self) {
        let _ = self.provider.shutdown();
    }

    pub fn from(settings: &OtelSettings) -> Result<Option<Self>, Box<dyn Error>> {
        if !settings.enabled {
            return Ok(None);
        }

        let sampler = match settings.sampler {
            OtelSampler::AlwaysOn => Sampler::AlwaysOn,
            OtelSampler::TraceIdRatioBased(ratio) => Sampler::TraceIdRatioBased(ratio),
        };

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

        let mut builder = SdkTracerProvider::builder()
            .with_resource(resource.clone())
            .with_sampler(sampler);

        match &settings.exporter {
            OtelExporter::None => {
                debug!("No exporter enabled in OTLP settings.");
            }
            OtelExporter::OtlpFile => {
                let (log_file, log_path) = create_log_file(settings)?;

                debug!("Using OTLP File exporter: {}", log_path.display());

                let exporter = FileExporter::new(log_file, resource);
                builder = builder.with_batch_exporter(exporter);
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

                let exporter = SpanExporter::builder()
                    .with_tonic()
                    .with_endpoint(endpoint)
                    .with_metadata(MetadataMap::from_headers(header_map))
                    .build()?;

                builder = builder.with_batch_exporter(exporter);
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

                let exporter = SpanExporter::builder()
                    .with_http()
                    .with_endpoint(endpoint)
                    .with_protocol(protocol)
                    .with_headers(headers.clone())
                    .build()?;

                builder = builder.with_batch_exporter(exporter);
            }
        }

        let provider = builder.build();

        global::set_tracer_provider(provider.clone());

        Ok(Some(Self {
            name: settings.service_name.clone(),
            provider,
        }))
    }
}

impl Drop for OtelProvider {
    fn drop(&mut self) {
        let _ = self.provider.shutdown();
    }
}
