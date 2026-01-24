//! Vision/image understanding tests.
//!
//! Tests multi-modal image understanding capabilities.

use anyhow::Result;
use codex_api::AdapterConfig;
use codex_api::ProviderAdapter;

use crate::common::TEST_RED_SQUARE_BASE64;
use crate::common::extract_text;
use crate::common::image_prompt;

/// Test image understanding.
///
/// Verifies that the adapter can analyze and describe images.
pub async fn run(adapter: &dyn ProviderAdapter, config: &AdapterConfig) -> Result<()> {
    let prompt = image_prompt(
        "What color is this square? Answer with just the color name.",
        TEST_RED_SQUARE_BASE64,
    );
    let result = adapter.generate(&prompt, config).await?;

    let text = extract_text(&result);
    assert!(
        text.to_lowercase().contains("red"),
        "Expected 'red' in response, got: {}",
        text
    );
    Ok(())
}
