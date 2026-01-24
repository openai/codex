//! Responses resource for the OpenAI API.

use crate::client::Client;
use crate::error::Result;
use crate::streaming::ResponseStream;
use crate::types::Response;
use crate::types::ResponseCreateParams;

/// Responses resource for creating API responses.
pub struct Responses<'a> {
    client: &'a Client,
}

impl<'a> Responses<'a> {
    /// Create a new Responses resource.
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Create a response (non-streaming).
    ///
    /// # Arguments
    ///
    /// * `params` - The parameters for creating the response
    ///
    /// # Returns
    ///
    /// The API response containing generated content.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use openai_sdk::{Client, ResponseCreateParams, InputMessage};
    ///
    /// let client = Client::from_env()?;
    /// let params = ResponseCreateParams::new("gpt-4o", vec![
    ///     InputMessage::user_text("Hello!")
    /// ]);
    ///
    /// let response = client.responses().create(params).await?;
    /// println!("Response: {}", response.text());
    /// ```
    pub async fn create(&self, params: ResponseCreateParams) -> Result<Response> {
        let body = serde_json::to_value(&params)?;
        self.client.post_response("/responses", body).await
    }

    /// Retrieve a response by ID.
    ///
    /// Use this to check the status of a background response or fetch
    /// a previously created response.
    ///
    /// # Arguments
    ///
    /// * `response_id` - The ID of the response to retrieve
    ///
    /// # Returns
    ///
    /// The response with current status and output.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use openai_sdk::Client;
    ///
    /// let client = Client::from_env()?;
    /// let response = client.responses().retrieve("resp-abc123").await?;
    /// println!("Status: {:?}", response.status);
    /// ```
    pub async fn retrieve(&self, response_id: impl AsRef<str>) -> Result<Response> {
        let path = format!("/responses/{}", response_id.as_ref());
        self.client.get_response(&path).await
    }

    /// Cancel a background response.
    ///
    /// Only works for responses created with `background: true` that are
    /// still in progress.
    ///
    /// # Arguments
    ///
    /// * `response_id` - The ID of the response to cancel
    ///
    /// # Returns
    ///
    /// The cancelled response.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use openai_sdk::Client;
    ///
    /// let client = Client::from_env()?;
    /// let response = client.responses().cancel("resp-abc123").await?;
    /// assert_eq!(response.status, ResponseStatus::Cancelled);
    /// ```
    pub async fn cancel(&self, response_id: impl AsRef<str>) -> Result<Response> {
        let path = format!("/responses/{}/cancel", response_id.as_ref());
        self.client
            .post_response(&path, serde_json::json!({}))
            .await
    }

    /// Create a streaming response.
    ///
    /// Returns a stream of events that can be iterated over. Each event
    /// provides incremental updates as the model generates content.
    ///
    /// # Arguments
    ///
    /// * `params` - The parameters for creating the response
    ///
    /// # Returns
    ///
    /// A `ResponseStream` that yields `ResponseStreamEvent` items.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use openai_sdk::{Client, ResponseCreateParams, InputMessage, ResponseStreamEvent};
    ///
    /// let client = Client::from_env()?;
    /// let params = ResponseCreateParams::new("gpt-4o", vec![
    ///     InputMessage::user_text("Hello!")
    /// ]);
    ///
    /// let mut stream = client.responses().stream(params).await?;
    ///
    /// // Iterate over events
    /// while let Some(event) = stream.next().await {
    ///     match event? {
    ///         ResponseStreamEvent::OutputTextDelta { delta, .. } => {
    ///             print!("{}", delta);
    ///         }
    ///         ResponseStreamEvent::ResponseCompleted { response, .. } => {
    ///             println!("\n\nDone! Response ID: {}", response.id);
    ///         }
    ///         _ => {}
    ///     }
    /// }
    ///
    /// // Or collect to final response
    /// let mut stream = client.responses().stream(params).await?;
    /// let response = stream.collect_response().await?;
    /// ```
    pub async fn stream(&self, params: ResponseCreateParams) -> Result<ResponseStream> {
        let mut body = serde_json::to_value(&params)?;
        // Set stream=true in the request body
        if let Some(obj) = body.as_object_mut() {
            obj.insert("stream".to_string(), serde_json::Value::Bool(true));
        }
        let byte_stream = self.client.post_stream("/responses", body).await?;
        Ok(ResponseStream::new(byte_stream))
    }

    /// Resume streaming from an existing response.
    ///
    /// This allows you to continue receiving events from a response that was
    /// interrupted, starting from a specific sequence number.
    ///
    /// # Arguments
    ///
    /// * `response_id` - The ID of the response to stream from
    /// * `starting_after` - The sequence number after which to start streaming.
    ///   Events with sequence numbers <= this value will be skipped.
    ///
    /// # Returns
    ///
    /// A `ResponseStream` that yields `ResponseStreamEvent` items starting
    /// from the specified sequence number.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use openai_sdk::{Client, ResponseStreamEvent};
    ///
    /// let client = Client::from_env()?;
    ///
    /// // Resume from sequence number 10
    /// let mut stream = client.responses().stream_from("resp-abc123", Some(10)).await?;
    ///
    /// while let Some(event) = stream.next().await {
    ///     match event? {
    ///         ResponseStreamEvent::OutputTextDelta { delta, sequence_number, .. } => {
    ///             println!("[{}] {}", sequence_number, delta);
    ///         }
    ///         _ => {}
    ///     }
    /// }
    /// ```
    pub async fn stream_from(
        &self,
        response_id: impl AsRef<str>,
        starting_after: Option<i32>,
    ) -> Result<ResponseStream> {
        let mut path = format!("/responses/{}?stream=true", response_id.as_ref());
        if let Some(seq) = starting_after {
            path.push_str(&format!("&starting_after={seq}"));
        }
        let byte_stream = self.client.get_stream(&path).await?;
        Ok(ResponseStream::new(byte_stream))
    }
}
