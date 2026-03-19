use crate::error::Result;
use async_trait::async_trait;
use codex_app_server_protocol::ModelCompactEnvelope;
use codex_app_server_protocol::ModelRequestEnvelope;
use codex_app_server_protocol::ModelRequestError;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq)]
pub struct DelegatedModelRequest {
    pub thread_id: ThreadId,
    pub turn_id: String,
    pub request_id: String,
    pub request: ModelRequestEnvelope,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DelegatedModelCompactRequest {
    pub thread_id: ThreadId,
    pub turn_id: String,
    pub request_id: String,
    pub request: ModelCompactEnvelope,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DelegatedModelEvent {
    StreamMetadata(HashMap<String, String>),
    StreamEvent(Value),
    RequestFailed(ModelRequestError),
}

#[async_trait]
pub trait DelegatedModelTransport: Send + Sync {
    async fn start_model_request(
        &self,
        request: DelegatedModelRequest,
    ) -> Result<mpsc::Receiver<DelegatedModelEvent>>;

    async fn run_model_compact_request(
        &self,
        request: DelegatedModelCompactRequest,
    ) -> Result<Vec<ResponseItem>>;
}
