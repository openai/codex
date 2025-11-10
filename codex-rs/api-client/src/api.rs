use async_trait::async_trait;

use crate::error::Result;
use crate::prompt::Prompt;
use crate::stream::ResponseStream;

#[async_trait]
pub trait ApiClient: Sized {
    type Config;

    async fn new(config: Self::Config) -> Result<Self>;
    async fn stream(&self, prompt: Prompt) -> Result<ResponseStream>;
}
