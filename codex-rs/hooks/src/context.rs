//! Model-visible hook context. Plain text retains its source attribution, while
//! encrypted MCP output retains the exact tool identity needed for tool-output admission.

use codex_protocol::models::ContentItemMetadata;

/// The model-visible payload produced by a hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookContextPayload {
    Text(String),
    EncryptedToolOutput {
        name: String,
        namespace: Option<String>,
        encrypted_content: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookContext {
    pub payload: HookContextPayload,
    pub metadata: ContentItemMetadata,
}

impl HookContext {
    pub fn harness(text: String) -> Self {
        Self {
            payload: HookContextPayload::Text(text),
            metadata: ContentItemMetadata::harness(),
        }
    }
}
