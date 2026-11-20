use codex_protocol::models::ImageReference;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use serde_json::value::RawValue;

const MAX_IMAGE_METADATA_BYTES: usize = 1024 * 1024;

// Protobuf Struct objects consume three decode levels per nesting level.
// Leave room for the enclosing CCA and app-server event and history messages.
const MAX_IMAGE_METADATA_DEPTH: usize = 24;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    #[serde(flatten)]
    pub output: Option<ImageOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<ImageBackground>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<ImageQuality>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ImageEditRequest {
    pub images: Vec<ImageReference>,
    pub prompt: String,
    #[serde(flatten)]
    pub output: Option<ImageOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<ImageBackground>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<ImageQuality>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

/// Optional GIF controls accepted by the Images generation and edit endpoints.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq)]
pub struct GifOptions {
    /// Number of frames (2–16). Defaults to 9.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 2, max = 16))]
    pub frame_count: Option<u32>,
    /// Duration of each frame in milliseconds (20–2000). Defaults to 200.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 20, max = 2000))]
    pub frame_duration_ms: Option<u32>,
}

/// The Images API expects the format and its controls as sibling fields.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(tag = "output_format", rename_all = "lowercase")]
pub enum ImageOutput {
    Gif(GifOptions),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageBackground {
    Transparent,
    Opaque,
    Auto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageQuality {
    Low,
    Medium,
    High,
    Auto,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ImageResponse {
    pub created: u64,
    pub data: Vec<ImageData>,
    #[serde(default)]
    pub background: Option<ImageBackground>,
    #[serde(default)]
    pub quality: Option<ImageQuality>,
    #[serde(default)]
    pub size: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ImageData {
    pub b64_json: String,
    #[serde(default)]
    pub generation_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_image_metadata")]
    pub image_metadata: Map<String, Value>,
}

fn deserialize_image_metadata<'de, D>(deserializer: D) -> Result<Map<String, Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Read the field independently so a depth error cannot invalidate the surrounding image.
    let raw = <&RawValue>::deserialize(deserializer)?;
    if raw.get().len() > MAX_IMAGE_METADATA_BYTES {
        return Ok(Map::new());
    }
    Ok(serde_json::from_str::<Map<String, Value>>(raw.get())
        .ok()
        .filter(|metadata| {
            metadata
                .values()
                .all(|value| fits_metadata_depth(value, /*depth*/ 1))
        })
        .unwrap_or_default())
}

fn fits_metadata_depth(value: &Value, depth: usize) -> bool {
    if depth > MAX_IMAGE_METADATA_DEPTH {
        return false;
    }
    match value {
        Value::Array(items) => items
            .iter()
            .all(|item| fits_metadata_depth(item, depth + 1)),
        Value::Object(fields) => fields
            .values()
            .all(|item| fits_metadata_depth(item, depth + 1)),
        _ => true,
    }
}
