//! Type definitions for the OpenAI SDK.

mod common;
mod content;
mod embeddings;
mod responses;
mod stream_events;
mod usage;

// Common types
pub use common::CustomToolInputFormat;
pub use common::FunctionDefinition;
pub use common::Metadata;
pub use common::RankingOptions;
pub use common::ResponseStatus;
pub use common::Role;
pub use common::StopReason;
pub use common::Tool;
pub use common::ToolChoice;
pub use common::UserLocation;

// Content types
pub use content::Annotation;
pub use content::AudioFormat;
pub use content::ComputerCallOutputData;
pub use content::ImageDetail;
pub use content::ImageMediaType;
pub use content::ImageSource;
pub use content::InputContentBlock;
pub use content::LogprobContent;
pub use content::Logprobs;
pub use content::OutputContentBlock;
pub use content::TokenLogprob;
pub use content::TopLogprob;

// Embedding types
pub use embeddings::CreateEmbeddingResponse;
pub use embeddings::Embedding;
pub use embeddings::EmbeddingCreateParams;
pub use embeddings::EmbeddingInput;
pub use embeddings::EmbeddingUsage;
pub use embeddings::EncodingFormat;

// Response types
pub use responses::CodeInterpreterOutput;
pub use responses::ComputerAction;
pub use responses::ConversationParam;
pub use responses::FileSearchResult;
pub use responses::ImageGenerationResult;
pub use responses::IncompleteDetails;
pub use responses::IncompleteReason;
pub use responses::InputMessage;
pub use responses::MIN_THINKING_BUDGET_TOKENS;
pub use responses::McpToolInfo;
pub use responses::MpcCallRef;
pub use responses::OutputItem;
pub use responses::PromptCacheRetention;
pub use responses::PromptCachingConfig;
pub use responses::PromptParam;
pub use responses::ReasoningConfig;
pub use responses::ReasoningEffort;
pub use responses::ReasoningSummary;
pub use responses::Response;
pub use responses::ResponseCreateParams;
pub use responses::ResponseError;
pub use responses::ResponseIncludable;
pub use responses::ResponseInput;
pub use responses::ResponsePrompt;
pub use responses::SafetyCheck;
pub use responses::SdkHttpResponse;
pub use responses::ServiceTier;
pub use responses::TextConfig;
pub use responses::TextFormat;
pub use responses::ThinkingConfig;
pub use responses::Truncation;
pub use responses::WebSearchResult;

// Usage types
pub use usage::InputTokensDetails;
pub use usage::OutputTokensDetails;
pub use usage::Usage;

// Stream event types
pub use stream_events::ContentPart;
pub use stream_events::OutputTextAnnotation;
pub use stream_events::ResponseStreamEvent;
pub use stream_events::StreamLogprob;
pub use stream_events::TopLogprob as StreamTopLogprob;
