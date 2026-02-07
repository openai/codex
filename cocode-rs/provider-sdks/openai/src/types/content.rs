//! Content types for input and output blocks.

use serde::Deserialize;
use serde::Serialize;

/// Image media types supported by OpenAI.
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

/// Image detail level for vision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    /// Low resolution processing.
    Low,
    /// High resolution processing.
    High,
    /// Auto-select based on image size.
    #[default]
    Auto,
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
        /// Detail level for image processing.
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
    },
    /// File ID reference.
    FileId {
        /// File ID from OpenAI Files API.
        file_id: String,
        /// Detail level for image processing.
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
    },
}

/// Input content blocks for requests (Responses API format).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputContentBlock {
    /// Text content.
    InputText {
        /// The text content.
        text: String,
    },
    /// Image content.
    InputImage {
        /// Image source (base64, URL, or file ID).
        #[serde(flatten)]
        source: ImageSource,
    },
    /// Audio content.
    InputAudio {
        /// Base64-encoded audio data.
        data: String,
        /// Audio format.
        format: AudioFormat,
    },
    /// File reference.
    InputFile {
        /// File ID from OpenAI Files API.
        file_id: String,
        /// Optional file name.
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
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
    /// Computer call output (screenshot/result).
    #[serde(rename = "computer_call_output")]
    ComputerCallOutput {
        /// ID of the computer call this is responding to.
        call_id: String,
        /// Output type.
        output: ComputerCallOutputData,
        /// Acknowledged safety checks.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        acknowledged_safety_checks: Vec<String>,
    },
    /// File search call output.
    #[serde(rename = "file_search_call_output")]
    FileSearchCallOutput {
        /// ID of the file search call this is responding to.
        call_id: String,
        /// Output (results fed back).
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    /// Web search call output.
    #[serde(rename = "web_search_call_output")]
    WebSearchCallOutput {
        /// ID of the web search call this is responding to.
        call_id: String,
        /// Output (results fed back).
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    /// Code interpreter call output.
    #[serde(rename = "code_interpreter_call_output")]
    CodeInterpreterCallOutput {
        /// ID of the code interpreter call this is responding to.
        call_id: String,
        /// Output (execution results fed back).
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    /// Local shell call output.
    #[serde(rename = "local_shell_call_output")]
    LocalShellCallOutput {
        /// ID of the shell call this is responding to.
        call_id: String,
        /// Output (command results fed back).
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    /// MCP call output.
    #[serde(rename = "mcp_call_output")]
    McpCallOutput {
        /// ID of the MCP call this is responding to.
        call_id: String,
        /// Output (tool results fed back).
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        /// Error if any.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Apply patch call output.
    #[serde(rename = "apply_patch_call_output")]
    ApplyPatchCallOutput {
        /// ID of the apply patch call this is responding to.
        call_id: String,
        /// Output (patch application result).
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    /// Custom tool call output.
    #[serde(rename = "custom_tool_call_output")]
    CustomToolCallOutput {
        /// ID of the custom tool call this is responding to.
        call_id: String,
        /// Output from the custom tool.
        output: String,
        /// Unique ID (optional).
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    /// Item reference (reference previous conversation items by ID).
    ItemReference {
        /// ID of the item to reference.
        id: String,
    },
}

/// Audio format for input audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    /// WAV format.
    Wav,
    /// MP3 format.
    Mp3,
    /// FLAC format.
    Flac,
    /// OGG format.
    Ogg,
    /// PCM format (raw audio).
    Pcm16,
}

/// Computer call output data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ComputerCallOutputData {
    /// Screenshot output.
    Screenshot {
        /// Base64-encoded image data.
        #[serde(skip_serializing_if = "Option::is_none")]
        image_data: Option<String>,
        /// Image URL.
        #[serde(skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
    },
    /// Action output.
    Action {
        /// Action result.
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
    },
}

impl InputContentBlock {
    /// Create a text content block.
    pub fn text(text: impl Into<String>) -> Self {
        Self::InputText { text: text.into() }
    }

    /// Create an image content block from base64 data.
    pub fn image_base64(data: impl Into<String>, media_type: ImageMediaType) -> Self {
        Self::InputImage {
            source: ImageSource::Base64 {
                data: data.into(),
                media_type,
            },
        }
    }

    /// Create an image content block from a URL.
    pub fn image_url(url: impl Into<String>) -> Self {
        Self::InputImage {
            source: ImageSource::Url {
                url: url.into(),
                detail: None,
            },
        }
    }

    /// Create an image content block from a URL with detail level.
    pub fn image_url_with_detail(url: impl Into<String>, detail: ImageDetail) -> Self {
        Self::InputImage {
            source: ImageSource::Url {
                url: url.into(),
                detail: Some(detail),
            },
        }
    }

    /// Create an image content block from a file ID.
    pub fn image_file(file_id: impl Into<String>) -> Self {
        Self::InputImage {
            source: ImageSource::FileId {
                file_id: file_id.into(),
                detail: None,
            },
        }
    }

    /// Create an image content block from a file ID with detail level.
    pub fn image_file_with_detail(file_id: impl Into<String>, detail: ImageDetail) -> Self {
        Self::InputImage {
            source: ImageSource::FileId {
                file_id: file_id.into(),
                detail: Some(detail),
            },
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

    /// Create an audio content block.
    pub fn audio(data: impl Into<String>, format: AudioFormat) -> Self {
        Self::InputAudio {
            data: data.into(),
            format,
        }
    }

    /// Create a file reference content block.
    pub fn file(file_id: impl Into<String>) -> Self {
        Self::InputFile {
            file_id: file_id.into(),
            filename: None,
        }
    }

    /// Create a file reference content block with filename.
    pub fn file_with_name(file_id: impl Into<String>, filename: impl Into<String>) -> Self {
        Self::InputFile {
            file_id: file_id.into(),
            filename: Some(filename.into()),
        }
    }

    /// Create a computer call output content block with screenshot.
    pub fn computer_call_output_screenshot(
        call_id: impl Into<String>,
        image_data: Option<String>,
        image_url: Option<String>,
    ) -> Self {
        Self::ComputerCallOutput {
            call_id: call_id.into(),
            output: ComputerCallOutputData::Screenshot {
                image_data,
                image_url,
            },
            acknowledged_safety_checks: vec![],
        }
    }

    /// Create a computer call output content block with action result.
    pub fn computer_call_output_action(call_id: impl Into<String>, result: Option<String>) -> Self {
        Self::ComputerCallOutput {
            call_id: call_id.into(),
            output: ComputerCallOutputData::Action { result },
            acknowledged_safety_checks: vec![],
        }
    }

    /// Create a file search call output content block.
    pub fn file_search_call_output(call_id: impl Into<String>, output: Option<String>) -> Self {
        Self::FileSearchCallOutput {
            call_id: call_id.into(),
            output,
        }
    }

    /// Create a web search call output content block.
    pub fn web_search_call_output(call_id: impl Into<String>, output: Option<String>) -> Self {
        Self::WebSearchCallOutput {
            call_id: call_id.into(),
            output,
        }
    }

    /// Create a code interpreter call output content block.
    pub fn code_interpreter_call_output(
        call_id: impl Into<String>,
        output: Option<String>,
    ) -> Self {
        Self::CodeInterpreterCallOutput {
            call_id: call_id.into(),
            output,
        }
    }

    /// Create a local shell call output content block.
    pub fn local_shell_call_output(call_id: impl Into<String>, output: Option<String>) -> Self {
        Self::LocalShellCallOutput {
            call_id: call_id.into(),
            output,
        }
    }

    /// Create an MCP call output content block.
    pub fn mcp_call_output(
        call_id: impl Into<String>,
        output: Option<String>,
        error: Option<String>,
    ) -> Self {
        Self::McpCallOutput {
            call_id: call_id.into(),
            output,
            error,
        }
    }

    /// Create an apply patch call output content block.
    pub fn apply_patch_call_output(call_id: impl Into<String>, output: Option<String>) -> Self {
        Self::ApplyPatchCallOutput {
            call_id: call_id.into(),
            output,
        }
    }

    /// Create a custom tool call output content block.
    pub fn custom_tool_call_output(call_id: impl Into<String>, output: impl Into<String>) -> Self {
        Self::CustomToolCallOutput {
            call_id: call_id.into(),
            output: output.into(),
            id: None,
        }
    }

    /// Create an item reference content block.
    pub fn item_reference(id: impl Into<String>) -> Self {
        Self::ItemReference { id: id.into() }
    }
}

// ============================================================================
// Logprobs types
// ============================================================================

/// Token-level log probability information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenLogprob {
    /// The token string.
    pub token: String,
    /// Log probability of this token.
    pub logprob: f64,
    /// Byte representation of the token (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<i32>>,
}

/// Alternative token with log probability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopLogprob {
    /// The token string.
    pub token: String,
    /// Log probability of this token.
    pub logprob: f64,
    /// Byte representation of the token (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<i32>>,
}

/// Log probability information for a token position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogprobContent {
    /// The token at this position.
    pub token: String,
    /// Log probability of the chosen token.
    pub logprob: f64,
    /// Byte representation of the token (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<i32>>,
    /// Top alternative tokens with their log probabilities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_logprobs: Vec<TopLogprob>,
}

/// Full logprobs data for output text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logprobs {
    /// Token-level log probability information.
    #[serde(default)]
    pub content: Vec<LogprobContent>,
}

impl Logprobs {
    /// Get the total log probability sum.
    pub fn total_logprob(&self) -> f64 {
        self.content.iter().map(|c| c.logprob).sum()
    }

    /// Get tokens as strings.
    pub fn tokens(&self) -> Vec<&str> {
        self.content.iter().map(|c| c.token.as_str()).collect()
    }
}

// ============================================================================
// Annotation types
// ============================================================================

/// Annotation for output text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Annotation {
    /// File citation.
    FileCitation {
        /// File ID referenced.
        file_id: String,
        /// Index in the text.
        #[serde(skip_serializing_if = "Option::is_none")]
        index: Option<i32>,
    },
    /// URL citation.
    UrlCitation {
        /// URL referenced.
        url: String,
        /// Title of the page.
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// Start index in text.
        #[serde(skip_serializing_if = "Option::is_none")]
        start_index: Option<i32>,
        /// End index in text.
        #[serde(skip_serializing_if = "Option::is_none")]
        end_index: Option<i32>,
    },
}

/// Output content blocks from responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputContentBlock {
    /// Text content.
    OutputText {
        /// The text content.
        text: String,
        /// Annotations in the text.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        annotations: Vec<Annotation>,
        /// Log probability information (when include: ["message.output_text.logprobs"]).
        #[serde(skip_serializing_if = "Option::is_none")]
        logprobs: Option<Logprobs>,
    },
    /// Refusal content (content was refused).
    Refusal {
        /// The refusal message.
        refusal: String,
    },
}

impl OutputContentBlock {
    /// Get the text content if this is a text block.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::OutputText { text, .. } => Some(text),
            _ => None,
        }
    }

    /// Get the refusal message if this is a refusal block.
    pub fn as_refusal(&self) -> Option<&str> {
        match self {
            Self::Refusal { refusal } => Some(refusal),
            _ => None,
        }
    }

    /// Get the logprobs if present.
    pub fn as_logprobs(&self) -> Option<&Logprobs> {
        match self {
            Self::OutputText { logprobs, .. } => logprobs.as_ref(),
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
        assert!(json.contains(r#""type":"input_text""#));
        assert!(json.contains(r#""text":"Hello""#));
    }

    #[test]
    fn test_input_content_image_base64() {
        let block = InputContentBlock::image_base64("data123", ImageMediaType::Png);
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"input_image""#));
        assert!(json.contains(r#""data":"data123""#));
        assert!(json.contains(r#""media_type":"image/png""#));
    }

    #[test]
    fn test_input_content_image_url() {
        let block = InputContentBlock::image_url("https://example.com/image.png");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"input_image""#));
        assert!(json.contains(r#""url":"https://example.com/image.png""#));
    }

    #[test]
    fn test_input_content_image_url_with_detail() {
        let block = InputContentBlock::image_url_with_detail(
            "https://example.com/image.png",
            ImageDetail::High,
        );
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""detail":"high""#));
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
        let text = OutputContentBlock::OutputText {
            text: "Hello".to_string(),
            annotations: vec![],
            logprobs: None,
        };
        assert_eq!(text.as_text(), Some("Hello"));
        assert!(text.as_refusal().is_none());

        let refusal = OutputContentBlock::Refusal {
            refusal: "Cannot do that".to_string(),
        };
        assert!(refusal.as_text().is_none());
        assert_eq!(refusal.as_refusal(), Some("Cannot do that"));
    }

    #[test]
    fn test_input_audio() {
        let block = InputContentBlock::audio("base64data", AudioFormat::Mp3);
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"input_audio""#));
        assert!(json.contains(r#""data":"base64data""#));
        assert!(json.contains(r#""format":"mp3""#));
    }

    #[test]
    fn test_input_file() {
        let block = InputContentBlock::file("file-123");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"input_file""#));
        assert!(json.contains(r#""file_id":"file-123""#));

        let block = InputContentBlock::file_with_name("file-123", "document.pdf");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""filename":"document.pdf""#));
    }

    #[test]
    fn test_computer_call_output() {
        let block = InputContentBlock::computer_call_output_screenshot(
            "call-1",
            Some("base64".into()),
            None,
        );
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"computer_call_output""#));
        assert!(json.contains(r#""call_id":"call-1""#));

        let block =
            InputContentBlock::computer_call_output_action("call-2", Some("success".into()));
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"computer_call_output""#));
    }

    #[test]
    fn test_tool_call_outputs() {
        let block = InputContentBlock::file_search_call_output("call-1", Some("results".into()));
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"file_search_call_output""#));

        let block = InputContentBlock::web_search_call_output("call-2", Some("results".into()));
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"web_search_call_output""#));

        let block =
            InputContentBlock::code_interpreter_call_output("call-3", Some("output".into()));
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"code_interpreter_call_output""#));

        let block = InputContentBlock::local_shell_call_output("call-4", Some("output".into()));
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"local_shell_call_output""#));

        let block = InputContentBlock::mcp_call_output("call-5", Some("output".into()), None);
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"mcp_call_output""#));

        let block = InputContentBlock::apply_patch_call_output("call-6", Some("patched".into()));
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"apply_patch_call_output""#));
    }

    #[test]
    fn test_item_reference() {
        let block = InputContentBlock::item_reference("item-123");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"item_reference""#));
        assert!(json.contains(r#""id":"item-123""#));
    }

    #[test]
    fn test_logprobs() {
        let logprobs = Logprobs {
            content: vec![
                LogprobContent {
                    token: "Hello".to_string(),
                    logprob: -0.5,
                    bytes: None,
                    top_logprobs: vec![TopLogprob {
                        token: "Hi".to_string(),
                        logprob: -1.0,
                        bytes: None,
                    }],
                },
                LogprobContent {
                    token: " world".to_string(),
                    logprob: -0.3,
                    bytes: None,
                    top_logprobs: vec![],
                },
            ],
        };

        assert_eq!(logprobs.tokens(), vec!["Hello", " world"]);
        assert!((logprobs.total_logprob() - (-0.8)).abs() < 0.001);

        let json = serde_json::to_string(&logprobs).unwrap();
        assert!(json.contains(r#""token":"Hello""#));
        assert!(json.contains(r#""logprob":-0.5"#));
    }

    #[test]
    fn test_custom_tool_call_output_serialization() {
        let block = InputContentBlock::custom_tool_call_output("call-custom-1", "patch applied");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"custom_tool_call_output""#));
        assert!(json.contains(r#""call_id":"call-custom-1""#));
        assert!(json.contains(r#""output":"patch applied""#));
        assert!(!json.contains(r#""id""#)); // id is None, should be skipped

        // Roundtrip
        let parsed: InputContentBlock = serde_json::from_str(&json).unwrap();
        if let InputContentBlock::CustomToolCallOutput {
            call_id,
            output,
            id,
        } = parsed
        {
            assert_eq!(call_id, "call-custom-1");
            assert_eq!(output, "patch applied");
            assert!(id.is_none());
        } else {
            panic!("Expected CustomToolCallOutput variant");
        }
    }

    #[test]
    fn test_output_text_with_logprobs() {
        let block = OutputContentBlock::OutputText {
            text: "Hello".to_string(),
            annotations: vec![],
            logprobs: Some(Logprobs {
                content: vec![LogprobContent {
                    token: "Hello".to_string(),
                    logprob: -0.5,
                    bytes: None,
                    top_logprobs: vec![],
                }],
            }),
        };
        assert!(block.as_logprobs().is_some());
        assert_eq!(block.as_logprobs().unwrap().tokens(), vec!["Hello"]);
    }
}
