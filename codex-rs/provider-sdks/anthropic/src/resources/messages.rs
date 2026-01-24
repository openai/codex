use crate::client::Client;
use crate::error::Result;
use crate::streaming::EventStream;
use crate::streaming::MessageStream;
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
        self.client.post_message("/v1/messages", body).await
    }

    /// Create a streaming message.
    ///
    /// Returns a `MessageStream` that yields raw events and can accumulate
    /// them into a complete `Message`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anthropic_sdk::{Client, MessageCreateParams, MessageParam, RawMessageStreamEvent, ContentBlockDelta};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::from_env()?;
    ///
    /// let mut stream = client.messages().create_stream(
    ///     MessageCreateParams::new(
    ///         "claude-sonnet-4-20250514",
    ///         1024,
    ///         vec![MessageParam::user("Tell me a story")],
    ///     )
    /// ).await?;
    ///
    /// // Option 1: Process individual events
    /// while let Some(event) = stream.next_event().await {
    ///     match event? {
    ///         RawMessageStreamEvent::ContentBlockDelta { delta, .. } => {
    ///             if let ContentBlockDelta::TextDelta { text } = delta {
    ///                 print!("{}", text);
    ///             }
    ///         }
    ///         _ => {}
    ///     }
    /// }
    ///
    /// // Option 2: Get final message (consumes stream)
    /// // let message = stream.get_final_message().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_stream(&self, params: MessageCreateParams) -> Result<MessageStream> {
        // Add stream: true to the request body
        let mut body = serde_json::to_value(&params)?;
        if let Some(obj) = body.as_object_mut() {
            obj.insert("stream".to_string(), serde_json::Value::Bool(true));
        }

        let event_stream = self.client.post_stream("/v1/messages", body).await?;
        Ok(MessageStream::new(event_stream))
    }

    /// Create a streaming message and return raw events stream.
    ///
    /// This is a lower-level method that returns the raw event stream
    /// without the `MessageStream` wrapper.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anthropic_sdk::{Client, MessageCreateParams, MessageParam, RawMessageStreamEvent};
    /// use futures::StreamExt;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::from_env()?;
    ///
    /// let mut stream = client.messages().create_stream_raw(
    ///     MessageCreateParams::new(
    ///         "claude-sonnet-4-20250514",
    ///         1024,
    ///         vec![MessageParam::user("Hello!")],
    ///     )
    /// ).await?;
    ///
    /// while let Some(event) = stream.next().await {
    ///     println!("{:?}", event?);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_stream_raw(&self, params: MessageCreateParams) -> Result<EventStream> {
        let mut body = serde_json::to_value(&params)?;
        if let Some(obj) = body.as_object_mut() {
            obj.insert("stream".to_string(), serde_json::Value::Bool(true));
        }

        self.client.post_stream("/v1/messages", body).await
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
