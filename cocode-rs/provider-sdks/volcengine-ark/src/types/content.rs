//! Content types for input and output blocks.

use serde::Deserialize;
use serde::Serialize;

/// Image media types supported by Volcengine Ark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageMediaType {
    /// JPEG image.
    #[serde(rename = "image/jpeg")]
    Jpeg,
    /// PNG image.
    #[serde(rename = "image/png")]
    Png,
    /// GIF image.
    #[serde(rename = "image/gif")]
    Gif,
    /// WebP image.
    #[serde(rename = "image/webp")]
    Webp,
}

/// Image source - base64 encoded or URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// Base64 encoded image data.
    Base64 {
        /// Base64 encoded image data.
        data: String,
        /// Media type of the image.
        media_type: ImageMediaType,
    },
    /// URL to the image.
    Url {
        /// URL to the image.
        url: String,
    },
}

/// Input content blocks for requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputContentBlock {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
    /// Image content.
    Image {
        /// Image source (base64 or URL).
        source: ImageSource,
    },
    /// Function call output (tool result).
    FunctionCallOutput {
        /// ID of the function call this is responding to.
        call_id: String,
        /// Output of the function call.
        output: String,
        /// Whether this is an error result.
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

impl InputContentBlock {
    /// Create a text content block.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create an image content block from base64 data.
    pub fn image_base64(data: impl Into<String>, media_type: ImageMediaType) -> Self {
        Self::Image {
            source: ImageSource::Base64 {
                data: data.into(),
                media_type,
            },
        }
    }

    /// Create an image content block from a URL.
    pub fn image_url(url: impl Into<String>) -> Self {
        Self::Image {
            source: ImageSource::Url { url: url.into() },
        }
    }

    /// Create a function call output content block.
    pub fn function_call_output(
        call_id: impl Into<String>,
        output: impl Into<String>,
        is_error: Option<bool>,
    ) -> Self {
        Self::FunctionCallOutput {
            call_id: call_id.into(),
            output: output.into(),
            is_error,
        }
    }
}

/// Output content blocks from responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputContentBlock {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
    /// Function call.
    FunctionCall {
        /// Unique ID for this function call.
        id: String,
        /// Name of the function to call.
        name: String,
        /// Arguments as a JSON value.
        arguments: serde_json::Value,
    },
    /// Thinking content (extended reasoning).
    Thinking {
        /// The thinking content.
        thinking: String,
        /// Verification signature.
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
}

impl OutputContentBlock {
    /// Get the text content if this is a text block.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            _ => None,
        }
    }

    /// Get the function call details if this is a function call block.
    pub fn as_function_call(&self) -> Option<(&str, &str, &serde_json::Value)> {
        match self {
            Self::FunctionCall {
                id,
                name,
                arguments,
            } => Some((id, name, arguments)),
            _ => None,
        }
    }

    /// Get the thinking content if this is a thinking block.
    pub fn as_thinking(&self) -> Option<&str> {
        match self {
            Self::Thinking { thinking, .. } => Some(thinking),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_content_text() {
        let block = InputContentBlock::text("Hello");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""text":"Hello""#));
    }

    #[test]
    fn test_input_content_image_base64() {
        let block = InputContentBlock::image_base64("data123", ImageMediaType::Png);
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"image""#));
        assert!(json.contains(r#""data":"data123""#));
        assert!(json.contains(r#""media_type":"image/png""#));
    }

    #[test]
    fn test_input_content_image_url() {
        let block = InputContentBlock::image_url("https://example.com/image.png");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"image""#));
        assert!(json.contains(r#""url":"https://example.com/image.png""#));
    }

    #[test]
    fn test_input_content_function_output() {
        let block = InputContentBlock::function_call_output("call-1", r#"{"result": 42}"#, None);
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"function_call_output""#));
        assert!(json.contains(r#""call_id":"call-1""#));
    }

    #[test]
    fn test_output_content_block_helpers() {
        let text = OutputContentBlock::Text {
            text: "Hello".to_string(),
        };
        assert_eq!(text.as_text(), Some("Hello"));
        assert!(text.as_function_call().is_none());

        let func = OutputContentBlock::FunctionCall {
            id: "call-1".to_string(),
            name: "test".to_string(),
            arguments: serde_json::json!({}),
        };
        assert!(func.as_text().is_none());
        assert!(func.as_function_call().is_some());
    }
}
