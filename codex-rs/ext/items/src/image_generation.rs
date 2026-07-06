use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Standalone image-generation display payload owned by the image extension.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
pub struct ImageGenerationPayload {
    pub status: String,
    pub revised_prompt: Option<String>,
    pub result: String,
    pub saved_path: Option<AbsolutePathBuf>,
}
