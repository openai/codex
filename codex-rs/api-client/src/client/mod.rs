use async_trait::async_trait;
use codex_otel::otel_event_manager::OtelEventManager;
use tokio::sync::mpsc;

use crate::error::Result;
use crate::stream::WireEvent;

pub mod fixtures;
pub mod http;
pub mod rate_limits;
pub mod sse;

// Legacy ResponseEvent-based decoder removed

/// Decodes framed SSE JSON into WireEvent(s).
#[async_trait]
pub trait WireResponseDecoder {
    async fn on_frame(
        &mut self,
        json: &str,
        tx: &mpsc::Sender<Result<WireEvent>>,
        otel: &OtelEventManager,
    ) -> Result<()>;
}
