use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", rename_all = "camelCase")]
pub enum ImageGenerationFailure {
    UsageLimitExceeded {
        #[serde(rename = "limitId")]
        #[ts(rename = "limitId")]
        limit_id: String,
        #[serde(rename = "resetsAt")]
        #[ts(rename = "resetsAt")]
        #[ts(type = "number | null")]
        resets_at: Option<i64>,
    },
}

/// A local image file successfully read for this exact image-generation request.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct ImageGenerationInputImage {
    pub path: AbsolutePathBuf,
    /// Lowercase SHA-256 of the file bytes read before image normalization.
    pub sha256: String,
}

// Standalone image-generation item owned by the image extension. This is also
// the field-level representation exposed by app-server; core and rollout
// persistence only carry it inside an ExtensionItem envelope.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct ImageGenerationItem {
    pub id: String,
    pub status: String,
    pub revised_prompt: Option<String>,
    #[serde(flatten)]
    pub image: GeneratedImageRef,
    #[serde(default)]
    #[ts(optional)]
    pub transparent_background: Option<bool>,
    #[serde(default)]
    pub failure: Option<ImageGenerationFailure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub saved_path: Option<AbsolutePathBuf>,
    /// Observed local inputs for a completed generation, absent on older items.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<_>", optional)]
    pub input_images: Vec<ImageGenerationInputImage>,
    /// Exact nested ImageGen request ID retained only for in-process analytics.
    ///
    /// This is deliberately excluded from the extension/app-server wire shape
    /// and rollout persistence because it is not a client-facing item field.
    #[serde(skip)]
    #[schemars(skip)]
    #[ts(skip)]
    pub imagegen_request_id: Option<String>,
    /// Image generation identifier for the generated image, when provided by the image backend.
    #[serde(default)]
    pub generation_id: Option<String>,
}

/// Either inline generated-image data or its durable attachment-store identity.
///
/// `File` must remain first so rollouts written during the migration, which contain both an empty
/// `result` and a `fileId`, retain the file reference when deserialized.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(untagged)]
#[ts(untagged)]
pub enum GeneratedImageRef {
    File {
        #[serde(rename = "fileId")]
        #[ts(rename = "fileId")]
        file_id: String,
    },
    Inline {
        result: String,
    },
}
