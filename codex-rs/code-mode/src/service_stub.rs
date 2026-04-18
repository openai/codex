use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use codex_protocol::ToolName;
use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::runtime::ExecuteRequest;
use crate::runtime::RuntimeResponse;
use crate::runtime::WaitRequest;

pub type CodeModeRuntimeFactory = Arc<dyn Fn() -> Arc<dyn CodeModeRuntimeService> + Send + Sync>;

#[async_trait]
pub trait CodeModeTurnHost: Send + Sync {
    async fn invoke_tool(
        &self,
        tool_name: ToolName,
        input: Option<JsonValue>,
        cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String>;

    async fn notify(&self, call_id: String, cell_id: String, text: String) -> Result<(), String>;
}

#[async_trait]
pub trait CodeModeRuntimeService: Send + Sync {
    async fn stored_values(&self) -> HashMap<String, JsonValue>;

    async fn replace_stored_values(&self, values: HashMap<String, JsonValue>);

    async fn execute(&self, request: ExecuteRequest) -> Result<RuntimeResponse, String>;

    async fn wait(&self, request: WaitRequest) -> Result<RuntimeResponse, String>;

    fn start_turn_worker(&self, host: Arc<dyn CodeModeTurnHost>) -> Box<dyn Send>;
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

    pub async fn execute(&self, request: ExecuteRequest) -> Result<RuntimeResponse, String> {
        Ok(RuntimeResponse::Result {
            cell_id: request.tool_call_id,
            content_items: Vec::new(),
            stored_values: request.stored_values,
            error_text: Some(
                "code mode runtime is unavailable in this build of codex-code-mode".to_string(),
            ),
        })
    }

    pub async fn wait(&self, request: WaitRequest) -> Result<RuntimeResponse, String> {
        Ok(RuntimeResponse::Result {
            cell_id: request.cell_id,
            content_items: Vec::new(),
            stored_values: self.stored_values().await,
            error_text: Some(
                "code mode runtime is unavailable in this build of codex-code-mode".to_string(),
            ),
        })
    }

    pub fn start_turn_worker(&self, _host: Arc<dyn CodeModeTurnHost>) -> CodeModeTurnWorker {
        CodeModeTurnWorker {}
    }
}

#[async_trait]
impl CodeModeRuntimeService for CodeModeService {
    async fn stored_values(&self) -> HashMap<String, JsonValue> {
        self.stored_values().await
    }

    async fn replace_stored_values(&self, values: HashMap<String, JsonValue>) {
        self.replace_stored_values(values).await;
    }

    async fn execute(&self, request: ExecuteRequest) -> Result<RuntimeResponse, String> {
        self.execute(request).await
    }

    async fn wait(&self, request: WaitRequest) -> Result<RuntimeResponse, String> {
        self.wait(request).await
    }

    fn start_turn_worker(&self, host: Arc<dyn CodeModeTurnHost>) -> Box<dyn Send> {
        Box::new(self.start_turn_worker(host))
    }
}

impl Default for CodeModeService {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CodeModeTurnWorker {}

pub fn default_runtime_factory() -> CodeModeRuntimeFactory {
    Arc::new(|| Arc::new(CodeModeService::new()))
}
