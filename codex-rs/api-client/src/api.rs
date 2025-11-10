use async_trait::async_trait;

use crate::error::Result;
use crate::prompt::Prompt;
use crate::stream::ResponseStream;

#[async_trait]
pub trait ApiClient: Sized {
    type Config;

    /// Construct a new client instance from the provided configuration.
    ///
    /// This is synchronous to avoid forcing callers to `await` when no async
    /// work is needed during construction. If an implementation needs async
    /// initialization, prefer doing it inside `stream` or provide an explicit
    /// async initializer on the concrete type.
    fn new(config: Self::Config) -> Result<Self>;

    /// Start a streaming request for the given prompt.
    async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream>;
}
