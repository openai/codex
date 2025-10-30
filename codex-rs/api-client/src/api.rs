use async_trait::async_trait;

use crate::error::Error;
use crate::prompt::Prompt;
use crate::stream::ResponseStream;

#[async_trait]
pub trait ApiClient: Send + Sync {
    type Config: Send + Sync;

    async fn new(config: Self::Config) -> Result<Self, Error>
    where
        Self: Sized;

    async fn stream(&self, prompt: Prompt) -> Result<ResponseStream, Error>;
}
