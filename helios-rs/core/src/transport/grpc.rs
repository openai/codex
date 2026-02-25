//! gRPC Transport for typed communication

use super::TransportConfig;

pub struct GrpcTransport {
    config: TransportConfig,
}

impl GrpcTransport {
    pub fn new(config: TransportConfig) -> Self {
        Self { config }
    }
    
    pub async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, String> {
        // Placeholder - would use tonic
        Ok(ChatResponse::default())
    }
}

#[derive(Default)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
}

#[derive(Default)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Default)]
pub struct ChatResponse {
    pub content: String,
}
