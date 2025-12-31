//! Core types for Google Generative AI (Gemini) API.
//!
//! This module contains all the data structures used for request/response
//! communication with the Gemini API.

use base64::Engine;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::de::Error as DeError;
use std::collections::HashMap;

// ============================================================================
// Base64 Serde Helpers (for bytes fields like thought_signature)
// ============================================================================

fn serialize_bytes_base64<S>(data: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match data {
        Some(bytes) => {
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            serializer.serialize_some(&encoded)
        }
        None => serializer.serialize_none(),
    }
}

fn deserialize_bytes_base64<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) => base64::engine::general_purpose::STANDARD
            .decode(&s)
            .map(Some)
            .map_err(|e| DeError::custom(format!("base64 decode error: {e}"))),
        None => Ok(None),
    }
}

// ============================================================================
// Enums
// ============================================================================

/// The reason why the model stopped generating tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinishReason {
    #[default]
    FinishReasonUnspecified,
    Stop,
    MaxTokens,
    Safety,
    Recitation,
    Language,
    Other,
    Blocklist,
    ProhibitedContent,
    Spii,
    MalformedFunctionCall,
    ImageSafety,
    UnexpectedToolCall,
}

/// Harm category for safety ratings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HarmCategory {
    HarmCategoryUnspecified,
    HarmCategoryHarassment,
    HarmCategoryHateSpeech,
    HarmCategorySexuallyExplicit,
    HarmCategoryDangerousContent,
    HarmCategoryCivicIntegrity,
}

/// Harm probability levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HarmProbability {
    HarmProbabilityUnspecified,
    Negligible,
    Low,
    Medium,
    High,
}

/// Harm block threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HarmBlockThreshold {
    HarmBlockThresholdUnspecified,
    BlockLowAndAbove,
    BlockMediumAndAbove,
    BlockOnlyHigh,
    BlockNone,
    Off,
}

/// The reason why the prompt was blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlockedReason {
    BlockedReasonUnspecified,
    Safety,
    Other,
    Blocklist,
    ProhibitedContent,
    ImageSafety,
}

/// Function calling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FunctionCallingMode {
    #[default]
    ModeUnspecified,
    Auto,
    Any,
    None,
    /// Validated function calls with constrained decoding.
    Validated,
}

/// JSON Schema type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SchemaType {
    TypeUnspecified,
    String,
    Number,
    Integer,
    Boolean,
    Array,
    Object,
    Null,
}

/// The level of thinking tokens that the model should generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThinkingLevel {
    #[default]
    ThinkingLevelUnspecified,
    Low,
    High,
}

/// Programming language for code execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Language {
    #[default]
    LanguageUnspecified,
    Python,
}

/// Outcome of the code execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Outcome {
    #[default]
    OutcomeUnspecified,
    OutcomeOk,
    OutcomeFailed,
    OutcomeDeadlineExceeded,
}

/// Function calling behavior (blocking vs non-blocking).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Behavior {
    #[default]
    BehaviorUnspecified,
    Blocking,
    NonBlocking,
}

/// Specifies how the function response should be scheduled in the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FunctionResponseScheduling {
    #[default]
    SchedulingUnspecified,
    /// Only add the result to the conversation context, do not trigger generation.
    Silent,
    /// Add the result and prompt generation without interrupting ongoing generation.
    WhenIdle,
    /// Add the result, interrupt ongoing generation and prompt to generate output.
    Interrupt,
}

/// Media modality for token counting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaModality {
    #[default]
    ModalityUnspecified,
    Text,
    Image,
    Audio,
    Video,
    Document,
}

/// Media resolution for parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PartMediaResolution {
    #[default]
    MediaResolutionUnspecified,
    MediaResolutionLow,
    MediaResolutionMedium,
    MediaResolutionHigh,
}

// ============================================================================
// Content Parts
// ============================================================================

/// Content blob (inline binary data).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Blob {
    /// Raw bytes, base64 encoded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,

    /// The IANA standard MIME type of the source data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

impl Blob {
    pub fn new(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            data: Some(data.into()),
            mime_type: Some(mime_type.into()),
        }
    }

    /// Create a Blob from raw bytes (will be base64 encoded).
    pub fn from_bytes(data: &[u8], mime_type: impl Into<String>) -> Self {
        use base64::Engine;
        Self {
            data: Some(base64::engine::general_purpose::STANDARD.encode(data)),
            mime_type: Some(mime_type.into()),
        }
    }
}

/// URI based data reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileData {
    /// URI of the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_uri: Option<String>,

    /// The IANA standard MIME type of the source data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

impl FileData {
    pub fn new(file_uri: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            file_uri: Some(file_uri.into()),
            mime_type: Some(mime_type.into()),
        }
    }
}

/// Partial argument value of the function call (for streaming).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PartialArg {
    /// Represents a null value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub null_value: Option<String>,

    /// Number value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_value: Option<f64>,

    /// String value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub string_value: Option<String>,

    /// Boolean value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bool_value: Option<bool>,

    /// JSON path for the partial argument.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_path: Option<String>,

    /// Whether this is not the last part of the same json_path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub will_continue: Option<bool>,
}

/// A function call predicted by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCall {
    /// The unique id of the function call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// The name of the function to call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The function parameters and values in JSON object format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,

    /// Partial argument values (for streaming function call arguments).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_args: Option<Vec<PartialArg>>,

    /// Whether this is not the last part of the FunctionCall.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub will_continue: Option<bool>,
}

impl FunctionCall {
    pub fn new(name: impl Into<String>, args: serde_json::Value) -> Self {
        Self {
            id: None,
            name: Some(name.into()),
            args: Some(args),
            partial_args: None,
            will_continue: None,
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

/// Raw media bytes for function response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FunctionResponseBlob {
    /// The IANA standard MIME type of the source data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,

    /// Inline media bytes (base64 encoded in JSON).
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_bytes_base64",
        deserialize_with = "deserialize_bytes_base64"
    )]
    pub data: Option<Vec<u8>>,

    /// Display name of the blob.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// URI based data for function response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FunctionResponseFileData {
    /// URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_uri: Option<String>,

    /// The IANA standard MIME type of the source data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,

    /// Display name of the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// A datatype containing media that is part of a FunctionResponse message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FunctionResponsePart {
    /// Inline media bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<FunctionResponseBlob>,

    /// URI based data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_data: Option<FunctionResponseFileData>,
}

/// The result of a function call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FunctionResponse {
    /// The id of the function call this response is for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// The name of the function.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The function response in JSON object format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<serde_json::Value>,

    /// Whether more responses are coming for this function call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub will_continue: Option<bool>,

    /// Scheduling for the response in the conversation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduling: Option<FunctionResponseScheduling>,

    /// Multi-part function response data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parts: Option<Vec<FunctionResponsePart>>,
}

impl FunctionResponse {
    pub fn new(name: impl Into<String>, response: serde_json::Value) -> Self {
        Self {
            id: None,
            name: Some(name.into()),
            response: Some(response),
            will_continue: None,
            scheduling: None,
            parts: None,
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

/// Code authored and executed by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableCode {
    /// Programming language of the code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,

    /// The code to be executed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Result of executing the code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CodeExecutionResult {
    /// Outcome of the code execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,

    /// Output from the code execution (stdout/stderr).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// Video metadata for video parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VideoMetadata {
    /// Start offset (duration string like "1.5s").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_offset: Option<String>,

    /// End offset (duration string like "10.5s").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<String>,
}

/// A datatype containing media content.
///
/// Exactly one field should be set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    /// Text content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Inline bytes data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<Blob>,

    /// URI based data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_data: Option<FileData>,

    /// A predicted function call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,

    /// The result of a function call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<FunctionResponse>,

    /// Indicates if the part is thought/reasoning from the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,

    /// Opaque signature for reusing thought in subsequent requests.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_bytes_base64",
        deserialize_with = "deserialize_bytes_base64"
    )]
    pub thought_signature: Option<Vec<u8>>,

    /// Code authored and executed by the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_code: Option<ExecutableCode>,

    /// Result of code execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_execution_result: Option<CodeExecutionResult>,

    /// Video metadata for video parts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_metadata: Option<VideoMetadata>,

    /// Media resolution for the input media.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_resolution: Option<PartMediaResolution>,
}

impl Part {
    /// Create a text part.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            ..Default::default()
        }
    }

    /// Create an inline data part from bytes.
    pub fn from_bytes(data: &[u8], mime_type: impl Into<String>) -> Self {
        Self {
            inline_data: Some(Blob::from_bytes(data, mime_type)),
            ..Default::default()
        }
    }

    /// Create a file data part from URI.
    pub fn from_uri(file_uri: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            file_data: Some(FileData::new(file_uri, mime_type)),
            ..Default::default()
        }
    }

    /// Create a function call part.
    pub fn function_call(name: impl Into<String>, args: serde_json::Value) -> Self {
        Self {
            function_call: Some(FunctionCall::new(name, args)),
            ..Default::default()
        }
    }

    /// Create a function response part.
    pub fn function_response(name: impl Into<String>, response: serde_json::Value) -> Self {
        Self {
            function_response: Some(FunctionResponse::new(name, response)),
            ..Default::default()
        }
    }

    /// Create a thought part with a signature (for passing thoughts to subsequent requests).
    pub fn with_thought_signature(signature: impl Into<Vec<u8>>) -> Self {
        Self {
            thought: Some(true),
            thought_signature: Some(signature.into()),
            ..Default::default()
        }
    }

    /// Create a thought part with a base64-encoded signature string.
    pub fn with_thought_signature_base64(signature: &str) -> Result<Self, base64::DecodeError> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(signature)?;
        Ok(Self {
            thought: Some(true),
            thought_signature: Some(bytes),
            ..Default::default()
        })
    }

    /// Check if this part is a thought/reasoning part.
    pub fn is_thought(&self) -> bool {
        self.thought == Some(true)
    }
}

impl From<&str> for Part {
    fn from(text: &str) -> Self {
        Part::text(text)
    }
}

impl From<String> for Part {
    fn from(text: String) -> Self {
        Part::text(text)
    }
}

/// Contains the multi-part content of a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    /// List of parts that constitute a single message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parts: Option<Vec<Part>>,

    /// The producer of the content. Must be either 'user' or 'model'.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

impl Content {
    /// Create a user content with text.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            parts: Some(vec![Part::text(text)]),
            role: Some("user".to_string()),
        }
    }

    /// Create a model content with text.
    pub fn model(text: impl Into<String>) -> Self {
        Self {
            parts: Some(vec![Part::text(text)]),
            role: Some("model".to_string()),
        }
    }

    /// Create content with multiple parts.
    pub fn with_parts(role: impl Into<String>, parts: Vec<Part>) -> Self {
        Self {
            parts: Some(parts),
            role: Some(role.into()),
        }
    }

    /// Create user content with image (bytes).
    pub fn user_with_image(
        text: impl Into<String>,
        image_data: &[u8],
        mime_type: impl Into<String>,
    ) -> Self {
        Self {
            parts: Some(vec![
                Part::text(text),
                Part::from_bytes(image_data, mime_type),
            ]),
            role: Some("user".to_string()),
        }
    }

    /// Create user content with image URI.
    pub fn user_with_image_uri(
        text: impl Into<String>,
        file_uri: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Self {
        Self {
            parts: Some(vec![Part::text(text), Part::from_uri(file_uri, mime_type)]),
            role: Some("user".to_string()),
        }
    }
}

// ============================================================================
// Tools
// ============================================================================

/// JSON Schema definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Schema {
    /// The type of the data.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub schema_type: Option<SchemaType>,

    /// Description of the schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Enum values for string types.
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,

    /// Properties for object types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Schema>>,

    /// Required property names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,

    /// Items schema for array types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<Schema>>,

    /// Default value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,

    /// Format hint (e.g., "int32", "int64", "float", "email").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    /// Minimum string length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<i32>,

    /// Maximum string length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<i32>,

    /// Minimum number value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,

    /// Maximum number value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,

    /// Minimum array items.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_items: Option<i32>,

    /// Maximum array items.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<i32>,

    /// Regex pattern for string validation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,

    /// Whether the value can be null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nullable: Option<bool>,

    /// Union types (any of these schemas).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub any_of: Option<Vec<Schema>>,

    /// Schema definitions for $ref.
    #[serde(rename = "$defs", skip_serializing_if = "Option::is_none")]
    pub defs: Option<HashMap<String, Schema>>,

    /// Reference to a schema definition.
    #[serde(rename = "$ref", skip_serializing_if = "Option::is_none")]
    pub schema_ref: Option<String>,

    /// Whether additional properties are allowed (bool or schema).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_properties: Option<serde_json::Value>,

    /// Preferred order of properties in the output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property_ordering: Option<Vec<String>>,
}

impl Schema {
    pub fn string() -> Self {
        Self {
            schema_type: Some(SchemaType::String),
            ..Default::default()
        }
    }

    pub fn number() -> Self {
        Self {
            schema_type: Some(SchemaType::Number),
            ..Default::default()
        }
    }

    pub fn integer() -> Self {
        Self {
            schema_type: Some(SchemaType::Integer),
            ..Default::default()
        }
    }

    pub fn boolean() -> Self {
        Self {
            schema_type: Some(SchemaType::Boolean),
            ..Default::default()
        }
    }

    pub fn array(items: Schema) -> Self {
        Self {
            schema_type: Some(SchemaType::Array),
            items: Some(Box::new(items)),
            ..Default::default()
        }
    }

    pub fn object(properties: HashMap<String, Schema>) -> Self {
        Self {
            schema_type: Some(SchemaType::Object),
            properties: Some(properties),
            ..Default::default()
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_required(mut self, required: Vec<String>) -> Self {
        self.required = Some(required);
        self
    }
}

/// Defines a function that the model can generate JSON inputs for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FunctionDeclaration {
    /// The name of the function to call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Description and purpose of the function.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The parameters to this function in OpenAPI Schema format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Schema>,

    /// Alternative: parameters in JSON Schema format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters_json_schema: Option<serde_json::Value>,

    /// Return type schema in OpenAPI Schema format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<Schema>,

    /// Alternative: return type in JSON Schema format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_json_schema: Option<serde_json::Value>,

    /// Function behavior: BLOCKING (default) or NON_BLOCKING.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behavior: Option<Behavior>,
}

impl FunctionDeclaration {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Default::default()
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_parameters(mut self, parameters: Schema) -> Self {
        self.parameters = Some(parameters);
        self
    }
}

/// GoogleSearch tool configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GoogleSearch {}

/// Code execution tool configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CodeExecution {}

/// RAG filter for retrieval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RagFilter {
    /// Metadata filter string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_filter: Option<String>,
}

/// RAG retrieval configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RagRetrievalConfig {
    /// Number of top results to return.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,

    /// Filter for retrieval.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<RagFilter>,
}

/// Vertex RAG store configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VertexRagStore {
    /// RAG corpora resource names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rag_corpora: Option<Vec<String>>,

    /// RAG retrieval configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rag_retrieval_config: Option<RagRetrievalConfig>,
}

/// Vertex AI Search configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VertexAISearch {
    /// Datastore resource name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datastore: Option<String>,
}

/// Retrieval tool configuration (Vertex AI only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Retrieval {
    /// Vertex AI Search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertex_ai_search: Option<VertexAISearch>,

    /// Vertex RAG store.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertex_rag_store: Option<VertexRagStore>,
}

/// Tool details that the model may use to generate a response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    /// List of function declarations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_declarations: Option<Vec<FunctionDeclaration>>,

    /// Google Search tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google_search: Option<GoogleSearch>,

    /// Code execution tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_execution: Option<CodeExecution>,

    /// Retrieval tool (Vertex AI only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieval: Option<Retrieval>,
}

impl Tool {
    /// Create a tool with function declarations.
    pub fn functions(declarations: Vec<FunctionDeclaration>) -> Self {
        Self {
            function_declarations: Some(declarations),
            ..Default::default()
        }
    }

    /// Create a Google Search tool.
    pub fn google_search() -> Self {
        Self {
            google_search: Some(GoogleSearch {}),
            ..Default::default()
        }
    }

    /// Create a code execution tool.
    pub fn code_execution() -> Self {
        Self {
            code_execution: Some(CodeExecution {}),
            ..Default::default()
        }
    }
}

/// Function calling configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCallingConfig {
    /// Function calling mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<FunctionCallingMode>,

    /// Function names to call (only when mode is ANY or VALIDATED).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_function_names: Option<Vec<String>>,

    /// When true, function call arguments are streamed out in partial_args.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_function_call_arguments: Option<bool>,
}

/// Tool configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ToolConfig {
    /// Function calling config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_calling_config: Option<FunctionCallingConfig>,
}

// ============================================================================
// Safety
// ============================================================================

/// Safety setting for a harm category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetySetting {
    /// The harm category.
    pub category: HarmCategory,

    /// The harm block threshold.
    pub threshold: HarmBlockThreshold,
}

/// Safety rating for generated content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyRating {
    /// The harm category.
    pub category: HarmCategory,

    /// The harm probability.
    pub probability: HarmProbability,

    /// Whether the content was blocked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<bool>,
}

// ============================================================================
// Generation Config
// ============================================================================

/// Thinking configuration for models that support it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingConfig {
    /// Whether to include thoughts in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,

    /// Budget of thinking tokens. 0=DISABLED, -1=AUTO.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,

    /// The level of thoughts tokens that the model should generate (LOW/HIGH).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<ThinkingLevel>,
}

impl ThinkingConfig {
    /// Create a config that includes thoughts in the response.
    pub fn with_thoughts() -> Self {
        Self {
            include_thoughts: Some(true),
            thinking_budget: None,
            thinking_level: None,
        }
    }

    /// Create a config with a thinking budget.
    pub fn with_budget(budget: i32) -> Self {
        Self {
            include_thoughts: Some(true),
            thinking_budget: Some(budget),
            thinking_level: None,
        }
    }

    /// Create a config with a thinking level (LOW/HIGH).
    pub fn with_level(level: ThinkingLevel) -> Self {
        Self {
            include_thoughts: Some(true),
            thinking_budget: None,
            thinking_level: Some(level),
        }
    }
}

/// Generation configuration parameters (wire format inside generationConfig).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    /// Temperature for randomness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p for nucleus sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top-k for sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,

    /// Maximum output tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,

    /// Number of candidates to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<i32>,

    /// Stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    /// Whether to return log probabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_logprobs: Option<bool>,

    /// Number of top log probabilities to return.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<i32>,

    /// Response MIME type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,

    /// Response schema for structured output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Schema>,

    /// Presence penalty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// Frequency penalty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Seed for reproducibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,

    /// Response modalities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_modalities: Option<Vec<String>>,

    /// Thinking configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
}

// ============================================================================
// Request / Response
// ============================================================================

/// Configuration for generate content request (user-facing API).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentConfig {
    /// System instruction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<Content>,

    /// Temperature for randomness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p for nucleus sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top-k for sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,

    /// Maximum output tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,

    /// Number of candidates to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<i32>,

    /// Stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    /// Whether to return log probabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_logprobs: Option<bool>,

    /// Number of top log probabilities to return.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<i32>,

    /// Response MIME type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,

    /// Response schema for structured output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Schema>,

    /// Presence penalty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// Frequency penalty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Seed for reproducibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,

    /// Response modalities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_modalities: Option<Vec<String>>,

    /// Safety settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_settings: Option<Vec<SafetySetting>>,

    /// Tools available to the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// Tool configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<ToolConfig>,

    /// Thinking configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,

    /// Cached content resource name for context caching.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_content: Option<String>,

    /// Alternative: response schema in raw JSON Schema format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_json_schema: Option<serde_json::Value>,

    /// Request extensions (headers, params, body) - not serialized.
    #[serde(skip)]
    pub extensions: Option<RequestExtensions>,
}

impl GenerateContentConfig {
    /// Check if any generation parameters are set.
    pub fn has_generation_params(&self) -> bool {
        self.temperature.is_some()
            || self.top_p.is_some()
            || self.top_k.is_some()
            || self.max_output_tokens.is_some()
            || self.candidate_count.is_some()
            || self.stop_sequences.is_some()
            || self.response_logprobs.is_some()
            || self.logprobs.is_some()
            || self.response_mime_type.is_some()
            || self.response_schema.is_some()
            || self.presence_penalty.is_some()
            || self.frequency_penalty.is_some()
            || self.seed.is_some()
            || self.response_modalities.is_some()
            || self.thinking_config.is_some()
    }
}

/// Request body for generateContent API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentRequest {
    /// The content of the conversation.
    pub contents: Vec<Content>,

    /// System instruction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<Content>,

    /// Generation configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,

    /// Safety settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_settings: Option<Vec<SafetySetting>>,

    /// Tools available to the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// Tool configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<ToolConfig>,
}

/// Prompt feedback in response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PromptFeedback {
    /// The reason why the prompt was blocked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_reason: Option<BlockedReason>,

    /// Safety ratings for the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_ratings: Option<Vec<SafetyRating>>,
}

// ============================================================================
// Citation & Grounding Types
// ============================================================================

/// A citation source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CitationSource {
    /// URI of the source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,

    /// Title of the source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// License information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Start index in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_index: Option<i32>,

    /// End index in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_index: Option<i32>,
}

/// Citation metadata for a response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CitationMetadata {
    /// List of citations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<CitationSource>>,
}

/// Web source for grounding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GroundingChunkWeb {
    /// URI of the web source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,

    /// Title of the web source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Retrieved context for grounding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GroundingChunkRetrievedContext {
    /// URI of the retrieved context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,

    /// Title of the retrieved context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// A grounding chunk (web or retrieved context).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GroundingChunk {
    /// Web source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web: Option<GroundingChunkWeb>,

    /// Retrieved context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieved_context: Option<GroundingChunkRetrievedContext>,
}

/// A segment of text in the response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    /// Start index of the segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_index: Option<i32>,

    /// End index of the segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_index: Option<i32>,

    /// Text of the segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Part index in the content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_index: Option<i32>,
}

/// Grounding support for a segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GroundingSupport {
    /// The segment being grounded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment: Option<Segment>,

    /// Indices of grounding chunks that support this segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_chunk_indices: Option<Vec<i32>>,

    /// Confidence scores for each grounding chunk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_scores: Option<Vec<f64>>,
}

/// Search entry point for grounding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchEntryPoint {
    /// Rendered HTML content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendered_content: Option<String>,

    /// SDK blob for the search entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_blob: Option<String>,
}

/// Retrieval metadata for grounding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalMetadata {
    /// Dynamic retrieval score from Google Search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google_search_dynamic_retrieval_score: Option<f64>,
}

/// Grounding metadata for a response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GroundingMetadata {
    /// Grounding chunks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_chunks: Option<Vec<GroundingChunk>>,

    /// Grounding supports linking segments to chunks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_supports: Option<Vec<GroundingSupport>>,

    /// Web search queries used for grounding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search_queries: Option<Vec<String>>,

    /// Search entry point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_entry_point: Option<SearchEntryPoint>,

    /// Retrieval metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieval_metadata: Option<RetrievalMetadata>,
}

// ============================================================================
// Logprobs Types
// ============================================================================

/// A candidate token with its log probability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LogprobsCandidate {
    /// The token string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// Token ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_id: Option<i32>,

    /// Log probability of the token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_probability: Option<f64>,
}

/// Top candidate tokens at a position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TopCandidates {
    /// List of top candidate tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<LogprobsCandidate>>,
}

/// Log probabilities result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LogprobsResult {
    /// Top candidates at each position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_candidates: Option<Vec<TopCandidates>>,

    /// Chosen candidates at each position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chosen_candidates: Option<Vec<LogprobsCandidate>>,
}

// ============================================================================
// Token Counting Types
// ============================================================================

/// Token count for a specific modality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModalityTokenCount {
    /// The modality.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modality: Option<MediaModality>,

    /// Token count for this modality.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<i32>,
}

/// Usage metadata in response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    /// Number of tokens in the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_token_count: Option<i32>,

    /// Number of tokens in the candidates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates_token_count: Option<i32>,

    /// Total token count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_token_count: Option<i32>,

    /// Cached content token count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_content_token_count: Option<i32>,

    /// Thoughts token count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thoughts_token_count: Option<i32>,

    /// Token count from tool use prompts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_prompt_token_count: Option<i32>,

    /// Token breakdown by modality for the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<Vec<ModalityTokenCount>>,

    /// Token breakdown by modality for cached content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_tokens_details: Option<Vec<ModalityTokenCount>>,

    /// Token breakdown by modality for candidates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates_tokens_details: Option<Vec<ModalityTokenCount>>,
}

/// A response candidate generated from the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// The generated content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Content>,

    /// The reason why the model stopped generating.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,

    /// Human-readable description of the finish reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_message: Option<String>,

    /// Safety ratings for the candidate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_ratings: Option<Vec<SafetyRating>>,

    /// Index of the candidate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i32>,

    /// Token count for this candidate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<i32>,

    /// Citation metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation_metadata: Option<CitationMetadata>,

    /// Average log probability across all tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_logprobs: Option<f64>,

    /// Grounding metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_metadata: Option<GroundingMetadata>,

    /// Log probabilities result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs_result: Option<LogprobsResult>,
}

/// Response from generateContent API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentResponse {
    /// Response candidates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<Candidate>>,

    /// Prompt feedback.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_feedback: Option<PromptFeedback>,

    /// Usage metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<UsageMetadata>,

    /// Model version used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,

    /// Unique response identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,

    /// Timestamp when the response was created (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<String>,
}

impl GenerateContentResponse {
    /// Get the text from the first candidate.
    pub fn text(&self) -> Option<String> {
        self.candidates
            .as_ref()?
            .first()?
            .content
            .as_ref()?
            .parts
            .as_ref()?
            .iter()
            .filter_map(|p| {
                // Skip thought parts
                if p.thought == Some(true) {
                    return None;
                }
                p.text.clone()
            })
            .reduce(|acc, s| acc + &s)
    }

    /// Get function calls from the first candidate.
    pub fn function_calls(&self) -> Option<Vec<&FunctionCall>> {
        let parts = self
            .candidates
            .as_ref()?
            .first()?
            .content
            .as_ref()?
            .parts
            .as_ref()?;

        let calls: Vec<_> = parts
            .iter()
            .filter_map(|p| p.function_call.as_ref())
            .collect();

        if calls.is_empty() { None } else { Some(calls) }
    }

    /// Get the finish reason from the first candidate.
    pub fn finish_reason(&self) -> Option<FinishReason> {
        self.candidates.as_ref()?.first()?.finish_reason
    }

    /// Get the parts from the first candidate.
    pub fn parts(&self) -> Option<&Vec<Part>> {
        self.candidates
            .as_ref()?
            .first()?
            .content
            .as_ref()?
            .parts
            .as_ref()
    }

    /// Get thought/reasoning text from the first candidate (for debugging).
    pub fn thought_text(&self) -> Option<String> {
        self.candidates
            .as_ref()?
            .first()?
            .content
            .as_ref()?
            .parts
            .as_ref()?
            .iter()
            .filter_map(|p| {
                if p.thought == Some(true) {
                    p.text.clone()
                } else {
                    None
                }
            })
            .reduce(|acc, s| acc + &s)
    }

    /// Get thought signatures from the response for use in subsequent requests.
    /// These can be passed back to the model to continue a thought chain.
    /// Returns raw bytes (base64 decoded) for each thought signature.
    pub fn thought_signatures(&self) -> Vec<Vec<u8>> {
        self.candidates
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.content.as_ref())
            .and_then(|c| c.parts.as_ref())
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p.thought_signature.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if the response contains thought/reasoning content.
    pub fn has_thoughts(&self) -> bool {
        self.candidates
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.content.as_ref())
            .and_then(|c| c.parts.as_ref())
            .map(|parts| parts.iter().any(|p| p.thought == Some(true)))
            .unwrap_or(false)
    }
}

// ============================================================================
// Error Response
// ============================================================================

/// Error details from the API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorDetail {
    #[serde(rename = "@type", skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// Error from the API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: i32,
    pub message: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<ApiErrorDetail>>,
}

/// Error response wrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: ApiError,
}

// ============================================================================
// Request Extensions
// ============================================================================

/// Extension configuration for API requests.
///
/// Allows adding extra headers, query parameters, and body fields to requests.
/// Supports two-level configuration: client-level (default) and request-level.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RequestExtensions {
    /// Additional HTTP headers.
    pub headers: Option<HashMap<String, String>>,
    /// Additional URL query parameters.
    pub params: Option<HashMap<String, String>>,
    /// Additional body fields (shallow-merged into request JSON root).
    pub body: Option<serde_json::Value>,
}

impl RequestExtensions {
    /// Create a new empty RequestExtensions.
    pub fn new() -> Self {
        Default::default()
    }

    /// Add an HTTP header.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }

    /// Add a URL query parameter.
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }

    /// Add a body field.
    pub fn with_body_field(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        let body = self.body.get_or_insert_with(|| serde_json::json!({}));
        if let Some(obj) = body.as_object_mut() {
            obj.insert(key.into(), value);
        }
        self
    }

    /// Set the entire body (replaces any existing body).
    pub fn with_body(mut self, body: serde_json::Value) -> Self {
        self.body = Some(body);
        self
    }

    /// Shallow merge: other's fields override self's fields.
    pub fn merge(&self, other: &RequestExtensions) -> RequestExtensions {
        RequestExtensions {
            headers: merge_hashmaps(&self.headers, &other.headers),
            params: merge_hashmaps(&self.params, &other.params),
            body: merge_json_objects(&self.body, &other.body),
        }
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.headers.as_ref().map_or(true, |h| h.is_empty())
            && self.params.as_ref().map_or(true, |p| p.is_empty())
            && self.body.is_none()
    }
}

fn merge_hashmaps(
    base: &Option<HashMap<String, String>>,
    other: &Option<HashMap<String, String>>,
) -> Option<HashMap<String, String>> {
    match (base, other) {
        (None, None) => None,
        (Some(b), None) => Some(b.clone()),
        (None, Some(o)) => Some(o.clone()),
        (Some(b), Some(o)) => {
            let mut merged = b.clone();
            merged.extend(o.iter().map(|(k, v)| (k.clone(), v.clone())));
            Some(merged)
        }
    }
}

fn merge_json_objects(
    base: &Option<serde_json::Value>,
    other: &Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    match (base, other) {
        (None, None) => None,
        (Some(b), None) => Some(b.clone()),
        (None, Some(o)) => Some(o.clone()),
        (Some(b), Some(o)) => {
            let mut merged = b.clone();
            if let (Some(base_obj), Some(other_obj)) = (merged.as_object_mut(), o.as_object()) {
                for (k, v) in other_obj {
                    base_obj.insert(k.clone(), v.clone());
                }
            }
            Some(merged)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization_structure() {
        // Test that the request is serialized with correct field names
        let request = GenerateContentRequest {
            contents: vec![Content::user("Hello")],
            system_instruction: Some(Content {
                parts: Some(vec![Part::text("You are helpful")]),
                role: Some("user".to_string()),
            }),
            generation_config: Some(GenerationConfig {
                temperature: Some(0.7),
                max_output_tokens: Some(1024),
                ..Default::default()
            }),
            safety_settings: None,
            tools: Some(vec![Tool::functions(vec![FunctionDeclaration::new(
                "test_func",
            )])]),
            tool_config: None,
        };

        let json = serde_json::to_value(&request).expect("serialization failed");

        // Verify top-level fields
        assert!(json.get("contents").is_some());
        assert!(json.get("systemInstruction").is_some());
        assert!(json.get("generationConfig").is_some());
        assert!(json.get("tools").is_some());

        // Verify generationConfig contains temperature (camelCase)
        let gen_config = json.get("generationConfig").unwrap();
        // Check temperature exists and is approximately 0.7 (f32 precision)
        let temp = gen_config.get("temperature").unwrap().as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.001);
        assert_eq!(
            gen_config.get("maxOutputTokens"),
            Some(&serde_json::json!(1024))
        );

        // Verify contents structure
        let contents = json.get("contents").unwrap().as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].get("role"), Some(&serde_json::json!("user")));
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello!"}],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 20,
                "totalTokenCount": 30
            }
        }"#;

        let response: GenerateContentResponse =
            serde_json::from_str(json).expect("deserialization failed");

        assert!(response.candidates.is_some());
        assert_eq!(response.text(), Some("Hello!".to_string()));
        assert_eq!(response.finish_reason(), Some(FinishReason::Stop));

        let usage = response.usage_metadata.unwrap();
        assert_eq!(usage.prompt_token_count, Some(10));
        assert_eq!(usage.candidates_token_count, Some(20));
        assert_eq!(usage.total_token_count, Some(30));
    }

    #[test]
    fn test_function_call_response_deserialization() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "get_weather",
                            "args": {"location": "Tokyo"}
                        }
                    }],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        }"#;

        let response: GenerateContentResponse =
            serde_json::from_str(json).expect("deserialization failed");

        let calls = response.function_calls().expect("no function calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, Some("get_weather".to_string()));
        assert_eq!(
            calls[0].args,
            Some(serde_json::json!({"location": "Tokyo"}))
        );
    }

    #[test]
    fn test_part_constructors() {
        // Text part
        let part = Part::text("hello");
        assert_eq!(part.text, Some("hello".to_string()));
        assert!(part.inline_data.is_none());

        // Image part from bytes
        let part = Part::from_bytes(&[1, 2, 3], "image/png");
        assert!(part.inline_data.is_some());
        let blob = part.inline_data.unwrap();
        assert_eq!(blob.mime_type, Some("image/png".to_string()));

        // Function call part
        let part = Part::function_call("test", serde_json::json!({"a": 1}));
        assert!(part.function_call.is_some());
        assert_eq!(part.function_call.unwrap().name, Some("test".to_string()));
    }

    #[test]
    fn test_content_constructors() {
        let user = Content::user("Hello");
        assert_eq!(user.role, Some("user".to_string()));

        let model = Content::model("Hi there");
        assert_eq!(model.role, Some("model".to_string()));
    }

    #[test]
    fn test_tool_constructors() {
        let tool = Tool::functions(vec![
            FunctionDeclaration::new("func1").with_description("A function"),
        ]);
        assert!(tool.function_declarations.is_some());
        assert!(tool.google_search.is_none());

        let search_tool = Tool::google_search();
        assert!(search_tool.google_search.is_some());
        assert!(search_tool.function_declarations.is_none());
    }

    #[test]
    fn test_generate_content_config_has_generation_params() {
        let empty = GenerateContentConfig::default();
        assert!(!empty.has_generation_params());

        let with_temp = GenerateContentConfig {
            temperature: Some(0.5),
            ..Default::default()
        };
        assert!(with_temp.has_generation_params());

        let with_system_only = GenerateContentConfig {
            system_instruction: Some(Content::user("system")),
            ..Default::default()
        };
        assert!(!with_system_only.has_generation_params());
    }

    #[test]
    fn test_request_extensions_builder() {
        let ext = RequestExtensions::new()
            .with_header("X-Custom", "value1")
            .with_param("key", "value2")
            .with_body_field("field", serde_json::json!("value3"));

        assert_eq!(
            ext.headers.as_ref().unwrap().get("X-Custom"),
            Some(&"value1".to_string())
        );
        assert_eq!(
            ext.params.as_ref().unwrap().get("key"),
            Some(&"value2".to_string())
        );
        assert_eq!(
            ext.body.as_ref().unwrap().get("field"),
            Some(&serde_json::json!("value3"))
        );
    }

    #[test]
    fn test_request_extensions_with_body() {
        let ext = RequestExtensions::new().with_body(serde_json::json!({"a": 1, "b": 2}));

        assert_eq!(ext.body, Some(serde_json::json!({"a": 1, "b": 2})));
    }

    #[test]
    fn test_request_extensions_merge_headers() {
        let base = RequestExtensions::new()
            .with_header("A", "1")
            .with_header("B", "2");

        let other = RequestExtensions::new()
            .with_header("B", "3") // Override
            .with_header("C", "4");

        let merged = base.merge(&other);

        let headers = merged.headers.unwrap();
        assert_eq!(headers.get("A"), Some(&"1".to_string()));
        assert_eq!(headers.get("B"), Some(&"3".to_string())); // Overridden
        assert_eq!(headers.get("C"), Some(&"4".to_string()));
    }

    #[test]
    fn test_request_extensions_merge_params() {
        let base = RequestExtensions::new().with_param("x", "1");
        let other = RequestExtensions::new()
            .with_param("x", "2") // Override
            .with_param("y", "3");

        let merged = base.merge(&other);

        let params = merged.params.unwrap();
        assert_eq!(params.get("x"), Some(&"2".to_string())); // Overridden
        assert_eq!(params.get("y"), Some(&"3".to_string()));
    }

    #[test]
    fn test_request_extensions_merge_body() {
        let base = RequestExtensions::new()
            .with_body_field("a", serde_json::json!(1))
            .with_body_field("b", serde_json::json!(2));

        let other = RequestExtensions::new()
            .with_body_field("b", serde_json::json!(3)) // Override
            .with_body_field("c", serde_json::json!(4));

        let merged = base.merge(&other);

        let body = merged.body.unwrap();
        assert_eq!(body.get("a"), Some(&serde_json::json!(1)));
        assert_eq!(body.get("b"), Some(&serde_json::json!(3))); // Overridden
        assert_eq!(body.get("c"), Some(&serde_json::json!(4)));
    }

    #[test]
    fn test_request_extensions_is_empty() {
        assert!(RequestExtensions::new().is_empty());

        assert!(!RequestExtensions::new().with_header("X", "Y").is_empty());
        assert!(!RequestExtensions::new().with_param("X", "Y").is_empty());
        assert!(
            !RequestExtensions::new()
                .with_body_field("X", serde_json::json!("Y"))
                .is_empty()
        );
    }

    #[test]
    fn test_request_extensions_merge_with_none() {
        let ext = RequestExtensions::new().with_header("A", "1");

        // Merge with empty
        let merged = ext.merge(&RequestExtensions::new());
        assert_eq!(
            merged.headers.as_ref().unwrap().get("A"),
            Some(&"1".to_string())
        );

        // Empty merge with non-empty
        let merged = RequestExtensions::new().merge(&ext);
        assert_eq!(
            merged.headers.as_ref().unwrap().get("A"),
            Some(&"1".to_string())
        );
    }

    // ========== Python SDK Alignment Tests ==========

    #[test]
    fn test_thinking_config_serialization() {
        let config = ThinkingConfig {
            include_thoughts: Some(true),
            thinking_budget: Some(1024),
            thinking_level: Some(ThinkingLevel::High),
        };

        let json = serde_json::to_value(&config).expect("serialization failed");

        // Verify camelCase field names
        assert_eq!(json["includeThoughts"], true);
        assert_eq!(json["thinkingBudget"], 1024);
        assert_eq!(json["thinkingLevel"], "HIGH");
    }

    #[test]
    fn test_thought_signature_base64_roundtrip() {
        // Test binary data with various byte values including 0x00 and 0xFF
        let original_sig = vec![0x00, 0x01, 0x02, 0xFF, 0xFE];
        let part = Part::with_thought_signature(original_sig.clone());

        // Serialize to JSON
        let json = serde_json::to_string(&part).expect("serialization failed");

        // Verify base64 encoding is present (0x00, 0x01, 0x02, 0xFF, 0xFE -> "AAEC//4=")
        assert!(
            json.contains("AAEC//4="),
            "Expected base64 encoding in: {}",
            json
        );

        // Deserialize back
        let parsed: Part = serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(
            parsed.thought_signature,
            Some(original_sig),
            "Round-trip should preserve exact bytes"
        );
        assert_eq!(parsed.thought, Some(true));
    }

    #[test]
    fn test_multiple_function_calls_in_response() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [
                        {"functionCall": {"id": "call_1", "name": "tool_a", "args": {"x": 1}}},
                        {"functionCall": {"id": "call_2", "name": "tool_b", "args": {"y": 2}}}
                    ],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        }"#;

        let response: GenerateContentResponse =
            serde_json::from_str(json).expect("deserialization failed");
        let calls = response.function_calls().expect("no function calls");

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, Some("call_1".to_string()));
        assert_eq!(calls[0].name, Some("tool_a".to_string()));
        assert_eq!(calls[1].id, Some("call_2".to_string()));
        assert_eq!(calls[1].name, Some("tool_b".to_string()));
    }

    #[test]
    fn test_thinking_config_nested_in_generation_config() {
        let gen_config = GenerationConfig {
            temperature: Some(0.7),
            thinking_config: Some(ThinkingConfig::with_budget(2048)),
            ..Default::default()
        };

        let request = GenerateContentRequest {
            contents: vec![Content::user("test")],
            system_instruction: None,
            generation_config: Some(gen_config),
            safety_settings: None,
            tools: None,
            tool_config: None,
        };

        let json = serde_json::to_value(&request).expect("serialization failed");

        // Verify thinkingConfig is nested inside generationConfig
        assert_eq!(
            json["generationConfig"]["thinkingConfig"]["thinkingBudget"], 2048,
            "thinkingConfig should be nested in generationConfig"
        );
        // Verify thinkingConfig is NOT at top level
        assert!(
            json.get("thinkingConfig").is_none(),
            "thinkingConfig should not be at top level"
        );
    }

    #[test]
    fn test_usage_metadata_full_fields() {
        let json = r#"{
            "candidates": [{
                "content": {"parts": [{"text": "Hi"}], "role": "model"}
            }],
            "usageMetadata": {
                "promptTokenCount": 100,
                "candidatesTokenCount": 50,
                "totalTokenCount": 150,
                "cachedContentTokenCount": 20,
                "thoughtsTokenCount": 30,
                "toolUsePromptTokenCount": 10
            }
        }"#;

        let response: GenerateContentResponse =
            serde_json::from_str(json).expect("deserialization failed");
        let usage = response.usage_metadata.expect("no usage metadata");

        assert_eq!(usage.prompt_token_count, Some(100));
        assert_eq!(usage.candidates_token_count, Some(50));
        assert_eq!(usage.total_token_count, Some(150));
        assert_eq!(usage.cached_content_token_count, Some(20));
        assert_eq!(usage.thoughts_token_count, Some(30));
        assert_eq!(usage.tool_use_prompt_token_count, Some(10));
    }

    #[test]
    fn test_function_call_with_thought_signature() {
        // Test that function call parts can have thought_signature attached
        let part = Part {
            function_call: Some(FunctionCall {
                id: Some("call_1".to_string()),
                name: Some("search".to_string()),
                args: Some(serde_json::json!({"query": "rust"})),
                partial_args: None,
                will_continue: None,
            }),
            thought_signature: Some(b"sig_for_call_1".to_vec()),
            ..Default::default()
        };

        let json = serde_json::to_string(&part).expect("serialization failed");
        let parsed: Part = serde_json::from_str(&json).expect("deserialization failed");

        assert!(parsed.function_call.is_some());
        assert!(parsed.thought_signature.is_some());
        assert_eq!(
            parsed.thought_signature.unwrap(),
            b"sig_for_call_1".to_vec()
        );
    }

    #[test]
    fn test_thought_part_with_text_and_signature() {
        // Test thought part containing both text and signature (common in reasoning)
        let part = Part {
            text: Some("Let me think about this...".to_string()),
            thought: Some(true),
            thought_signature: Some(b"thought_sig_123".to_vec()),
            ..Default::default()
        };

        let json = serde_json::to_string(&part).expect("serialization failed");
        let parsed: Part = serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(parsed.text, Some("Let me think about this...".to_string()));
        assert_eq!(parsed.thought, Some(true));
        assert!(parsed.is_thought());
        assert!(parsed.thought_signature.is_some());
    }

    #[test]
    fn test_response_thought_extraction() {
        // Test response.thought_text() and response.thought_signatures()
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [
                        {"text": "Thinking step 1...", "thought": true, "thoughtSignature": "c2lnMQ=="},
                        {"text": "Thinking step 2...", "thought": true},
                        {"text": "Final answer here"}
                    ],
                    "role": "model"
                }
            }]
        }"#;

        let response: GenerateContentResponse =
            serde_json::from_str(json).expect("deserialization failed");

        // text() should exclude thought parts
        let text = response.text().expect("no text");
        assert_eq!(text, "Final answer here");

        // thought_text() should only include thought parts
        let thought = response.thought_text().expect("no thought text");
        assert!(thought.contains("Thinking step 1"));
        assert!(thought.contains("Thinking step 2"));

        // has_thoughts() should return true
        assert!(response.has_thoughts());

        // thought_signatures() should extract the signature
        let sigs = response.thought_signatures();
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0], b"sig1"); // "c2lnMQ==" decodes to "sig1"
    }
}
