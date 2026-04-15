use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::ExecuteRequest;
use crate::RuntimeResponse;
use crate::WaitRequest;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait CodeModeTurnHost: Send + Sync {
    async fn invoke_tool(
        &self,
        tool_name: String,
        input: Option<JsonValue>,
        cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String>;

    async fn notify(&self, call_id: String, cell_id: String, text: String) -> Result<(), String>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait CodeModeRuntime: Send + Sync {
    async fn stored_values(&self) -> HashMap<String, JsonValue>;

    async fn replace_stored_values(&self, values: HashMap<String, JsonValue>);

    async fn execute(&self, request: ExecuteRequest) -> Result<RuntimeResponse, String>;

    async fn wait(&self, request: WaitRequest) -> Result<RuntimeResponse, String>;

    fn start_turn_worker(
        &self,
        host: Arc<dyn CodeModeTurnHost>,
    ) -> Box<dyn CodeModeTurnWorkerHandle>;
}

pub struct CodeModeService {
    stored_values: Mutex<HashMap<String, JsonValue>>,
}

impl CodeModeService {
    pub fn new() -> Self {
        Self {
            stored_values: Mutex::new(HashMap::new()),
        }
    }

    pub async fn stored_values(&self) -> HashMap<String, JsonValue> {
        self.stored_values.lock().await.clone()
    }

    pub async fn replace_stored_values(&self, values: HashMap<String, JsonValue>) {
        *self.stored_values.lock().await = values;
    }

    pub async fn execute(&self, _request: ExecuteRequest) -> Result<RuntimeResponse, String> {
        Err(
            "native code mode runtime is unavailable on wasm32; inject a browser CodeModeRuntime"
                .to_string(),
        )
    }

    pub async fn wait(&self, request: WaitRequest) -> Result<RuntimeResponse, String> {
        Ok(RuntimeResponse::Result {
            cell_id: request.cell_id,
            content_items: Vec::new(),
            stored_values: self.stored_values().await,
            error_text: Some(
                "code mode wait is unavailable on wasm32 without an injected runtime".to_string(),
            ),
        })
    }

    pub fn start_turn_worker(&self, _host: Arc<dyn CodeModeTurnHost>) -> CodeModeTurnWorker {
        CodeModeTurnWorker
    }
}

impl Default for CodeModeService {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CodeModeTurnWorker;

pub trait CodeModeTurnWorkerHandle: Send {}

impl CodeModeTurnWorkerHandle for CodeModeTurnWorker {}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl CodeModeRuntime for CodeModeService {
    async fn stored_values(&self) -> HashMap<String, JsonValue> {
        CodeModeService::stored_values(self).await
    }

    async fn replace_stored_values(&self, values: HashMap<String, JsonValue>) {
        CodeModeService::replace_stored_values(self, values).await;
    }

    async fn execute(&self, request: ExecuteRequest) -> Result<RuntimeResponse, String> {
        CodeModeService::execute(self, request).await
    }

    async fn wait(&self, request: WaitRequest) -> Result<RuntimeResponse, String> {
        CodeModeService::wait(self, request).await
    }

    fn start_turn_worker(
        &self,
        host: Arc<dyn CodeModeTurnHost>,
    ) -> Box<dyn CodeModeTurnWorkerHandle> {
        Box::new(CodeModeService::start_turn_worker(self, host))
    }
}
