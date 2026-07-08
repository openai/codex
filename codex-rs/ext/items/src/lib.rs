//! Typed display items owned by Codex extensions.
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
///   "kind": "image_gen.generation",
///   "id": "call-id",
///   "status": "completed",
///   "revisedPrompt": "A blue square",
///   "result": "cG5n",
///   "savedPath": "/tmp/image.png"
/// }
/// ```
///
/// `kind` values follow `<extension_namespace>.<item_kind>`. Adding a variant
/// also requires app-server to add its typed public wrapper.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(tag = "kind")]
#[ts(tag = "kind")]
pub enum ExtensionItem {
    #[serde(rename = "image_gen.generation")]
    #[ts(rename = "image_gen.generation")]
    ImageGeneration(image_generation::ImageGenerationItem),
}

impl ExtensionItem {
    /// Returns the stable item identifier without exposing variant fields to
    /// core or rollout persistence.
    pub fn id(&self) -> &str {
        match self {
            Self::ImageGeneration(item) => &item.id,
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
