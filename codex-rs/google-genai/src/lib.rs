//! # Google Generative AI (Gemini) Rust Client
//!
//! A Rust client library for the Google Generative AI (Gemini) API.
//!
//! ## Features
//!
//! - **Chat**: Stateful conversation management with history
//! - **Tool Calling**: Function calling / tool use support
//! - **Multimodal**: Support for images and other media types
//! - **Streaming**: SSE-based streaming responses via `streamGenerateContent`
//! - **Non-streaming**: Synchronous request/response API via `generateContent`
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use google_genai::{Client, ClientConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create client from environment variable (GOOGLE_API_KEY or GEMINI_API_KEY)
//!     let client = Client::from_env()?;
//!
//!     // Simple text generation
//!     let response = client
//!         .generate_content_text("gemini-2.0-flash", "Hello, how are you?", None)
//!         .await?;
//!
//!     println!("{}", response.text().unwrap_or_default());
//!     Ok(())
//! }
//! ```
//!
//! ## Chat Sessions
//!
//! ```rust,no_run
//! use google_genai::{Client, Chat, ChatBuilder};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = Client::from_env()?;
//!
//!     // Create a chat session
//!     let mut chat = ChatBuilder::new(client, "gemini-2.0-flash")
//!         .system_instruction("You are a helpful assistant.")
//!         .temperature(0.7)
//!         .build();
//!
//!     // Send messages
//!     let response = chat.send_message("What is Rust?").await?;
//!     println!("{}", response.text().unwrap_or_default());
//!
//!     // Continue the conversation
//!     let response = chat.send_message("How do I use it?").await?;
//!     println!("{}", response.text().unwrap_or_default());
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Tool Calling
//!
//! ```rust,no_run
//! use google_genai::{Client, types::*};
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = Client::from_env()?;
//!
//!     // Define a function
//!     let get_weather = FunctionDeclaration::new("get_weather")
//!         .with_description("Get the current weather for a location")
//!         .with_parameters(
//!             Schema::object(HashMap::from([
//!                 ("location".to_string(), Schema::string().with_description("The city name")),
//!             ]))
//!             .with_required(vec!["location".to_string()])
//!         );
//!
//!     let tools = vec![Tool::functions(vec![get_weather])];
//!
//!     let response = client
//!         .generate_content_with_tools(
//!             "gemini-2.0-flash",
//!             vec![Content::user("What's the weather in Tokyo?")],
//!             tools,
//!             None,
//!         )
//!         .await?;
//!
//!     // Handle function calls
//!     if let Some(calls) = response.function_calls() {
//!         for call in calls {
//!             println!("Function: {:?}, Args: {:?}", call.name, call.args);
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Multimodal (Images)
//!
//! ```rust,no_run
//! use google_genai::{Client, Chat};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = Client::from_env()?;
//!     let mut chat = Chat::new(client, "gemini-2.0-flash");
//!
//!     // Send message with image bytes
//!     let image_data = std::fs::read("image.jpg")?;
//!     let response = chat
//!         .send_message_with_image("What's in this image?", &image_data, "image/jpeg")
//!         .await?;
//!
//!     println!("{}", response.text().unwrap_or_default());
//!     Ok(())
//! }
//! ```

pub mod chat;
pub mod client;
pub mod error;
pub mod stream;
pub mod types;

// Re-export main types at crate root for convenience
pub use chat::Chat;
pub use chat::ChatBuilder;
pub use client::Client;
pub use client::ClientConfig;
pub use error::GenAiError;
pub use error::Result;

// Re-export streaming types
pub use stream::ContentStream;

// Re-export commonly used types
pub use types::Content;
pub use types::FunctionCall;
pub use types::FunctionDeclaration;
pub use types::FunctionResponse;
pub use types::GenerateContentConfig;
pub use types::GenerateContentResponse;
pub use types::Part;
pub use types::RequestExtensions;
pub use types::Schema;
pub use types::Tool;
