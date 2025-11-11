use async_trait::async_trait;

use crate::error::Result;
use crate::stream::ResponseStream;
use codex_protocol::protocol::SessionSource;
use serde_json::Value;

#[async_trait]
pub trait PayloadClient: Sized {
    type Config;

    fn new(config: Self::Config) -> Result<Self>;

    /// Start a streaming request for a pre-built wire JSON payload.
    async fn stream_payload(
        &self,
        payload_json: &Value,
        session_source: Option<&SessionSource>,
    ) -> Result<ResponseStream>;
}
