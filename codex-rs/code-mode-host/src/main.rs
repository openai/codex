use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use codex_otel::OtelExporter;
use codex_otel::OtelHttpProtocol;
use codex_otel::OtelProvider;
use codex_otel::OtelSettings;
use codex_otel_trace_websocket::TraceWebSocket;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const OTEL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 5);

#[derive(Debug, Parser)]
struct Cli {
    /// Transport endpoint: `stdio`, `stdio://`, or `grpc://IP:PORT`.
    #[arg(
        long,
        value_name = "URL",
        default_value = codex_code_mode_host::DEFAULT_LISTEN_URL
    )]
    listen: String,

    /// Optional WebSocket endpoint that streams only raw OTLP trace batches.
    #[arg(long, value_name = "URL")]
    otel_trace_listen: Option<String>,

    /// Optional OTLP/HTTP JSON trace exporter endpoint, analogous to
    /// `otel.trace_exporter` in app-server configuration.
    #[arg(long, value_name = "URL", conflicts_with = "otel_trace_listen")]
    otel_trace_exporter: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut trace_transport = if let Some(trace_listen) = cli.otel_trace_listen.as_deref() {
        Some(TraceWebSocket::start(trace_listen).await?)
    } else {
        None
    };
    let trace_exporter_endpoint = trace_transport
        .as_ref()
        .map(TraceWebSocket::exporter_endpoint)
        .or(cli.otel_trace_exporter.as_deref());
    let otel = trace_exporter_endpoint
        .map(build_trace_provider)
        .transpose()?;
    let otel_layer = otel.as_ref().and_then(OtelProvider::tracing_layer);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false)
                .with_filter(tracing_subscriber::filter::LevelFilter::INFO),
        )
        .with(otel_layer)
        .init();
    if let Some(trace_transport) = trace_transport.as_ref() {
        let listen_addr = trace_transport.listen_addr();
        tracing::info!("codex-code-mode-host OTEL trace websocket listening on ws://{listen_addr}");
    }
    tracing::info_span!(
        "code_mode_host.startup",
        otel.name = "code_mode_host.startup"
    )
    .in_scope(|| {});

    let main_transport = codex_code_mode_host::run_main(&cli.listen);
    let result = match trace_transport.as_mut() {
        Some(trace_transport) => tokio::select! {
            result = main_transport => result,
            result = trace_transport.wait_for_failure() => result,
        },
        None => main_transport.await,
    };
    if let Some(otel) = otel
        && let Err(error) = otel.shutdown_with_timeout(OTEL_SHUTDOWN_TIMEOUT).await
    {
        tracing::warn!(%error, "failed to finish code-mode host telemetry shutdown");
    }
    drop(trace_transport);
    result
}

fn build_trace_provider(endpoint: &str) -> anyhow::Result<OtelProvider> {
    OtelProvider::try_new(&OtelSettings {
        http_client_factory: codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
        environment: "code-mode-host".to_string(),
        service_name: "codex-code-mode-host".to_string(),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        codex_home: PathBuf::from("/tmp"),
        exporter: OtelExporter::None,
        trace_exporter: OtelExporter::OtlpHttp {
            endpoint: endpoint.to_string(),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        },
        metrics_exporter: OtelExporter::None,
        runtime_metrics: false,
        span_attributes: BTreeMap::new(),
        tracestate: BTreeMap::new(),
    })
    .map_err(|error| anyhow::anyhow!("failed to build code-mode host OTEL provider: {error}"))?
    .context("code-mode host OTEL trace provider was unexpectedly disabled")
}
