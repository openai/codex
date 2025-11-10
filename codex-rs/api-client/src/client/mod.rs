use async_trait::async_trait;
use codex_otel::otel_event_manager::OtelEventManager;
use tokio::sync::mpsc;

use crate::error::Result;
use crate::prompt::Prompt;
use crate::stream::ResponseEvent;

pub mod fixtures;
pub mod http;
pub mod rate_limits;
pub mod sse;

/// Builds provider-specific JSON payloads from a Prompt.
pub trait PayloadBuilder {
    fn build(&self, prompt: &Prompt) -> Result<serde_json::Value>;
}

/// Decodes framed SSE JSON into ResponseEvent(s).
/// Implementations may keep state across frames (e.g., Chat function-call state).
#[async_trait]
pub trait ResponseDecoder {
    async fn on_frame(
        &mut self,
        json: &str,
        tx: &mpsc::Sender<Result<ResponseEvent>>,
        otel: &OtelEventManager,
    ) -> Result<()>;
}
