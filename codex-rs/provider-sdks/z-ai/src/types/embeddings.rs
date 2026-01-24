//! Embedding types for Z.AI SDK.
//!
//! Types aligned with Python SDK `embeddings.py`.

use serde::Deserialize;
use serde::Serialize;

use super::CompletionUsage;

/// Parameters for creating embeddings.
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingsCreateParams {
    /// Input text or texts to embed.
    pub input: EmbeddingInput,
    /// Model name.
    pub model: String,
    /// Number of dimensions (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<i32>,
    /// Encoding format (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,
    /// User identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Request ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Input for embeddings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    /// Single text.
    Single(String),
    /// Multiple texts.
    Multiple(Vec<String>),
}

impl EmbeddingsCreateParams {
    /// Create new embedding parameters for single text.
    pub fn new(model: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            input: EmbeddingInput::Single(input.into()),
            dimensions: None,
            encoding_format: None,
            user: None,
            request_id: None,
        }
    }

    /// Create new embedding parameters for multiple texts.
    pub fn new_batch(model: impl Into<String>, inputs: Vec<String>) -> Self {
        Self {
            model: model.into(),
            input: EmbeddingInput::Multiple(inputs),
            dimensions: None,
            encoding_format: None,
            user: None,
            request_id: None,
        }
    }

    /// Set dimensions.
    pub fn dimensions(mut self, dimensions: i32) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    /// Set encoding format.
    pub fn encoding_format(mut self, format: impl Into<String>) -> Self {
        self.encoding_format = Some(format.into());
        self
    }

    /// Set user.
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Set request ID.
    pub fn request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

/// Embedding vector data.
///
/// From Python SDK `embeddings.py:9`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// Object type identifier.
    pub object: String,
    /// Index of the embedding in the list.
    #[serde(default)]
    pub index: Option<i32>,
    /// The embedding vector.
    pub embedding: Vec<f64>,
}

/// Embeddings generation response.
///
/// From Python SDK `embeddings.py:24`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsResponded {
    /// Object type identifier.
    pub object: String,
    /// List of embedding vectors.
    pub data: Vec<Embedding>,
    /// Model used for embedding generation.
    pub model: String,
    /// Token usage information.
    pub usage: CompletionUsage,
}

impl EmbeddingsResponded {
    /// Get the first embedding vector.
    pub fn embedding(&self) -> Option<&[f64]> {
        self.data.first().map(|e| e.embedding.as_slice())
    }

    /// Get all embedding vectors.
    pub fn embeddings(&self) -> Vec<&[f64]> {
        self.data.iter().map(|e| e.embedding.as_slice()).collect()
    }
}
