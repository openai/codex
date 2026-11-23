//! Compile-time exporter selection for providers that only send one signal over HTTP.

use super::OtelProvider;
use crate::config::OtelExporter;
use crate::config::OtelHttpProtocol;
use crate::config::OtelSettings;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::error::Error;
use std::marker::PhantomData;

/// Selects the builders each provider needs so the linker can drop the others.
pub(crate) trait ExporterScope {
    const ALL: bool = false;
    const TRACES: bool = Self::ALL;
    const METRICS: bool = Self::ALL;
    const CONFIGURE_HTTP_CLIENT: bool = true;
}

pub(crate) struct AllExporters;
impl ExporterScope for AllExporters {
    const ALL: bool = true;
}

pub(super) struct HttpTraces;
impl ExporterScope for HttpTraces {
    const TRACES: bool = true;
}

pub(super) struct HttpMetrics;
impl ExporterScope for HttpMetrics {
    const METRICS: bool = true;
}

struct SdkDefault<E>(PhantomData<E>);
impl<E: ExporterScope> ExporterScope for SdkDefault<E> {
    const TRACES: bool = E::TRACES;
    const METRICS: bool = E::METRICS;
    const CONFIGURE_HTTP_CLIENT: bool = false;
}

/// Identifies a service that exports a single signal using the OTLP SDK's default HTTP client.
pub struct BlockingHttpService<'a> {
    pub environment: &'a str,
    pub name: &'a str,
    pub version: &'a str,
}

/// Destinations supported by the blocking HTTP metrics provider.
pub enum BlockingMetricsDestination<'a> {
    Statsig,
    OtlpJson(&'a str),
}

impl BlockingHttpService<'_> {
    fn json_exporter(endpoint: &str) -> OtelExporter {
        OtelExporter::OtlpHttp {
            endpoint: endpoint.to_string(),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        }
    }

    fn settings(&self) -> OtelSettings {
        OtelSettings {
            http_client_factory: codex_http_client::HttpClientFactory::new(
                codex_http_client::OutboundProxyPolicy::ReqwestDefault,
            ),
            headers_helper: None,
            environment: self.environment.to_string(),
            service_name: self.name.to_string(),
            service_version: self.version.to_string(),
            exporter: OtelExporter::None,
            trace_exporter: OtelExporter::None,
            metrics_exporter: OtelExporter::None,
            runtime_metrics: false,
            span_attributes: HashMap::new(),
            tracestate: BTreeMap::new(),
        }
    }

    /// Export traces to an OTLP/HTTP JSON endpoint using the dedicated batch processor.
    pub fn traces(&self, endpoint: &str) -> Result<OtelProvider, Box<dyn Error>> {
        let mut settings = self.settings();
        settings.trace_exporter = Self::json_exporter(endpoint);
        OtelProvider::try_new_with_exporters::<SdkDefault<HttpTraces>>(&settings)?
            .ok_or_else(|| "OTLP/HTTP trace exporter was unexpectedly disabled".into())
    }

    /// Export metrics using the SDK's periodic reader.
    pub fn metrics(
        &self,
        destination: BlockingMetricsDestination<'_>,
    ) -> Result<Option<OtelProvider>, Box<dyn Error>> {
        let mut settings = self.settings();
        settings.metrics_exporter = match destination {
            BlockingMetricsDestination::Statsig => OtelExporter::Statsig,
            BlockingMetricsDestination::OtlpJson(endpoint) => Self::json_exporter(endpoint),
        };
        OtelProvider::try_new_with_exporters::<SdkDefault<HttpMetrics>>(&settings)
    }
}
