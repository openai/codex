use crate::client::Client;
use crate::error::Result;
use crate::types::CountTokensParams;
use crate::types::Message;
use crate::types::MessageCreateParams;
use crate::types::MessageTokensCount;

/// Messages resource for creating and managing messages.
pub struct Messages<'a> {
    client: &'a Client,
}

impl<'a> Messages<'a> {
    /// Create a new Messages resource.
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Create a message.
    ///
    /// This sends a structured list of input messages and returns a Message object
    /// containing the model's response.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anthropic_sdk::{Client, MessageCreateParams, MessageParam};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::from_env()?;
    ///
    /// let message = client.messages().create(
    ///     MessageCreateParams::new(
    ///         "claude-3-5-sonnet-20241022",
    ///         1024,
    ///         vec![MessageParam::user("Hello, Claude!")],
    ///     )
    /// ).await?;
    ///
    /// println!("{}", message.text());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create(&self, params: MessageCreateParams) -> Result<Message> {
        let body = serde_json::to_value(&params)?;
        self.client.post("/v1/messages", body).await
    }

    /// Count the number of tokens in a message.
    ///
    /// This counts the tokens for the provided messages, system prompt, and tools
    /// without creating a message.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anthropic_sdk::{Client, CountTokensParams, MessageParam};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::from_env()?;
    ///
    /// let count = client.messages().count_tokens(
    ///     CountTokensParams::new(
    ///         "claude-3-5-sonnet-20241022",
    ///         vec![MessageParam::user("Hello, world!")],
    ///     )
    /// ).await?;
    ///
    /// println!("Input tokens: {}", count.input_tokens);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn count_tokens(&self, params: CountTokensParams) -> Result<MessageTokensCount> {
        let body = serde_json::to_value(&params)?;
        self.client.post("/v1/messages/count_tokens", body).await
    }
}
