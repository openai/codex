//! OTLP SDK transports remain disabled under application destination restrictions.
//! Export futures are revocable, including HTTP exporters driven from SDK worker threads.

use bytes::Bytes;
use codex_http_client::NetworkPolicy;
use opentelemetry_http::HttpError;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkError;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::logs::LogBatch;
use opentelemetry_sdk::logs::LogExporter;
use opentelemetry_sdk::metrics::Temporality;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::metrics::exporter::PushMetricExporter;
use opentelemetry_sdk::trace::SpanData;
use opentelemetry_sdk::trace::SpanExporter;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct PolicyExporter<E> {
    pub(crate) exporter: E,
    pub(crate) policy: NetworkPolicy,
}

fn denied(error: codex_http_client::NetworkPolicyDenied) -> OTelSdkError {
    OTelSdkError::InternalFailure(error.to_string())
}

impl<E: SpanExporter> SpanExporter for PolicyExporter<E> {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let permit = self.policy.acquire_for_unsupported_sdk().map_err(denied)?;
        let export = self.exporter.export(batch);
        permit.run(export).await.map_err(denied)?
    }
    fn shutdown_with_timeout(&mut self, timeout: Duration) -> OTelSdkResult {
        self.exporter.shutdown_with_timeout(timeout)
    }
    fn force_flush(&mut self) -> OTelSdkResult {
        self.exporter.force_flush()
    }
    fn set_resource(&mut self, resource: &Resource) {
        self.exporter.set_resource(resource);
    }
}

impl<E: LogExporter> LogExporter for PolicyExporter<E> {
    async fn export(&self, batch: LogBatch<'_>) -> OTelSdkResult {
        let permit = self.policy.acquire_for_unsupported_sdk().map_err(denied)?;
        let export = self.exporter.export(batch);
        permit.run(export).await.map_err(denied)?
    }
    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.exporter.shutdown_with_timeout(timeout)
    }
    fn set_resource(&mut self, resource: &Resource) {
        self.exporter.set_resource(resource);
    }
}

impl<E: PushMetricExporter> PushMetricExporter for PolicyExporter<E> {
    async fn export(&self, metrics: &ResourceMetrics) -> OTelSdkResult {
        let permit = self.policy.acquire_for_unsupported_sdk().map_err(denied)?;
        let export = self.exporter.export(metrics);
        permit.run(export).await.map_err(denied)?
    }
    fn force_flush(&self) -> OTelSdkResult {
        self.exporter.force_flush()
    }
    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.exporter.shutdown_with_timeout(timeout)
    }
    fn temporality(&self) -> Temporality {
        self.exporter.temporality()
    }
}

/// Keeps blocking SDK worker threads from performing uncancellable HTTP I/O.
#[derive(Debug)]
pub(crate) struct RuntimeHttpClient {
    pub(crate) client: codex_http_client::HttpClient,
    pub(crate) timeout: Duration,
    pub(crate) runtime: tokio::runtime::Handle,
}

impl opentelemetry_http::HttpClient for RuntimeHttpClient {
    fn send_bytes<'client, 'request>(
        &'client self,
        request: http::Request<Bytes>,
    ) -> Pin<Box<dyn Future<Output = Result<http::Response<Bytes>, HttpError>> + Send + 'request>>
    where
        'client: 'request,
        Self: 'request,
    {
        Box::pin(async move {
            let client = self.client.clone();
            let timeout = self.timeout;
            let mut requests = tokio::task::JoinSet::new();
            requests.spawn_on(
                async move {
                    let (parts, body) = request.into_parts();
                    let response = client
                        .request(parts.method, parts.uri.to_string())
                        .version(parts.version)
                        .headers(parts.headers)
                        .body(body)
                        .timeout(timeout)
                        .send()
                        .await?
                        .error_for_status()?;
                    let mut builder = http::Response::builder().status(response.status());
                    if let Some(headers) = builder.headers_mut() {
                        *headers = response.headers().clone();
                    }
                    Ok::<_, HttpError>(builder.body(response.bytes().await?)?)
                },
                &self.runtime,
            );
            // Dropping the SDK export future drops this set and aborts the HTTP request.
            requests
                .join_next()
                .await
                .ok_or("telemetry request task disappeared")??
        })
    }
}
