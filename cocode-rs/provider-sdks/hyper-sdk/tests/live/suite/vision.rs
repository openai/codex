//! Vision/image understanding tests.
//!
//! Tests multi-modal image understanding capabilities.

use anyhow::Result;
use hyper_sdk::Model;
use std::sync::Arc;

use crate::common::TEST_RED_SQUARE_BASE64;
use crate::common::extract_text;
use crate::common::image_request;

/// Test image understanding.
///
/// Verifies that the model can analyze and describe images.
pub async fn run(model: &Arc<dyn Model>) -> Result<()> {
    let request = image_request(
        "What color is this square? Answer with just the color name.",
        TEST_RED_SQUARE_BASE64,
    );
    let response = model.generate(request).await?;

    let text = extract_text(&response);
    assert!(
        text.to_lowercase().contains("red"),
        "Expected 'red' in response, got: {}",
        text
    );
    Ok(())
}
