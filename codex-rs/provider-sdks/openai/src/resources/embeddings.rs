//! Embeddings resource for creating text embeddings.

use crate::client::Client;
use crate::error::Result;
use crate::types::CreateEmbeddingResponse;
use crate::types::EmbeddingCreateParams;

/// Resource for creating embeddings.
pub struct Embeddings<'a> {
    client: &'a Client,
}

impl<'a> Embeddings<'a> {
    /// Create a new Embeddings resource.
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Create embeddings for the given input.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use openai_sdk::{Client, EmbeddingCreateParams};
    ///
    /// let client = Client::from_env()?;
    /// let response = client.embeddings().create(
    ///     EmbeddingCreateParams::new("text-embedding-3-small", "Hello, world!")
    /// ).await?;
    ///
    /// println!("Embedding: {:?}", response.embedding());
    /// ```
    pub async fn create(&self, params: EmbeddingCreateParams) -> Result<CreateEmbeddingResponse> {
        let body = serde_json::to_value(&params)?;
        self.client.post("/embeddings", body).await
    }
}
