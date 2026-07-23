mod file_system_handler;
mod handler;
mod process_handler;
mod processor;
mod registry;
mod session_registry;
mod transport;

pub(crate) use handler::ExecServerHandler;
pub(crate) use processor::ConnectionProcessor;
pub use transport::DEFAULT_LISTEN_URL;
pub use transport::ExecServerListenUrlParseError;

use crate::ExecServerRuntimePaths;
use crate::ExecServerTelemetry;
use codex_http_client::HttpClientFactory;

pub async fn run_main(
    listen_url: &str,
    runtime_paths: ExecServerRuntimePaths,
    http_client_factory: HttpClientFactory,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_main_with_telemetry(
        listen_url,
        runtime_paths,
        ExecServerTelemetry::default(),
        http_client_factory,
    )
    .await
}

#[tracing::instrument(
    name = "codex.exec_server",
    skip_all,
    fields(otel.kind = "internal")
)]
pub async fn run_main_with_telemetry(
    listen_url: &str,
    runtime_paths: ExecServerRuntimePaths,
    telemetry: ExecServerTelemetry,
    http_client_factory: HttpClientFactory,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    transport::run_transport(listen_url, runtime_paths, telemetry, http_client_factory).await
}

#[cfg(test)]
mod tests {
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::InMemorySpanExporter;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use tracing::instrument::WithSubscriber;
    use tracing_subscriber::prelude::*;

    use super::run_main_with_telemetry;
    use crate::ExecServerRuntimePaths;
    use crate::ExecServerTelemetry;

    #[tokio::test]
    async fn telemetry_entrypoint_emits_root_span() {
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("exec-server-test")));

        async {
            tracing::callsite::rebuild_interest_cache();
            run_main_with_telemetry(
                "invalid",
                ExecServerRuntimePaths::new(
                    std::env::current_exe().expect("current executable"),
                    /*codex_linux_sandbox_exe*/ None,
                )
                .expect("runtime paths"),
                ExecServerTelemetry::default(),
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            )
            .await
            .expect_err("invalid listen URL should fail");
        }
        .with_subscriber(subscriber)
        .await;

        provider.force_flush().expect("flush traces");
        let spans = exporter.get_finished_spans().expect("span export");
        assert!(
            spans.iter().any(|span| span.name == "codex.exec_server"),
            "root exec-server span missing: {spans:?}"
        );
    }
}
