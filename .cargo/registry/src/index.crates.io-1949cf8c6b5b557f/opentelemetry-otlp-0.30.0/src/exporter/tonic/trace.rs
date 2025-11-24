use core::fmt;
use tokio::sync::Mutex;

use opentelemetry::otel_debug;
use opentelemetry_proto::tonic::collector::trace::v1::{
    trace_service_client::TraceServiceClient, ExportTraceServiceRequest,
};
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::error::OTelSdkError;
use opentelemetry_sdk::{
    error::OTelSdkResult,
    trace::{SpanData, SpanExporter},
};
use tonic::{codegen::CompressionEncoding, service::Interceptor, transport::Channel, Request};

use super::BoxInterceptor;

pub(crate) struct TonicTracesClient {
    inner: Option<ClientInner>,
    #[allow(dead_code)]
    // <allow dead> would be removed once we support set_resource for metrics.
    resource: opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema,
}

struct ClientInner {
    client: TraceServiceClient<Channel>,
    interceptor: Mutex<BoxInterceptor>,
}

impl fmt::Debug for TonicTracesClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TonicTracesClient")
    }
}

impl TonicTracesClient {
    pub(super) fn new(
        channel: Channel,
        interceptor: BoxInterceptor,
        compression: Option<CompressionEncoding>,
    ) -> Self {
        let mut client = TraceServiceClient::new(channel);
        if let Some(compression) = compression {
            client = client
                .send_compressed(compression)
                .accept_compressed(compression);
        }

        otel_debug!(name: "TonicsTracesClientBuilt");

        TonicTracesClient {
            inner: Some(ClientInner {
                client,
                interceptor: Mutex::new(interceptor),
            }),
            resource: Default::default(),
        }
    }
}

impl SpanExporter for TonicTracesClient {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let (mut client, metadata, extensions) = match &self.inner {
            Some(inner) => {
                let (m, e, _) = inner
                    .interceptor
                    .lock()
                    .await // tokio::sync::Mutex doesn't return a poisoned error, so we can safely use the interceptor here
                    .call(Request::new(()))
                    .map_err(|e| OTelSdkError::InternalFailure(format!("error: {:?}", e)))?
                    .into_parts();
                (inner.client.clone(), m, e)
            }
            None => return Err(OTelSdkError::AlreadyShutdown),
        };

        let resource_spans = group_spans_by_resource_and_scope(batch, &self.resource);

        otel_debug!(name: "TonicsTracesClient.CallingExport");

        client
            .export(Request::from_parts(
                metadata,
                extensions,
                ExportTraceServiceRequest { resource_spans },
            ))
            .await
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;
        Ok(())
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        match self.inner.take() {
            Some(_) => Ok(()), // Successfully took `inner`, indicating a successful shutdown.
            None => Err(OTelSdkError::AlreadyShutdown), // `inner` was already `None`, meaning it's already shut down.
        }
    }

    fn set_resource(&mut self, resource: &opentelemetry_sdk::Resource) {
        self.resource = resource.into();
    }
}
