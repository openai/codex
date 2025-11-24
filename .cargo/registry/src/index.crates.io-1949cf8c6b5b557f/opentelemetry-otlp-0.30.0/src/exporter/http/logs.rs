use super::OtlpHttpClient;
use http::{header::CONTENT_TYPE, Method};
use opentelemetry::otel_debug;
use opentelemetry_sdk::error::{OTelSdkError, OTelSdkResult};
use opentelemetry_sdk::logs::{LogBatch, LogExporter};
use std::time;

impl LogExporter for OtlpHttpClient {
    async fn export(&self, batch: LogBatch<'_>) -> OTelSdkResult {
        let client = self
            .client
            .lock()
            .map_err(|e| OTelSdkError::InternalFailure(format!("Mutex lock failed: {}", e)))?
            .clone()
            .ok_or(OTelSdkError::AlreadyShutdown)?;

        let (body, content_type) = self
            .build_logs_export_body(batch)
            .map_err(OTelSdkError::InternalFailure)?;

        let mut request = http::Request::builder()
            .method(Method::POST)
            .uri(&self.collector_endpoint)
            .header(CONTENT_TYPE, content_type)
            .body(body.into())
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;

        for (k, v) in &self.headers {
            request.headers_mut().insert(k.clone(), v.clone());
        }

        let request_uri = request.uri().to_string();
        otel_debug!(name: "HttpLogsClient.CallingExport");
        let response = client
            .send_bytes(request)
            .await
            .map_err(|e| OTelSdkError::InternalFailure(format!("{e:?}")))?;
        if !response.status().is_success() {
            let error = format!(
                "OpenTelemetry logs export failed. Url: {}, Status Code: {}, Response: {:?}",
                request_uri,
                response.status().as_u16(),
                response.body()
            );
            return Err(OTelSdkError::InternalFailure(error));
        }
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: time::Duration) -> OTelSdkResult {
        let mut client_guard = self.client.lock().map_err(|e| {
            OTelSdkError::InternalFailure(format!("Failed to acquire client lock: {}", e))
        })?;

        if client_guard.take().is_none() {
            return Err(OTelSdkError::AlreadyShutdown);
        }

        Ok(())
    }

    fn set_resource(&mut self, resource: &opentelemetry_sdk::Resource) {
        self.resource = resource.into();
    }
}
