use serde::Deserialize;
use serde::Serialize;

/// Detailed cache creation breakdown by TTL.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheCreation {
    /// Tokens cached with 5 minute TTL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ephemeral_5m_input_tokens: Option<i32>,

    /// Tokens cached with 1 hour TTL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ephemeral_1h_input_tokens: Option<i32>,
}

/// Token usage information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Number of input tokens used.
    pub input_tokens: i32,

    /// Number of output tokens generated.
    pub output_tokens: i32,

    /// Tokens used to create cache entries (total).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i32>,

    /// Tokens read from cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i32>,

    /// Detailed cache creation breakdown by TTL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation: Option<CacheCreation>,

    /// Service tier used for this request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

/// Response from token counting endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageTokensCount {
    /// Total number of tokens for the input.
    pub input_tokens: i32,
}
