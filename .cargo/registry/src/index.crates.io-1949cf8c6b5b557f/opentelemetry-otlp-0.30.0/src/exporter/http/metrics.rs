use std::sync::Arc;

use crate::metric::MetricsClient;
use http::{header::CONTENT_TYPE, Method};
use opentelemetry::otel_debug;
use opentelemetry_sdk::error::{OTelSdkError, OTelSdkResult};
use opentelemetry_sdk::metrics::data::ResourceMetrics;

use super::OtlpHttpClient;

impl MetricsClient for OtlpHttpClient {
    async fn export(&self, metrics: &ResourceMetrics) -> OTelSdkResult {
        let client = self
            .client
            .lock()
            .map_err(|e| OTelSdkError::InternalFailure(format!("Failed to acquire lock: {e:?}")))
            .and_then(|g| match &*g {
                Some(client) => Ok(Arc::clone(client)),
                _ => Err(OTelSdkError::AlreadyShutdown),
            })?;

        let (body, content_type) = self.build_metrics_export_body(metrics).ok_or_else(|| {
            OTelSdkError::InternalFailure("Failed to serialize metrics".to_string())
        })?;
        let mut request = http::Request::builder()
            .method(Method::POST)
            .uri(&self.collector_endpoint)
            .header(CONTENT_TYPE, content_type)
            .body(body.into())
            .map_err(|e| OTelSdkError::InternalFailure(format!("{e:?}")))?;

        for (k, v) in &self.headers {
            request.headers_mut().insert(k.clone(), v.clone());
        }

        otel_debug!(name: "HttpMetricsClient.CallingExport");
        client
            .send_bytes(request)
            .await
            .map_err(|e| OTelSdkError::InternalFailure(format!("{e:?}")))?;

        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        self.client
            .lock()
            .map_err(|e| OTelSdkError::InternalFailure(format!("Failed to acquire lock: {}", e)))?
            .take();

        Ok(())
    }
}
