use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use serde_json::Value;
use serde_json::json;

use crate::cdp::CdpClient;
use crate::scripts;

const MAX_SCREENSHOT_BYTES: usize = 4 * 1024 * 1024;
const MAX_SNAPSHOT_OUTPUT_BYTES: usize = 32 * 1024;
const MAX_SNAPSHOT_URL_CHARS: usize = 2_048;
const MAX_SNAPSHOT_TITLE_CHARS: usize = 512;
const MAX_SNAPSHOT_TEXT_CHARS: usize = 12_000;

pub(crate) struct PageMetadata {
    pub(crate) url: Option<String>,
    pub(crate) title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserToolOutput {
    Text(String),
    ImageDataUrl(String),
}

pub(crate) async fn navigate(client: &mut CdpClient, url: &str) -> Result<()> {
    client
        .evaluate("globalThis.__codexTerminalBrowserNavigationMarker = true")
        .await?;
    let result = client.call("Page.navigate", json!({ "url": url })).await?;
    if let Some(error) = result.get("errorText").and_then(Value::as_str) {
        anyhow::bail!("navigation failed: {error}");
    }
    let cross_document = result.get("loaderId").and_then(Value::as_str).is_some();
    wait_until_ready(client, cross_document).await
}

pub(crate) async fn page_metadata(client: &mut CdpClient) -> Result<PageMetadata> {
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

pub(crate) async fn snapshot(client: &mut CdpClient) -> Result<BrowserToolOutput> {
    let snapshot = client.evaluate(scripts::SNAPSHOT_EXPRESSION).await?;
    Ok(BrowserToolOutput::Text(bounded_snapshot_json(snapshot)?))
}

pub(crate) async fn click(client: &mut CdpClient, node_id: &str) -> Result<BrowserToolOutput> {
    let result = client
        .evaluate(&scripts::click_expression(node_id)?)
        .await?;
    ensure_script_ok(&result)?;
    Ok(BrowserToolOutput::Text(format!("clicked {node_id}")))
}

pub(crate) async fn fill(
    client: &mut CdpClient,
    node_id: &str,
    text: &str,
) -> Result<BrowserToolOutput> {
    let result = client
        .evaluate(&scripts::fill_expression(node_id, text)?)
        .await?;
    ensure_script_ok(&result)?;
    Ok(BrowserToolOutput::Text(format!("filled {node_id}")))
}

pub(crate) async fn press(client: &mut CdpClient, key: &str) -> Result<BrowserToolOutput> {
    anyhow::ensure!(!key.is_empty(), "key must not be empty");
    anyhow::ensure!(key.chars().count() <= 32, "key is too long");
    let code = scripts::key_code(key);
    let text = if key.chars().count() == 1 { key } else { "" };
    client
        .call(
            "Input.dispatchKeyEvent",
            json!({
                "type": "rawKeyDown",
                "key": key,
                "code": code,
                "text": text,
                "unmodifiedText": text,
            }),
        )
        .await?;
    client
        .call(
            "Input.dispatchKeyEvent",
            json!({ "type": "keyUp", "key": key, "code": code }),
        )
        .await?;
    Ok(BrowserToolOutput::Text(format!("pressed {key}")))
}

pub(crate) async fn scroll(
    client: &mut CdpClient,
    delta_x: i64,
    delta_y: i64,
) -> Result<BrowserToolOutput> {
    let result = client
        .evaluate(&scripts::scroll_expression(delta_x, delta_y))
        .await?;
    Ok(BrowserToolOutput::Text(serde_json::to_string(&result)?))
}

pub(crate) async fn screenshot(client: &mut CdpClient) -> Result<BrowserToolOutput> {
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

async fn wait_until_ready(client: &mut CdpClient, cross_document: bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 15);
    loop {
        if let Ok(state) = client
            .evaluate(
                "({ state: document.readyState, marker: globalThis.__codexTerminalBrowserNavigationMarker === true })",
            )
            .await
        {
            let ready = matches!(
                state.get("state").and_then(Value::as_str),
                Some("interactive" | "complete")
            );
            let old_document = state
                .get("marker")
                .and_then(Value::as_bool)
                .unwrap_or(/*default*/ false);
            if ready && (!cross_document || !old_document) {
                return Ok(());
            }
        }
        anyhow::ensure!(Instant::now() < deadline, "navigation timed out");
        tokio::time::sleep(Duration::from_millis(/*millis*/ 100)).await;
    }
}

fn ensure_script_ok(result: &Value) -> Result<()> {
    if result.get("ok").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    let error = result
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("browser action failed");
    anyhow::bail!("{error}; take a new snapshot and retry")
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
