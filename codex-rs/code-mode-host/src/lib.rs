//! Runs the gRPC code-mode host over stdio or TCP.

use std::sync::Arc;

use anyhow::Result;
use codex_code_mode_protocol::host::MAX_PENDING_DELEGATE_CALLS;
use codex_code_mode_runtime::ToolPreload;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::sync::TryAcquireError;

pub use self::grpc::GrpcCodeModeHost;
pub use self::process_metrics::CodeModeProcessMetricsCollector;
pub use self::transport::DEFAULT_LISTEN_URL;
pub use codex_code_mode_protocol::CodeModeProcessMetrics;

mod grpc;
mod grpc_stdio;
mod grpc_transport;
mod process_metrics;
mod transport;

// A host is shared by independent chats and all their subagents.
// Test live V8 isolates before increasing this, not just cell permit capacity.
const MAX_ACTIVE_CELLS: usize = 2_048;
// Leave observation and control headroom during a full callback-completion burst.
const MAX_IN_FLIGHT_REQUESTS: usize = MAX_PENDING_DELEGATE_CALLS * 2;
const MAX_RECENT_REQUEST_IDS: usize = 4096;
// Allow a full callback-completion burst plus concurrent control messages.
const OUTGOING_CHANNEL_CAPACITY: usize = MAX_PENDING_DELEGATE_CALLS * 2;

struct HostLimits {
    request_permits: Arc<Semaphore>,
    active_cell_permits: Arc<Semaphore>,
}

impl HostLimits {
    fn new() -> Self {
        Self {
            request_permits: Arc::new(Semaphore::new(MAX_IN_FLIGHT_REQUESTS)),
            active_cell_permits: Arc::new(Semaphore::new(MAX_ACTIVE_CELLS)),
        }
    }

    fn request_permit(&self) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        Arc::clone(&self.request_permits).try_acquire_owned()
    }

    fn cell_permit(&self) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        Arc::clone(&self.active_cell_permits).try_acquire_owned()
    }
}

/// Runs the code-mode host over gRPC on the configured endpoint.
pub async fn run_main(listen_url: &str) -> Result<()> {
    run_main_with_tool_preloads(listen_url, &[]).await
}

/// Opens each session on the configured transport with the supplied tool helpers.
#[tracing::instrument(
    name = "code_mode_host.run_main",
    level = "info",
    skip_all,
    fields(otel.name = "code_mode_host.run_main")
)]
pub async fn run_main_with_tool_preloads(
    listen_url: &str,
    tool_preloads: &'static [ToolPreload],
) -> Result<()> {
    transport::run_transport(
        listen_url,
        GrpcCodeModeHost::new().with_tool_preloads(tool_preloads),
    )
    .await
}

/// Runs with an optional gRPC metrics collector initialized before sandbox entry.
pub async fn run_main_with_process_metrics(
    listen_url: &str,
    collector: CodeModeProcessMetricsCollector,
) -> Result<()> {
    transport::run_transport(
        listen_url,
        GrpcCodeModeHost::with_process_metrics(collector),
    )
    .await
}
