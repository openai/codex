//! Embeddings resource for Z.AI SDK.

use crate::client::BaseClient;
use crate::error::Result;
use crate::types::EmbeddingsCreateParams;
use crate::types::EmbeddingsResponded;

/// Embeddings resource.
pub struct Embeddings<'a> {
    client: &'a BaseClient,
    accept_language: Option<&'a str>,
}

impl<'a> Embeddings<'a> {
    pub(crate) fn new(client: &'a BaseClient, accept_language: Option<&'a str>) -> Self {
        Self {
            client,
            accept_language,
        }
    }

    /// Create embeddings.
    pub async fn create(&self, params: EmbeddingsCreateParams) -> Result<EmbeddingsResponded> {
        let body = serde_json::to_value(&params)?;
        self.client
            .post("/embeddings", body, self.accept_language)
            .await
    }
}
