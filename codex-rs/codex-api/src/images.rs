use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ImageGenerationRequest {
    pub prompt: String,
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
    pub images: Vec<ImageUrl>,
    pub prompt: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageUrl {
    pub image_url: String,
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
}
