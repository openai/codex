//! Chat completions resource for Z.AI SDK.

use crate::client::BaseClient;
use crate::error::Result;
use crate::types::ChatCompletionsCreateParams;
use crate::types::Completion;

/// Chat resource.
pub struct Chat<'a> {
    client: &'a BaseClient,
    accept_language: Option<&'a str>,
}

impl<'a> Chat<'a> {
    pub(crate) fn new(client: &'a BaseClient, accept_language: Option<&'a str>) -> Self {
        Self {
            client,
            accept_language,
        }
    }

    /// Get the completions resource.
    pub fn completions(&self) -> Completions<'_> {
        Completions {
            client: self.client,
            accept_language: self.accept_language,
        }
    }
}

/// Chat completions resource.
pub struct Completions<'a> {
    client: &'a BaseClient,
    accept_language: Option<&'a str>,
}

impl<'a> Completions<'a> {
    /// Create a chat completion (non-streaming).
    pub async fn create(&self, params: ChatCompletionsCreateParams) -> Result<Completion> {
        let body = serde_json::to_value(&params)?;
        self.client
            .post_completion("/chat/completions", body, self.accept_language)
            .await
    }
}
