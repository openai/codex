//! Stream event types for the OpenAI Responses API.
//!
//! This module defines all 53 event types that can be received during streaming.

use serde::Deserialize;
use serde::Serialize;

use super::OutputContentBlock;
use super::OutputItem;
use super::Response;

// ============================================================================
// Logprob types for streaming
// ============================================================================

/// Top logprob alternative.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopLogprob {
    /// Token text.
    #[serde(default)]
    pub token: Option<String>,
    /// Log probability of this token.
    #[serde(default)]
    pub logprob: Option<f64>,
}

/// Logprob for a token in streaming output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamLogprob {
    /// Token text.
    pub token: String,
    /// Log probability of this token.
    pub logprob: f64,
    /// Top alternative tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<Vec<TopLogprob>>,
}

// ============================================================================
// Content part types for streaming
// ============================================================================

/// Output text content part.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputTextPart {
    /// Part type (always "output_text").
    #[serde(rename = "type")]
    pub part_type: String,
    /// Text content.
    #[serde(default)]
    pub text: String,
    /// Annotations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<serde_json::Value>,
}

/// Refusal content part.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefusalPart {
    /// Part type (always "refusal").
    #[serde(rename = "type")]
    pub part_type: String,
    /// Refusal text.
    #[serde(default)]
    pub refusal: String,
}

/// Reasoning text content part.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTextPart {
    /// Part type (always "reasoning_text").
    #[serde(rename = "type")]
    pub part_type: String,
    /// Reasoning text.
    #[serde(default)]
    pub text: String,
}

/// Content part in stream events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// Output text part.
    OutputText {
        /// Text content.
        #[serde(default)]
        text: String,
        /// Annotations.
        #[serde(default)]
        annotations: Vec<serde_json::Value>,
    },
    /// Refusal part.
    Refusal {
        /// Refusal text.
        #[serde(default)]
        refusal: String,
    },
    /// Reasoning text part.
    ReasoningText {
        /// Reasoning text.
        #[serde(default)]
        text: String,
    },
}

// ============================================================================
// Annotation types
// ============================================================================

/// Output text annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputTextAnnotation {
    /// File citation.
    FileCitation {
        /// File ID.
        file_id: String,
        /// Index.
        index: i32,
    },
    /// URL citation.
    UrlCitation {
        /// URL.
        url: String,
        /// Title.
        #[serde(default)]
        title: Option<String>,
        /// Start index.
        #[serde(default)]
        start_index: Option<i32>,
        /// End index.
        #[serde(default)]
        end_index: Option<i32>,
    },
    /// File path.
    FilePath {
        /// File ID.
        file_id: String,
        /// Index.
        index: i32,
    },
}

// ============================================================================
// Main ResponseStreamEvent enum
// ============================================================================

/// All possible events that can be received during streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseStreamEvent {
    // ========================================================================
    // Lifecycle events
    // ========================================================================
    /// Emitted when a response is created.
    #[serde(rename = "response.created")]
    ResponseCreated {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// The response object.
        response: Response,
    },

    /// Emitted when a response starts processing.
    #[serde(rename = "response.in_progress")]
    ResponseInProgress {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// The response object.
        response: Response,
    },

    /// Emitted when a response is completed successfully.
    #[serde(rename = "response.completed")]
    ResponseCompleted {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// The completed response.
        response: Response,
    },

    /// Emitted when a response fails.
    #[serde(rename = "response.failed")]
    ResponseFailed {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// The failed response with error details.
        response: Response,
    },

    /// Emitted when a response is incomplete.
    #[serde(rename = "response.incomplete")]
    ResponseIncomplete {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// The incomplete response.
        response: Response,
    },

    /// Emitted when a response is queued.
    #[serde(rename = "response.queued")]
    ResponseQueued {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// The queued response.
        response: Response,
    },

    // ========================================================================
    // Output text events
    // ========================================================================
    /// Emitted when there is a text delta.
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Content part index.
        content_index: i32,
        /// Text delta.
        delta: String,
        /// Log probabilities.
        #[serde(default)]
        logprobs: Vec<StreamLogprob>,
    },

    /// Emitted when text output is complete.
    #[serde(rename = "response.output_text.done")]
    OutputTextDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Content part index.
        content_index: i32,
        /// Complete text.
        text: String,
        /// Log probabilities.
        #[serde(default)]
        logprobs: Vec<StreamLogprob>,
    },

    // ========================================================================
    // Refusal events
    // ========================================================================
    /// Emitted when there is a refusal delta.
    #[serde(rename = "response.refusal.delta")]
    RefusalDelta {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Content part index.
        content_index: i32,
        /// Refusal delta.
        delta: String,
    },

    /// Emitted when refusal is complete.
    #[serde(rename = "response.refusal.done")]
    RefusalDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Content part index.
        content_index: i32,
        /// Complete refusal text.
        refusal: String,
    },

    // ========================================================================
    // Function call events
    // ========================================================================
    /// Emitted when there is a function call arguments delta.
    #[serde(rename = "response.function_call_arguments.delta")]
    FunctionCallArgumentsDelta {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Arguments delta (partial JSON).
        delta: String,
    },

    /// Emitted when function call arguments are complete.
    #[serde(rename = "response.function_call_arguments.done")]
    FunctionCallArgumentsDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Complete arguments JSON.
        arguments: String,
    },

    // ========================================================================
    // Output item events
    // ========================================================================
    /// Emitted when a new output item is added.
    #[serde(rename = "response.output_item.added")]
    OutputItemAdded {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Index in output array.
        output_index: i32,
        /// The output item.
        item: OutputItem,
    },

    /// Emitted when an output item is complete.
    #[serde(rename = "response.output_item.done")]
    OutputItemDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Index in output array.
        output_index: i32,
        /// The completed output item.
        item: OutputItem,
    },

    // ========================================================================
    // Content part events
    // ========================================================================
    /// Emitted when a new content part is added.
    #[serde(rename = "response.content_part.added")]
    ContentPartAdded {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Content part index.
        content_index: i32,
        /// The content part.
        part: ContentPart,
    },

    /// Emitted when a content part is complete.
    #[serde(rename = "response.content_part.done")]
    ContentPartDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Content part index.
        content_index: i32,
        /// The completed content part.
        part: OutputContentBlock,
    },

    // ========================================================================
    // Reasoning events
    // ========================================================================
    /// Emitted when there is a reasoning text delta.
    #[serde(rename = "response.reasoning_text.delta")]
    ReasoningTextDelta {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Content part index.
        content_index: i32,
        /// Reasoning text delta.
        delta: String,
    },

    /// Emitted when reasoning text is complete.
    #[serde(rename = "response.reasoning_text.done")]
    ReasoningTextDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Content part index.
        content_index: i32,
        /// Complete reasoning text.
        text: String,
    },

    /// Emitted when a reasoning summary part is added.
    #[serde(rename = "response.reasoning_summary_part.added")]
    ReasoningSummaryPartAdded {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Summary index.
        summary_index: i32,
    },

    /// Emitted when a reasoning summary part is complete.
    #[serde(rename = "response.reasoning_summary_part.done")]
    ReasoningSummaryPartDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Summary index.
        summary_index: i32,
    },

    /// Emitted when there is a reasoning summary text delta.
    #[serde(rename = "response.reasoning_summary_text.delta")]
    ReasoningSummaryTextDelta {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Summary index.
        summary_index: i32,
        /// Text delta.
        delta: String,
    },

    /// Emitted when reasoning summary text is complete.
    #[serde(rename = "response.reasoning_summary_text.done")]
    ReasoningSummaryTextDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Summary index.
        summary_index: i32,
        /// Complete summary text.
        text: String,
    },

    // ========================================================================
    // Audio events
    // ========================================================================
    /// Emitted when there is an audio delta.
    #[serde(rename = "response.audio.delta")]
    AudioDelta {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Base64-encoded audio delta.
        delta: String,
    },

    /// Emitted when audio output is complete.
    #[serde(rename = "response.audio.done")]
    AudioDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Complete base64-encoded audio.
        #[serde(default)]
        data: Option<String>,
    },

    /// Emitted when there is an audio transcript delta.
    #[serde(rename = "response.audio_transcript.delta")]
    AudioTranscriptDelta {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Transcript delta.
        delta: String,
    },

    /// Emitted when audio transcript is complete.
    #[serde(rename = "response.audio_transcript.done")]
    AudioTranscriptDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Complete transcript.
        transcript: String,
    },

    // ========================================================================
    // MCP events
    // ========================================================================
    /// Emitted when an MCP call starts.
    #[serde(rename = "response.mcp_call.in_progress")]
    McpCallInProgress {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when an MCP call completes.
    #[serde(rename = "response.mcp_call.completed")]
    McpCallCompleted {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when an MCP call fails.
    #[serde(rename = "response.mcp_call.failed")]
    McpCallFailed {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when there is an MCP call arguments delta.
    #[serde(rename = "response.mcp_call_arguments.delta")]
    McpCallArgumentsDelta {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Arguments delta.
        delta: String,
    },

    /// Emitted when MCP call arguments are complete.
    #[serde(rename = "response.mcp_call_arguments.done")]
    McpCallArgumentsDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Complete arguments.
        arguments: String,
    },

    /// Emitted when MCP list tools starts.
    #[serde(rename = "response.mcp_list_tools.in_progress")]
    McpListToolsInProgress {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when MCP list tools completes.
    #[serde(rename = "response.mcp_list_tools.completed")]
    McpListToolsCompleted {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when MCP list tools fails.
    #[serde(rename = "response.mcp_list_tools.failed")]
    McpListToolsFailed {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    // ========================================================================
    // File search events
    // ========================================================================
    /// Emitted when file search starts.
    #[serde(rename = "response.file_search_call.in_progress")]
    FileSearchCallInProgress {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when file search is searching.
    #[serde(rename = "response.file_search_call.searching")]
    FileSearchCallSearching {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when file search completes.
    #[serde(rename = "response.file_search_call.completed")]
    FileSearchCallCompleted {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    // ========================================================================
    // Web search events
    // ========================================================================
    /// Emitted when web search starts.
    #[serde(rename = "response.web_search_call.in_progress")]
    WebSearchCallInProgress {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when web search is searching.
    #[serde(rename = "response.web_search_call.searching")]
    WebSearchCallSearching {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when web search completes.
    #[serde(rename = "response.web_search_call.completed")]
    WebSearchCallCompleted {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    // ========================================================================
    // Code interpreter events
    // ========================================================================
    /// Emitted when code interpreter starts.
    #[serde(rename = "response.code_interpreter_call.in_progress")]
    CodeInterpreterCallInProgress {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when code interpreter is interpreting.
    #[serde(rename = "response.code_interpreter_call.interpreting")]
    CodeInterpreterCallInterpreting {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when code interpreter completes.
    #[serde(rename = "response.code_interpreter_call.completed")]
    CodeInterpreterCallCompleted {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when there is a code delta.
    #[serde(rename = "response.code_interpreter_call.code_delta")]
    CodeInterpreterCallCodeDelta {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Code delta.
        delta: String,
    },

    /// Emitted when code is complete.
    #[serde(rename = "response.code_interpreter_call.code_done")]
    CodeInterpreterCallCodeDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Complete code.
        code: String,
    },

    // ========================================================================
    // Image generation events
    // ========================================================================
    /// Emitted when image generation starts.
    #[serde(rename = "response.image_generation_call.in_progress")]
    ImageGenCallInProgress {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when image generation is generating.
    #[serde(rename = "response.image_generation_call.generating")]
    ImageGenCallGenerating {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    /// Emitted when there is a partial image.
    #[serde(rename = "response.image_generation_call.partial_image")]
    ImageGenCallPartialImage {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Partial image data (base64).
        #[serde(default)]
        partial_image_b64: Option<String>,
        /// Partial image index.
        #[serde(default)]
        partial_image_index: Option<i32>,
    },

    /// Emitted when image generation completes.
    #[serde(rename = "response.image_generation_call.completed")]
    ImageGenCallCompleted {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
    },

    // ========================================================================
    // Custom tool events
    // ========================================================================
    /// Emitted when there is a custom tool input delta.
    #[serde(rename = "response.custom_tool_call_input.delta")]
    CustomToolCallInputDelta {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Input delta.
        delta: String,
    },

    /// Emitted when custom tool input is complete.
    #[serde(rename = "response.custom_tool_call_input.done")]
    CustomToolCallInputDone {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Complete input.
        input: String,
    },

    // ========================================================================
    // Annotation events
    // ========================================================================
    /// Emitted when a text annotation is added.
    #[serde(rename = "response.output_text.annotation.added")]
    OutputTextAnnotationAdded {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Output item ID.
        item_id: String,
        /// Index in output array.
        output_index: i32,
        /// Content part index.
        content_index: i32,
        /// Annotation index.
        annotation_index: i32,
        /// The annotation.
        annotation: serde_json::Value,
    },

    // ========================================================================
    // Error event
    // ========================================================================
    /// Emitted when an error occurs during streaming.
    #[serde(rename = "error")]
    Error {
        /// Sequence number for ordering.
        sequence_number: i32,
        /// Error code.
        #[serde(default)]
        code: Option<String>,
        /// Error message.
        message: String,
        /// Error parameter.
        #[serde(default)]
        param: Option<String>,
    },
}

impl ResponseStreamEvent {
    /// Get the sequence number of this event.
    pub fn sequence_number(&self) -> i32 {
        match self {
            Self::ResponseCreated {
                sequence_number, ..
            } => *sequence_number,
            Self::ResponseInProgress {
                sequence_number, ..
            } => *sequence_number,
            Self::ResponseCompleted {
                sequence_number, ..
            } => *sequence_number,
            Self::ResponseFailed {
                sequence_number, ..
            } => *sequence_number,
            Self::ResponseIncomplete {
                sequence_number, ..
            } => *sequence_number,
            Self::ResponseQueued {
                sequence_number, ..
            } => *sequence_number,
            Self::OutputTextDelta {
                sequence_number, ..
            } => *sequence_number,
            Self::OutputTextDone {
                sequence_number, ..
            } => *sequence_number,
            Self::RefusalDelta {
                sequence_number, ..
            } => *sequence_number,
            Self::RefusalDone {
                sequence_number, ..
            } => *sequence_number,
            Self::FunctionCallArgumentsDelta {
                sequence_number, ..
            } => *sequence_number,
            Self::FunctionCallArgumentsDone {
                sequence_number, ..
            } => *sequence_number,
            Self::OutputItemAdded {
                sequence_number, ..
            } => *sequence_number,
            Self::OutputItemDone {
                sequence_number, ..
            } => *sequence_number,
            Self::ContentPartAdded {
                sequence_number, ..
            } => *sequence_number,
            Self::ContentPartDone {
                sequence_number, ..
            } => *sequence_number,
            Self::ReasoningTextDelta {
                sequence_number, ..
            } => *sequence_number,
            Self::ReasoningTextDone {
                sequence_number, ..
            } => *sequence_number,
            Self::ReasoningSummaryPartAdded {
                sequence_number, ..
            } => *sequence_number,
            Self::ReasoningSummaryPartDone {
                sequence_number, ..
            } => *sequence_number,
            Self::ReasoningSummaryTextDelta {
                sequence_number, ..
            } => *sequence_number,
            Self::ReasoningSummaryTextDone {
                sequence_number, ..
            } => *sequence_number,
            Self::AudioDelta {
                sequence_number, ..
            } => *sequence_number,
            Self::AudioDone {
                sequence_number, ..
            } => *sequence_number,
            Self::AudioTranscriptDelta {
                sequence_number, ..
            } => *sequence_number,
            Self::AudioTranscriptDone {
                sequence_number, ..
            } => *sequence_number,
            Self::McpCallInProgress {
                sequence_number, ..
            } => *sequence_number,
            Self::McpCallCompleted {
                sequence_number, ..
            } => *sequence_number,
            Self::McpCallFailed {
                sequence_number, ..
            } => *sequence_number,
            Self::McpCallArgumentsDelta {
                sequence_number, ..
            } => *sequence_number,
            Self::McpCallArgumentsDone {
                sequence_number, ..
            } => *sequence_number,
            Self::McpListToolsInProgress {
                sequence_number, ..
            } => *sequence_number,
            Self::McpListToolsCompleted {
                sequence_number, ..
            } => *sequence_number,
            Self::McpListToolsFailed {
                sequence_number, ..
            } => *sequence_number,
            Self::FileSearchCallInProgress {
                sequence_number, ..
            } => *sequence_number,
            Self::FileSearchCallSearching {
                sequence_number, ..
            } => *sequence_number,
            Self::FileSearchCallCompleted {
                sequence_number, ..
            } => *sequence_number,
            Self::WebSearchCallInProgress {
                sequence_number, ..
            } => *sequence_number,
            Self::WebSearchCallSearching {
                sequence_number, ..
            } => *sequence_number,
            Self::WebSearchCallCompleted {
                sequence_number, ..
            } => *sequence_number,
            Self::CodeInterpreterCallInProgress {
                sequence_number, ..
            } => *sequence_number,
            Self::CodeInterpreterCallInterpreting {
                sequence_number, ..
            } => *sequence_number,
            Self::CodeInterpreterCallCompleted {
                sequence_number, ..
            } => *sequence_number,
            Self::CodeInterpreterCallCodeDelta {
                sequence_number, ..
            } => *sequence_number,
            Self::CodeInterpreterCallCodeDone {
                sequence_number, ..
            } => *sequence_number,
            Self::ImageGenCallInProgress {
                sequence_number, ..
            } => *sequence_number,
            Self::ImageGenCallGenerating {
                sequence_number, ..
            } => *sequence_number,
            Self::ImageGenCallPartialImage {
                sequence_number, ..
            } => *sequence_number,
            Self::ImageGenCallCompleted {
                sequence_number, ..
            } => *sequence_number,
            Self::CustomToolCallInputDelta {
                sequence_number, ..
            } => *sequence_number,
            Self::CustomToolCallInputDone {
                sequence_number, ..
            } => *sequence_number,
            Self::OutputTextAnnotationAdded {
                sequence_number, ..
            } => *sequence_number,
            Self::Error {
                sequence_number, ..
            } => *sequence_number,
        }
    }

    /// Check if this is an error event.
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Check if this is a terminal event (completed, failed, or incomplete).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::ResponseCompleted { .. }
                | Self::ResponseFailed { .. }
                | Self::ResponseIncomplete { .. }
        )
    }

    /// Get the event type as a string.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ResponseCreated { .. } => "response.created",
            Self::ResponseInProgress { .. } => "response.in_progress",
            Self::ResponseCompleted { .. } => "response.completed",
            Self::ResponseFailed { .. } => "response.failed",
            Self::ResponseIncomplete { .. } => "response.incomplete",
            Self::ResponseQueued { .. } => "response.queued",
            Self::OutputTextDelta { .. } => "response.output_text.delta",
            Self::OutputTextDone { .. } => "response.output_text.done",
            Self::RefusalDelta { .. } => "response.refusal.delta",
            Self::RefusalDone { .. } => "response.refusal.done",
            Self::FunctionCallArgumentsDelta { .. } => "response.function_call_arguments.delta",
            Self::FunctionCallArgumentsDone { .. } => "response.function_call_arguments.done",
            Self::OutputItemAdded { .. } => "response.output_item.added",
            Self::OutputItemDone { .. } => "response.output_item.done",
            Self::ContentPartAdded { .. } => "response.content_part.added",
            Self::ContentPartDone { .. } => "response.content_part.done",
            Self::ReasoningTextDelta { .. } => "response.reasoning_text.delta",
            Self::ReasoningTextDone { .. } => "response.reasoning_text.done",
            Self::ReasoningSummaryPartAdded { .. } => "response.reasoning_summary_part.added",
            Self::ReasoningSummaryPartDone { .. } => "response.reasoning_summary_part.done",
            Self::ReasoningSummaryTextDelta { .. } => "response.reasoning_summary_text.delta",
            Self::ReasoningSummaryTextDone { .. } => "response.reasoning_summary_text.done",
            Self::AudioDelta { .. } => "response.audio.delta",
            Self::AudioDone { .. } => "response.audio.done",
            Self::AudioTranscriptDelta { .. } => "response.audio_transcript.delta",
            Self::AudioTranscriptDone { .. } => "response.audio_transcript.done",
            Self::McpCallInProgress { .. } => "response.mcp_call.in_progress",
            Self::McpCallCompleted { .. } => "response.mcp_call.completed",
            Self::McpCallFailed { .. } => "response.mcp_call.failed",
            Self::McpCallArgumentsDelta { .. } => "response.mcp_call_arguments.delta",
            Self::McpCallArgumentsDone { .. } => "response.mcp_call_arguments.done",
            Self::McpListToolsInProgress { .. } => "response.mcp_list_tools.in_progress",
            Self::McpListToolsCompleted { .. } => "response.mcp_list_tools.completed",
            Self::McpListToolsFailed { .. } => "response.mcp_list_tools.failed",
            Self::FileSearchCallInProgress { .. } => "response.file_search_call.in_progress",
            Self::FileSearchCallSearching { .. } => "response.file_search_call.searching",
            Self::FileSearchCallCompleted { .. } => "response.file_search_call.completed",
            Self::WebSearchCallInProgress { .. } => "response.web_search_call.in_progress",
            Self::WebSearchCallSearching { .. } => "response.web_search_call.searching",
            Self::WebSearchCallCompleted { .. } => "response.web_search_call.completed",
            Self::CodeInterpreterCallInProgress { .. } => {
                "response.code_interpreter_call.in_progress"
            }
            Self::CodeInterpreterCallInterpreting { .. } => {
                "response.code_interpreter_call.interpreting"
            }
            Self::CodeInterpreterCallCompleted { .. } => "response.code_interpreter_call.completed",
            Self::CodeInterpreterCallCodeDelta { .. } => {
                "response.code_interpreter_call.code_delta"
            }
            Self::CodeInterpreterCallCodeDone { .. } => "response.code_interpreter_call.code_done",
            Self::ImageGenCallInProgress { .. } => "response.image_generation_call.in_progress",
            Self::ImageGenCallGenerating { .. } => "response.image_generation_call.generating",
            Self::ImageGenCallPartialImage { .. } => "response.image_generation_call.partial_image",
            Self::ImageGenCallCompleted { .. } => "response.image_generation_call.completed",
            Self::CustomToolCallInputDelta { .. } => "response.custom_tool_call_input.delta",
            Self::CustomToolCallInputDone { .. } => "response.custom_tool_call_input.done",
            Self::OutputTextAnnotationAdded { .. } => "response.output_text.annotation.added",
            Self::Error { .. } => "error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_output_text_delta() {
        let json = r#"{
            "type": "response.output_text.delta",
            "sequence_number": 5,
            "item_id": "item-123",
            "output_index": 0,
            "content_index": 0,
            "delta": "Hello",
            "logprobs": []
        }"#;
        let event: ResponseStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(
            &event,
            ResponseStreamEvent::OutputTextDelta { delta, .. } if delta == "Hello"
        ));
        assert_eq!(event.sequence_number(), 5);
        assert_eq!(event.event_type(), "response.output_text.delta");
    }

    #[test]
    fn test_parse_response_completed() {
        let json = r#"{
            "type": "response.completed",
            "sequence_number": 10,
            "response": {
                "id": "resp-123",
                "status": "completed",
                "output": [],
                "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
            }
        }"#;
        let event: ResponseStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(
            event,
            ResponseStreamEvent::ResponseCompleted { .. }
        ));
        assert!(event.is_terminal());
    }

    #[test]
    fn test_parse_function_call_delta() {
        let json = r#"{
            "type": "response.function_call_arguments.delta",
            "sequence_number": 3,
            "item_id": "item-456",
            "output_index": 0,
            "delta": "{\"foo\":"
        }"#;
        let event: ResponseStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(
            &event,
            ResponseStreamEvent::FunctionCallArgumentsDelta { delta, .. } if delta == "{\"foo\":"
        ));
    }

    #[test]
    fn test_parse_error_event() {
        let json = r#"{
            "type": "error",
            "sequence_number": 1,
            "code": "context_length_exceeded",
            "message": "The context length exceeded the limit"
        }"#;
        let event: ResponseStreamEvent = serde_json::from_str(json).unwrap();
        assert!(event.is_error());
        assert!(matches!(
            &event,
            ResponseStreamEvent::Error { code: Some(c), message, .. }
                if c == "context_length_exceeded" && message.contains("context")
        ));
    }

    #[test]
    fn test_parse_output_item_added() {
        let json = r#"{
            "type": "response.output_item.added",
            "sequence_number": 2,
            "output_index": 0,
            "item": {
                "type": "message",
                "role": "assistant",
                "content": []
            }
        }"#;
        let event: ResponseStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, ResponseStreamEvent::OutputItemAdded { .. }));
    }
}
