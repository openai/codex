use anyhow::Context;
use anyhow::Result;
use serde_json::Value;
use serde_json::json;

use crate::cdp::CdpClient;
use crate::input::BrowserKeyInput;
use crate::key_event;
use crate::scripts;

const MAX_SCREENSHOT_BYTES: usize = 4 * 1024 * 1024;
const MAX_SNAPSHOT_OUTPUT_BYTES: usize = 8 * 1024;
const MAX_SNAPSHOT_URL_CHARS: usize = 2_048;
const MAX_SNAPSHOT_TITLE_CHARS: usize = 512;
const MAX_SNAPSHOT_TEXT_CHARS: usize = 6_000;

pub(crate) struct PageMetadata {
    pub(crate) url: Option<String>,
    pub(crate) title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserToolOutput {
    Text(String),
    ImageDataUrl(String),
}

pub(crate) async fn page_metadata(client: &CdpClient) -> Result<PageMetadata> {
    let metadata = client
        .evaluate("({ url: location.href, title: document.title })")
        .await?;
    Ok(PageMetadata {
        url: metadata
            .get("url")
            .and_then(Value::as_str)
            .map(|url| clipped(url, MAX_SNAPSHOT_URL_CHARS)),
        title: metadata
            .get("title")
            .and_then(Value::as_str)
            .map(|title| clipped(title, MAX_SNAPSHOT_TITLE_CHARS)),
    })
}

pub(crate) async fn press(client: &CdpClient, key: &str) -> Result<BrowserToolOutput> {
    anyhow::ensure!(!key.is_empty(), "key must not be empty");
    anyhow::ensure!(key.chars().count() <= 32, "key is too long");
    key_event::dispatch_tool_key(client, key).await?;
    Ok(BrowserToolOutput::Text(format!("pressed {key}")))
}

pub(crate) async fn scroll(
    client: &CdpClient,
    delta_x: i64,
    delta_y: i64,
) -> Result<BrowserToolOutput> {
    let result = client
        .evaluate(&scripts::scroll_expression(delta_x, delta_y))
        .await?;
    Ok(BrowserToolOutput::Text(serde_json::to_string(&result)?))
}

pub(crate) async fn screenshot(client: &CdpClient) -> Result<BrowserToolOutput> {
    let result = client
        .call(
            "Page.captureScreenshot",
            json!({ "format": "png", "captureBeyondViewport": false }),
        )
        .await?;
    let data = result
        .get("data")
        .and_then(Value::as_str)
        .context("screenshot response did not include image data")?;
    let decoded_size_estimate = data.len().saturating_mul(/*rhs*/ 3) / 4;
    anyhow::ensure!(
        decoded_size_estimate <= MAX_SCREENSHOT_BYTES,
        "screenshot exceeds the 4 MiB tool-output limit"
    );
    Ok(BrowserToolOutput::ImageDataUrl(format!(
        "data:image/png;base64,{data}"
    )))
}

pub(crate) async fn dispatch_human_key(client: &CdpClient, input: &BrowserKeyInput) -> Result<()> {
    key_event::dispatch_human_key(client, input).await
}

pub(crate) async fn insert_human_text(client: &CdpClient, text: &str) -> Result<()> {
    client
        .call("Input.insertText", json!({ "text": text }))
        .await?;
    Ok(())
}

pub(crate) fn bounded_snapshot_json(mut snapshot: Value) -> Result<String> {
    truncate_string_field(&mut snapshot, "url", MAX_SNAPSHOT_URL_CHARS);
    truncate_string_field(&mut snapshot, "title", MAX_SNAPSHOT_TITLE_CHARS);
    truncate_string_field(&mut snapshot, "text", MAX_SNAPSHOT_TEXT_CHARS);
    let mut output = serde_json::to_string_pretty(&snapshot)?;
    if output.len() <= MAX_SNAPSHOT_OUTPUT_BYTES {
        return Ok(output);
    }

    if let Some(object) = snapshot.as_object_mut() {
        object.insert("truncated".to_string(), Value::Bool(true));
        if let Some(text) = object.get("text").and_then(Value::as_str) {
            object.insert(
                "text".to_string(),
                Value::String(text.chars().take(/*n*/ 8_000).collect()),
            );
        }
    }
    loop {
        output = serde_json::to_string_pretty(&snapshot)?;
        if output.len() <= MAX_SNAPSHOT_OUTPUT_BYTES {
            return Ok(output);
        }
        let removed = snapshot
            .get_mut("nodes")
            .and_then(Value::as_array_mut)
            .and_then(Vec::pop)
            .is_some();
        if !removed {
            break;
        }
    }

    Ok(serde_json::to_string_pretty(&json!({
        "url": snapshot.get("url").cloned().unwrap_or(Value::Null),
        "title": snapshot.get("title").cloned().unwrap_or(Value::Null),
        "truncated": true,
        "text": "Snapshot exceeded the output limit. Narrow the page state and take another snapshot."
    }))?)
}

fn truncate_string_field(value: &mut Value, field: &str, max_chars: usize) {
    let Some(text) = value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
    else {
        return;
    };
    if let Some(object) = value.as_object_mut() {
        object.insert(field.to_string(), Value::String(clipped(&text, max_chars)));
    }
}

fn clipped(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;
