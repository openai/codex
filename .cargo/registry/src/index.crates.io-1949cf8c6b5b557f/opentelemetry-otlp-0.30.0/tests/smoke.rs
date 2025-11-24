use futures_util::StreamExt;
use opentelemetry::global;
use opentelemetry::trace::{Span, SpanKind, Tracer};
use opentelemetry_otlp::{WithExportConfig, WithTonicConfig};
use opentelemetry_proto::tonic::collector::trace::v1::{
    trace_service_server::{TraceService, TraceServiceServer},
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use std::{net::SocketAddr, sync::Mutex};
use tokio::sync::mpsc;
use tokio_stream::wrappers::TcpListenerStream;
#[cfg(feature = "gzip-tonic")]
use tonic::codec::CompressionEncoding;

struct MockServer {
    tx: Mutex<mpsc::Sender<ExportTraceServiceRequest>>,
}

impl MockServer {
    pub fn new(tx: mpsc::Sender<ExportTraceServiceRequest>) -> Self {
        Self { tx: Mutex::new(tx) }
    }
}

#[tonic::async_trait]
impl TraceService for MockServer {
    async fn export(
        &self,
        request: tonic::Request<ExportTraceServiceRequest>,
    ) -> Result<tonic::Response<ExportTraceServiceResponse>, tonic::Status> {
        println!("Sending request into channel...");
        // assert we have required metadata key
        assert_eq!(
            request.metadata().get("x-header-key"),
            Some(&("header-value".parse().unwrap()))
        );
        self.tx
            .lock()
            .unwrap()
            .try_send(request.into_inner())
            .expect("Channel full");
        Ok(tonic::Response::new(ExportTraceServiceResponse {
            partial_success: None,
        }))
    }
}

async fn setup() -> (SocketAddr, mpsc::Receiver<ExportTraceServiceRequest>) {
    let addr: SocketAddr = "[::1]:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");
    let addr = listener.local_addr().unwrap();
    let stream = TcpListenerStream::new(listener).map(|s| {
        if let Ok(ref s) = s {
            println!("Got new conn at {}", s.peer_addr().unwrap());
        }
        s
    });

    let (req_tx, req_rx) = mpsc::channel(10);
    #[cfg(feature = "gzip-tonic")]
    let service = TraceServiceServer::new(MockServer::new(req_tx))
        .accept_compressed(CompressionEncoding::Gzip);
    #[cfg(not(feature = "gzip-tonic"))]
    let service = TraceServiceServer::new(MockServer::new(req_tx));
    tokio::task::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming(stream)
            .await
            .expect("Server failed");
    });
    (addr, req_rx)
}

#[tokio::test(flavor = "multi_thread")]
async fn smoke_tracer() {
    println!("Starting server setup...");
    let (addr, mut req_rx) = setup().await;

    {
        println!("Installing tracer provider...");
        let mut metadata = tonic::metadata::MetadataMap::new();
        metadata.insert("x-header-key", "header-value".parse().unwrap());
        let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(
                #[cfg(feature = "gzip-tonic")]
                opentelemetry_otlp::SpanExporter::builder()
                    .with_tonic()
                    .with_compression(opentelemetry_otlp::Compression::Gzip)
                    .with_endpoint(format!("http://{}", addr))
                    .with_metadata(metadata)
                    .build()
                    .expect("gzip-tonic SpanExporter failed to build"),
                #[cfg(not(feature = "gzip-tonic"))]
                opentelemetry_otlp::SpanExporter::builder()
                    .with_tonic()
                    .with_endpoint(format!("http://{}", addr))
                    .with_metadata(metadata)
                    .build()
                    .expect("NON gzip-tonic SpanExporter failed to build"),
            )
            .build();

        global::set_tracer_provider(tracer_provider.clone());

        let tracer = global::tracer("smoke");

        println!("Sending span...");
        let mut span = tracer
            .span_builder("my-test-span")
            .with_kind(SpanKind::Server)
            .start(&tracer);
        span.add_event("my-test-event", vec![]);
        span.end();

        tracer_provider
            .shutdown()
            .expect("tracer_provider should shutdown successfully");
    }

    println!("Waiting for request...");
    let req = req_rx.recv().await.expect("missing export request");
    let first_span = req
        .resource_spans
        .first()
        .unwrap()
        .scope_spans
        .first()
        .unwrap()
        .spans
        .first()
        .unwrap();
    assert_eq!("my-test-span", first_span.name);
    let first_event = first_span.events.first().unwrap();
    assert_eq!("my-test-event", first_event.name);
}
