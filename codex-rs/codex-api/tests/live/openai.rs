//! Live integration tests for OpenAI adapter.
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p codex-api --test live openai -- --test-threads=1
//! ```

use anyhow::Result;

use crate::common::TEST_RED_SQUARE_BASE64;
use crate::common::adapter_config;
use crate::common::extract_text;
use crate::common::has_function_call;
use crate::common::image_prompt;
use crate::common::text_prompt;
use crate::common::tool_prompt;
use crate::common::weather_tool;
use crate::common::{self};
use crate::require_provider;

#[tokio::test]
async fn test_text_generation() -> Result<()> {
    let cfg = require_provider!("openai");

    // OpenAI uses built-in handling, not an adapter
    // Skip if openai adapter doesn't exist (uses native endpoint)
    if common::get_adapter("openai").is_none() {
        eprintln!("OpenAI uses native endpoint, skipping adapter test");
        return Ok(());
    }

    let adapter = common::get_adapter("openai").unwrap();
    let config = adapter_config(&cfg);

    let prompt = text_prompt("Say 'hello' in exactly one word, nothing else.");
    let result = adapter.generate(&prompt, &config).await?;

    let text = extract_text(&result);
    assert!(
        text.to_lowercase().contains("hello"),
        "Expected 'hello' in response, got: {}",
        text
    );
    Ok(())
}

#[tokio::test]
async fn test_tool_calling() -> Result<()> {
    let cfg = require_provider!("openai");

    if common::get_adapter("openai").is_none() {
        eprintln!("OpenAI uses native endpoint, skipping adapter test");
        return Ok(());
    }

    let adapter = common::get_adapter("openai").unwrap();
    let config = adapter_config(&cfg);

    let prompt = tool_prompt(
        "What's the weather in Tokyo? Use the get_weather tool.",
        vec![weather_tool()],
    );
    let result = adapter.generate(&prompt, &config).await?;

    assert!(
        has_function_call(&result, "get_weather"),
        "Expected get_weather function call in response"
    );
    Ok(())
}

#[tokio::test]
async fn test_image_understanding() -> Result<()> {
    let cfg = require_provider!("openai");

    if common::get_adapter("openai").is_none() {
        eprintln!("OpenAI uses native endpoint, skipping adapter test");
        return Ok(());
    }

    let adapter = common::get_adapter("openai").unwrap();
    let config = adapter_config(&cfg);

    let prompt = image_prompt(
        "What color is this square? Answer with just the color name.",
        TEST_RED_SQUARE_BASE64,
    );
    let result = adapter.generate(&prompt, &config).await?;

    let text = extract_text(&result);
    assert!(
        text.to_lowercase().contains("red"),
        "Expected 'red' in response, got: {}",
        text
    );
    Ok(())
}
