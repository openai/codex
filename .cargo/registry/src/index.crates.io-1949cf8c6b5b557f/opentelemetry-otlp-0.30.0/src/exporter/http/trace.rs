use std::sync::Arc;

use super::OtlpHttpClient;
use http::{header::CONTENT_TYPE, Method};
use opentelemetry::otel_debug;
use opentelemetry_sdk::{
    error::{OTelSdkError, OTelSdkResult},
    trace::{SpanData, SpanExporter},
};

impl SpanExporter for OtlpHttpClient {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let client = match self
            .client
            .lock()
            .map_err(|e| OTelSdkError::InternalFailure(format!("Mutex lock failed: {}", e)))
            .and_then(|g| match &*g {
                Some(client) => Ok(Arc::clone(client)),
                _ => Err(OTelSdkError::AlreadyShutdown),
            }) {
            Ok(client) => client,
            Err(err) => return Err(err),
        };

        let (body, content_type) = match self.build_trace_export_body(batch) {
            Ok(body) => body,
            Err(e) => return Err(OTelSdkError::InternalFailure(e.to_string())),
        };

        let mut request = match http::Request::builder()
            .method(Method::POST)
            .uri(&self.collector_endpoint)
            .header(CONTENT_TYPE, content_type)
            .body(body.into())
        {
            Ok(req) => req,
            Err(e) => return Err(OTelSdkError::InternalFailure(e.to_string())),
        };

        for (k, v) in &self.headers {
            request.headers_mut().insert(k.clone(), v.clone());
        }

        let request_uri = request.uri().to_string();
        otel_debug!(name: "HttpTracesClient.CallingExport");
        let response = client
            .send_bytes(request)
            .await
            .map_err(|e| OTelSdkError::InternalFailure(format!("{e:?}")))?;

        if !response.status().is_success() {
            let error = format!(
                "OpenTelemetry trace export failed. Url: {}, Status Code: {}, Response: {:?}",
                response.status().as_u16(),
                request_uri,
                response.body()
            );
            return Err(OTelSdkError::InternalFailure(error));
        }

        Ok(())
    }

    fn shutdown(&mut self) -> OTelSdkResult {
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
