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
///
/// The item is serialized as a flattened, namespaced envelope:
///
/// ```json
/// {
///   "id": "call-id",
///   "kind": "image_gen.generation",
///   "payload": {
///     "status": "completed",
///     "revised_prompt": "A blue square",
///     "result": "cG5n",
///     "saved_path": null
///   }
/// }
/// ```
///
/// `kind` values follow `<extension_namespace>.<item_kind>`. Adding a payload
/// variant also requires app-server to add its typed public projection.
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
    #[serde(rename = "image_gen.generation")]
    #[ts(rename = "image_gen.generation")]
    ImageGeneration(image_generation::ImageGenerationPayload),
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
