//! Typed display-item payloads owned by Codex extensions.
//!
//! This crate intentionally sits below `codex-protocol` so core can carry
//! extension items without owning each extension's display schema.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

pub mod image_generation;

/// Canonical extension-owned turn item carried through core lifecycle events.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
pub struct ExtensionItem {
    pub id: String,
    #[serde(flatten)]
    #[ts(flatten)]
    pub payload: ExtensionItemPayload,
}

/// Closed set of extension-owned turn-item payloads that app-server projects
/// into its typed public API.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(tag = "kind", content = "payload")]
#[ts(tag = "kind", content = "payload")]
pub enum ExtensionItemPayload {
    #[serde(rename = "image_gen.image_generation")]
    #[ts(rename = "image_gen.image_generation")]
    ImageGeneration(image_generation::ImageGenerationPayload),
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
