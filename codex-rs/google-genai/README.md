# google-genai

Rust client for Google Generative AI (Gemini) API.

Reference: [python-genai](https://github.com/googleapis/python-genai) @ `feae46dd`

## Features

- **Chat**: Stateful conversation with history
- **Tool Calling**: Function calling support
- **Multimodal**: Images via bytes or URI
- **Non-streaming**: Synchronous request/response

## Quick Start

```rust
use google_genai::{Client, Chat, types::*};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env()?; // GOOGLE_API_KEY or GEMINI_API_KEY

    // Simple text
    let resp = client.generate_content_text("gemini-2.0-flash", "Hello!", None).await?;
    println!("{}", resp.text().unwrap_or_default());

    // Chat session
    let mut chat = Chat::new(client, "gemini-2.0-flash");
    let resp = chat.send_message("What is Rust?").await?;

    Ok(())
}
```

See [docs/STATUS.md](docs/STATUS.md) for full implementation status.
