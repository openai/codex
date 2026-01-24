//! Responses resource for the Volcengine Ark API.

use crate::client::Client;
use crate::error::Result;
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
    /// use volcengine_ark_sdk::{Client, ResponseCreateParams, InputMessage};
    ///
    /// let client = Client::with_api_key("ark-xxx")?;
    /// let params = ResponseCreateParams::new("ep-xxx", vec![
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
}
