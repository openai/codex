//! Object generation types for structured output.
//!
//! This module provides types for generating structured JSON output
//! that conforms to a JSON schema, similar to the Go Fantasy SDK's
//! GenerateObject and StreamObject functionality.

use crate::messages::Message;
use crate::options::ProviderOptions;
use crate::response::TokenUsage;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Request for generating structured object output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectRequest {
    /// Messages in the conversation.
    pub messages: Vec<Message>,
    /// JSON schema that the output must conform to.
    pub schema: Value,
    /// Optional name for the schema (used for documentation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_name: Option<String>,
    /// Optional description for the schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_description: Option<String>,
    /// Sampling temperature (0.0-2.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    /// Provider-specific options.
    #[serde(skip)]
    pub provider_options: Option<ProviderOptions>,
}

impl ObjectRequest {
    /// Create a new object request with a JSON schema.
    pub fn new(messages: Vec<Message>, schema: Value) -> Self {
        Self {
            messages,
            schema,
            schema_name: None,
            schema_description: None,
            temperature: None,
            max_tokens: None,
            provider_options: None,
        }
    }

    /// Create a request from a single text prompt.
    pub fn from_text(text: impl Into<String>, schema: Value) -> Self {
        Self::new(vec![Message::user(text)], schema)
    }

    /// Set the schema name.
    pub fn schema_name(mut self, name: impl Into<String>) -> Self {
        self.schema_name = Some(name.into());
        self
    }

    /// Set the schema description.
    pub fn schema_description(mut self, desc: impl Into<String>) -> Self {
        self.schema_description = Some(desc.into());
        self
    }

    /// Set the sampling temperature.
    pub fn temperature(mut self, t: f64) -> Self {
        self.temperature = Some(t);
        self
    }

    /// Set the maximum tokens to generate.
    pub fn max_tokens(mut self, n: i32) -> Self {
        self.max_tokens = Some(n);
        self
    }

    /// Set provider-specific options.
    pub fn provider_options(mut self, options: ProviderOptions) -> Self {
        self.provider_options = Some(options);
        self
    }
}

/// Response from object generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectResponse {
    /// Unique response ID.
    pub id: String,
    /// The generated object as JSON.
    pub object: Value,
    /// Token usage statistics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    /// Model that generated the response.
    pub model: String,
}

impl ObjectResponse {
    /// Create a new object response.
    pub fn new(id: impl Into<String>, model: impl Into<String>, object: Value) -> Self {
        Self {
            id: id.into(),
            object,
            usage: None,
            model: model.into(),
        }
    }

    /// Set token usage.
    pub fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }

    /// Parse the object into a typed value.
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.object.clone())
    }

    /// Get the raw JSON object.
    pub fn json(&self) -> &Value {
        &self.object
    }
}

/// Streaming event for object generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObjectStreamEvent {
    /// Object generation started.
    Started { id: String },
    /// Partial JSON delta.
    Delta { delta: String },
    /// Object generation completed.
    Done {
        id: String,
        object: Value,
        usage: Option<TokenUsage>,
    },
    /// Error during generation.
    Error { message: String },
}

impl ObjectStreamEvent {
    /// Create a started event.
    pub fn started(id: impl Into<String>) -> Self {
        Self::Started { id: id.into() }
    }

    /// Create a delta event.
    pub fn delta(delta: impl Into<String>) -> Self {
        Self::Delta {
            delta: delta.into(),
        }
    }

    /// Create a done event.
    pub fn done(id: impl Into<String>, object: Value, usage: Option<TokenUsage>) -> Self {
        Self::Done {
            id: id.into(),
            object,
            usage,
        }
    }

    /// Create an error event.
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }
}

/// Streaming response for object generation.
pub struct ObjectStreamResponse {
    /// The underlying async stream of events.
    pub stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<ObjectStreamEvent, crate::error::HyperError>> + Send>,
    >,
}

impl ObjectStreamResponse {
    /// Create from a stream.
    pub fn new(
        stream: impl futures::Stream<Item = Result<ObjectStreamEvent, crate::error::HyperError>>
        + Send
        + 'static,
    ) -> Self {
        Self {
            stream: Box::pin(stream),
        }
    }
}

impl std::fmt::Debug for ObjectStreamResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectStreamResponse")
            .field("stream", &"<stream>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_request() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            },
            "required": ["name", "age"]
        });

        let request = ObjectRequest::from_text("Generate a person", schema.clone())
            .schema_name("Person")
            .temperature(0.7)
            .max_tokens(100);

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.schema, schema);
        assert_eq!(request.schema_name, Some("Person".to_string()));
        assert_eq!(request.temperature, Some(0.7));
    }

    #[test]
    fn test_object_response_parse() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Person {
            name: String,
            age: i32,
        }

        let response = ObjectResponse::new(
            "resp_1",
            "gpt-4o",
            serde_json::json!({
                "name": "Alice",
                "age": 30
            }),
        );

        let person: Person = response.parse().unwrap();
        assert_eq!(
            person,
            Person {
                name: "Alice".to_string(),
                age: 30
            }
        );
    }

    #[test]
    fn test_object_stream_events() {
        let started = ObjectStreamEvent::started("resp_1");
        assert!(matches!(started, ObjectStreamEvent::Started { id } if id == "resp_1"));

        let delta = ObjectStreamEvent::delta(r#"{"name":"#);
        assert!(matches!(delta, ObjectStreamEvent::Delta { delta } if delta == r#"{"name":"#));

        let done = ObjectStreamEvent::done("resp_1", serde_json::json!({"name": "Alice"}), None);
        assert!(matches!(done, ObjectStreamEvent::Done { .. }));
    }
}
