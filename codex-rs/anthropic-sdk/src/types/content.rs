use serde::Deserialize;
use serde::Serialize;

// ============================================================================
// Image media types
// ============================================================================

/// Supported image media types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageMediaType {
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/gif")]
    Gif,
    #[serde(rename = "image/webp")]
    Webp,
}

// ============================================================================
// Input content blocks (for requests)
// ============================================================================

/// Content that can be sent in a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockParam {
    /// Text content.
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },

    /// Image content.
    Image {
        source: ImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },

    /// Tool use request (assistant -> user).
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Tool result (user -> assistant).
    ToolResult {
        tool_use_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<ToolResultContent>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

/// Image source for image content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// Base64-encoded image data.
    Base64 {
        /// The base64-encoded image data.
        data: String,
        /// Media type.
        media_type: ImageMediaType,
    },
    /// URL to an image.
    Url {
        /// The URL of the image.
        url: String,
    },
}

/// Content for tool result blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    /// Simple text result.
    Text(String),
    /// Multiple content blocks.
    Blocks(Vec<ToolResultContentBlock>),
}

/// Content blocks allowed in tool results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultContentBlock {
    Text { text: String },
    Image { source: ImageSource },
}

// ============================================================================
// Output content blocks (from responses)
// ============================================================================

/// Content block in a response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text output.
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        citations: Option<Vec<TextCitation>>,
    },

    /// Tool use request from the model.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Thinking block (extended thinking mode).
    Thinking {
        /// The thinking content.
        thinking: String,
        /// Signature for verification (required).
        signature: String,
    },

    /// Redacted thinking (when thinking is hidden).
    RedactedThinking { data: String },
}

/// Citation for text content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextCitation {
    #[serde(rename = "type")]
    pub citation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cited_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_char_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_char_index: Option<i32>,
}

// ============================================================================
// System prompt
// ============================================================================

/// System prompt content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemPrompt {
    /// Simple text system prompt.
    Text(String),
    /// Multiple text blocks with optional cache control.
    Blocks(Vec<SystemPromptBlock>),
}

/// A block in a system prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPromptBlock {
    /// Block type (always "text").
    #[serde(rename = "type")]
    pub block_type: String,
    /// The text content.
    pub text: String,
    /// Optional cache control.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl SystemPromptBlock {
    /// Create a new system prompt block.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            block_type: "text".to_string(),
            text: text.into(),
            cache_control: None,
        }
    }

    /// Create a system prompt block with cache control.
    pub fn with_cache(text: impl Into<String>, cache_control: CacheControl) -> Self {
        Self {
            block_type: "text".to_string(),
            text: text.into(),
            cache_control: Some(cache_control),
        }
    }
}

/// Cache control type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheControlType {
    /// Ephemeral cache control.
    Ephemeral,
}

/// Cache TTL (time to live).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheTtl {
    /// 5 minute TTL.
    #[serde(rename = "5m")]
    FiveMinutes,
    /// 1 hour TTL.
    #[serde(rename = "1h")]
    OneHour,
}

/// Cache control settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    /// The type of cache control.
    #[serde(rename = "type")]
    pub control_type: CacheControlType,
    /// Optional TTL (defaults to 5m if not specified).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<CacheTtl>,
}

impl CacheControl {
    /// Create ephemeral cache control (defaults to 5m TTL).
    pub fn ephemeral() -> Self {
        Self {
            control_type: CacheControlType::Ephemeral,
            ttl: None,
        }
    }

    /// Create ephemeral cache control with 5 minute TTL.
    pub fn ephemeral_5m() -> Self {
        Self {
            control_type: CacheControlType::Ephemeral,
            ttl: Some(CacheTtl::FiveMinutes),
        }
    }

    /// Create ephemeral cache control with 1 hour TTL.
    pub fn ephemeral_1h() -> Self {
        Self {
            control_type: CacheControlType::Ephemeral,
            ttl: Some(CacheTtl::OneHour),
        }
    }
}

// ============================================================================
// Helper implementations
// ============================================================================

impl From<String> for ContentBlockParam {
    fn from(text: String) -> Self {
        Self::Text {
            text,
            cache_control: None,
        }
    }
}

impl From<&str> for ContentBlockParam {
    fn from(text: &str) -> Self {
        Self::Text {
            text: text.to_string(),
            cache_control: None,
        }
    }
}

impl ContentBlockParam {
    /// Create a text content block.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
            cache_control: None,
        }
    }

    /// Create a text content block with cache control.
    pub fn text_with_cache(text: impl Into<String>, cache_control: CacheControl) -> Self {
        Self::Text {
            text: text.into(),
            cache_control: Some(cache_control),
        }
    }

    /// Create an image content block from base64 data.
    pub fn image_base64(data: impl Into<String>, media_type: ImageMediaType) -> Self {
        Self::Image {
            source: ImageSource::Base64 {
                data: data.into(),
                media_type,
            },
            cache_control: None,
        }
    }

    /// Create an image content block from base64 data with cache control.
    pub fn image_base64_with_cache(
        data: impl Into<String>,
        media_type: ImageMediaType,
        cache_control: CacheControl,
    ) -> Self {
        Self::Image {
            source: ImageSource::Base64 {
                data: data.into(),
                media_type,
            },
            cache_control: Some(cache_control),
        }
    }

    /// Create an image content block from a URL.
    pub fn image_url(url: impl Into<String>) -> Self {
        Self::Image {
            source: ImageSource::Url { url: url.into() },
            cache_control: None,
        }
    }

    /// Create an image content block from a URL with cache control.
    pub fn image_url_with_cache(url: impl Into<String>, cache_control: CacheControl) -> Self {
        Self::Image {
            source: ImageSource::Url { url: url.into() },
            cache_control: Some(cache_control),
        }
    }

    /// Create a tool result content block.
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: Some(ToolResultContent::Text(content.into())),
            is_error: None,
            cache_control: None,
        }
    }

    /// Create a tool result with error.
    pub fn tool_result_error(tool_use_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: Some(ToolResultContent::Text(error.into())),
            is_error: Some(true),
            cache_control: None,
        }
    }
}
